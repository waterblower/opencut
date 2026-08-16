use core::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use crate::video2;

use gst::{message::MessageView, prelude::*};

use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_video as gst_video;

use parking_lot::Mutex;
use std::{sync::Arc, thread::JoinHandle, time::Duration};
use url::Url;

struct PlaybackState {
    alive: AtomicBool,
    frame: Mutex<Option<gst::Sample>>,
    frame_ready: AtomicBool,
    eos: AtomicBool,
    looping: AtomicBool,
    speed: AtomicU64,
}

struct VideoInner {
    pipeline: gst::Pipeline,
    state: Arc<PlaybackState>,
    worker: Mutex<Option<JoinHandle<()>>>,
    width: u32,
    height: u32,
    framerate: Option<f64>,
    duration: Duration,
}

impl Drop for VideoInner {
    fn drop(&mut self) {
        self.state.alive.store(false, Ordering::Release);
        if let Some(worker) = self.worker.lock().take()
            && let Err(error) = worker.join()
        {
            log::error!("video worker panicked: {error:?}");
        }
        if let Err(error) = self.pipeline.set_state(gst::State::Null) {
            log::error!("could not stop video pipeline: {error}");
        }
    }
}

#[derive(Clone)]
pub(crate) struct Video(Arc<VideoInner>);

impl Video {
    pub(crate) fn open(uri: &Url, looping: bool) -> Result<Self, String> {
        gst::init().map_err(|error| format!("could not initialize GStreamer: {error}"))?;
        let video_sink = gst::parse::bin_from_description(
            "videoconvert ! appsink name=opencut_player_video drop=true max-buffers=3 enable-last-sample=false caps=video/x-raw,format=NV12,pixel-aspect-ratio=1/1",
            true,
        )
        .map_err(|error| format!("could not create video sink: {error}"))?;
        let sink = video_sink
            .by_name("opencut_player_video")
            .ok_or_else(|| "video sink was not created".to_string())?
            .downcast::<gst_app::AppSink>()
            .map_err(|_| "video sink had an unexpected type".to_string())?;
        let playbin = gst::ElementFactory::make("playbin")
            .property("uri", uri.as_str())
            .property("video-sink", &video_sink)
            .build()
            .map_err(|error| format!("could not create video pipeline: {error}"))?;
        let pipeline = playbin
            .downcast::<gst::Pipeline>()
            .map_err(|_| "video pipeline had an unexpected type".to_string())?;

        let video = Self::from_pipeline(pipeline, sink, looping)?;
        video.set_paused(false);
        Ok(video)
    }

    pub(crate) fn from_pipeline(
        pipeline: gst::Pipeline,
        sink: gst_app::AppSink,
        looping: bool,
    ) -> Result<Self, String> {
        gst::init().map_err(|error| format!("could not initialize GStreamer: {error}"))?;
        pipeline
            .set_state(gst::State::Paused)
            .map_err(|error| format!("could not prepare video: {error}"))?;
        if let Err(error) = pipeline.state(gst::ClockTime::from_seconds(5)).0 {
            let _ = pipeline.set_state(gst::State::Null);
            return Err(format!("video did not finish preparing: {error}"));
        }

        let Some(caps) = sink.static_pad("sink").and_then(|pad| pad.current_caps()) else {
            let _ = pipeline.set_state(gst::State::Null);
            return Err("video format is unavailable".to_string());
        };
        let info = match gst_video::VideoInfo::from_caps(&caps) {
            Ok(info) => info,
            Err(error) => {
                let _ = pipeline.set_state(gst::State::Null);
                return Err(format!("could not read video format: {error}"));
            }
        };
        // GStreamer negotiates `framerate=0/1` for variable-frame-rate sources such as
        // screen recordings. That is a valid "unknown rate" marker, not a broken file, so
        // it must not fail the load — the container's nominal rate is still available from
        // the probed asset when a caller needs one.
        let framerate = frame_rate_from_caps(&info);
        let duration = Duration::from_nanos(
            pipeline
                .query_duration::<gst::ClockTime>()
                .map_or(0, |duration| duration.nseconds()),
        );
        let state = Arc::new(PlaybackState {
            alive: AtomicBool::new(true),
            frame: Mutex::new(None),
            frame_ready: AtomicBool::new(false),
            eos: AtomicBool::new(false),
            looping: AtomicBool::new(looping),
            speed: AtomicU64::new(1.0_f64.to_bits()),
        });
        let worker = spawn_video_worker(pipeline.clone(), sink, state.clone());
        let video = Self(Arc::new(VideoInner {
            pipeline,
            state,
            worker: Mutex::new(Some(worker)),
            width: info.width(),
            height: info.height(),
            framerate,
            duration,
        }));
        Ok(video)
    }

    pub(crate) fn display_size(&self) -> (u32, u32) {
        (self.0.width, self.0.height)
    }

    /// The negotiated frame rate, or `None` for variable-frame-rate sources where
    /// GStreamer reports `0/1`.
    pub(crate) fn framerate(&self) -> Option<f64> {
        self.0.framerate
    }

    pub fn frame(&self) -> Option<gst::Sample> {
        self.0.state.frame.lock().as_ref().cloned()
    }

    pub(crate) fn duration(&self) -> Duration {
        self.0.duration
    }

    pub(crate) fn position(&self) -> Duration {
        Duration::from_nanos(
            self.0
                .pipeline
                .query_position::<gst::ClockTime>()
                .map_or(0, |position| position.nseconds()),
        )
    }

    pub(crate) fn paused(&self) -> bool {
        self.0.pipeline.state(gst::ClockTime::ZERO).1 != gst::State::Playing
    }

    pub(crate) fn set_paused(&self, paused: bool) {
        let state = if paused {
            gst::State::Paused
        } else {
            gst::State::Playing
        };
        if let Err(error) = self.0.pipeline.set_state(state) {
            log::error!("could not change video playback state: {error}");
        }
    }

    pub(crate) fn seek(&self, position: Duration, accurate: bool) -> Result<(), String> {
        self.0.state.eos.store(false, Ordering::Release);
        let speed = self.speed();
        let mut flags = gst::SeekFlags::FLUSH;
        if accurate {
            flags |= gst::SeekFlags::ACCURATE;
        } else {
            flags |= gst::SeekFlags::KEY_UNIT | gst::SeekFlags::SNAP_AFTER;
        }
        self.0
            .pipeline
            .seek(
                speed,
                flags,
                gst::SeekType::Set,
                gst::ClockTime::from_nseconds(position.as_nanos() as u64),
                gst::SeekType::None,
                gst::ClockTime::NONE,
            )
            .map_err(|error| format!("could not seek video: {error}"))?;
        self.0.state.frame_ready.store(false, Ordering::Release);
        Ok(())
    }

    pub(crate) fn restart_stream(&self) -> Result<(), String> {
        self.seek(Duration::ZERO, false)?;
        self.set_paused(false);
        Ok(())
    }

    pub(crate) fn eos(&self) -> bool {
        self.0.state.eos.load(Ordering::Acquire)
    }

    pub(crate) fn speed(&self) -> f64 {
        f64::from_bits(self.0.state.speed.load(Ordering::Acquire))
    }

    #[allow(dead_code)] // Used by the player binary, but not the editor binary.
    pub(crate) fn set_speed(&self, speed: f64) -> Result<(), String> {
        if !speed.is_finite() || speed <= 0.0 {
            return Err("playback speed must be greater than zero".to_string());
        }
        let position = self
            .0
            .pipeline
            .query_position::<gst::ClockTime>()
            .ok_or_else(|| "video position is unavailable".to_string())?;
        self.0
            .pipeline
            .seek(
                speed,
                gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                gst::SeekType::Set,
                position,
                gst::SeekType::End,
                gst::ClockTime::ZERO,
            )
            .map_err(|error| format!("could not change playback speed: {error}"))?;
        self.0.state.speed.store(speed.to_bits(), Ordering::Release);
        Ok(())
    }

    #[allow(dead_code)] // Used by the player binary, but not the editor binary.
    pub(crate) fn volume(&self) -> f64 {
        self.0.pipeline.property("volume")
    }

    pub(crate) fn set_volume(&self, volume: f64) {
        self.0
            .pipeline
            .set_property("volume", volume.clamp(0.0, 1.0));
    }

    #[allow(dead_code)] // Used by the player binary, but not the editor binary.
    pub(crate) fn muted(&self) -> bool {
        self.0.pipeline.property("mute")
    }

    pub(crate) fn set_muted(&self, muted: bool) {
        self.0.pipeline.set_property("mute", muted);
    }

    pub(crate) fn pipeline(&self) -> gst::Pipeline {
        self.0.pipeline.clone()
    }
}

fn spawn_video_worker(
    pipeline: gst::Pipeline,
    sink: gst_app::AppSink,
    state: Arc<PlaybackState>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let Some(bus) = pipeline.bus() else {
            log::error!("video pipeline has no message bus");
            return;
        };
        while state.alive.load(Ordering::Acquire) {
            while let Some(message) = bus.timed_pop(gst::ClockTime::ZERO) {
                match message.view() {
                    MessageView::Eos(_) if state.looping.load(Ordering::Acquire) => {
                        state.eos.store(false, Ordering::Release);
                        if let Err(error) = pipeline.seek_simple(
                            gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT,
                            gst::ClockTime::ZERO,
                        ) {
                            log::error!("could not loop video: {error}");
                            state.eos.store(true, Ordering::Release);
                        }
                    }
                    MessageView::Eos(_) => state.eos.store(true, Ordering::Release),
                    MessageView::Error(error) => {
                        log::error!(
                            "GStreamer error from {:?}: {} ({})",
                            error.src(),
                            error.error(),
                            error.debug().unwrap_or_default()
                        );
                    }
                    _ => {}
                }
            }
            if state.eos.load(Ordering::Acquire) {
                std::thread::sleep(Duration::from_millis(25));
                continue;
            }
            let sample = if pipeline.state(gst::ClockTime::ZERO).1 == gst::State::Playing {
                sink.try_pull_sample(gst::ClockTime::from_mseconds(16))
            } else {
                sink.try_pull_preroll(gst::ClockTime::from_mseconds(16))
            };
            if let Some(sample) = sample {
                *state.frame.lock() = Some(sample);
                state.frame_ready.store(true, Ordering::Release);
            }
        }
    })
}

/// Reads the negotiated frame rate, returning `None` when it is unusable.
///
/// GStreamer uses `0/1` to mean "variable or unknown frame rate", which is what
/// variable-frame-rate sources such as screen recordings negotiate even when the
/// container declares a nominal rate. A zero denominator is likewise unusable.
fn frame_rate_from_caps(info: &gst_video::VideoInfo) -> Option<f64> {
    frame_rate_from_fraction(info.fps().numer(), info.fps().denom())
}

fn frame_rate_from_fraction(numerator: i32, denominator: i32) -> Option<f64> {
    if numerator <= 0 || denominator <= 0 {
        return None;
    }
    let frame_rate = numerator as f64 / denominator as f64;
    frame_rate.is_finite().then_some(frame_rate)
}

pub(crate) fn video(frame: Option<gst::Sample>, width: u32, height: u32) -> video2::VideoElement {
    video2::VideoElement {
        frame,
        width: gpui::Pixels::from(width),
        height: gpui::Pixels::from(height),
        id: None,
    }
}
