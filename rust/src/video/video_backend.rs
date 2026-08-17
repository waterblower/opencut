use std::sync::Arc;

use gst::prelude::*;

use gstreamer as gst;
use gstreamer_app as gst_app;

use gstreamer_video as gst_video;

use parking_lot::Mutex;
use std::time::Duration;
use url::Url;

#[derive(Debug)]
pub(crate) struct VideoBackend {
    pipeline: gst::Pipeline,
    sink: gst_app::AppSink,
    current_frame: Arc<Mutex<Option<gst::Sample>>>,
    cached_position: Duration,
}

impl Drop for VideoBackend {
    fn drop(&mut self) {
        if let Err(error) = self.pipeline.set_state(gst::State::Null) {
            log::error!("could not stop video pipeline: {error}");
        }
    }
}

impl VideoBackend {
    // pub fn from_file(path: Path) -> Result<Self, String> {}
    pub(crate) fn open(uri: &Url) -> Result<Self, String> {
        gst::init().map_err(|error| format!("could not initialize GStreamer: {error}"))?;
        let caps = gst_video::VideoCapsBuilder::new()
            .format(gst_video::VideoFormat::Nv12)
            .pixel_aspect_ratio(gst::Fraction::new(1, 1))
            .build();
        let converter = gst::ElementFactory::make("videoconvert")
            .build()
            .map_err(|error| format!("could not create video converter: {error}"))?;
        let sink = gst_app::AppSink::builder()
            .name("opencut_player_video")
            .drop(true)
            .max_buffers(3)
            .enable_last_sample(false)
            .caps(&caps)
            .build();
        let video_sink = gst::Bin::new();
        video_sink
            .add(&converter)
            .map_err(|error| format!("could not add video converter: {error}"))?;
        video_sink
            .add(&sink)
            .map_err(|error| format!("could not add video sink: {error}"))?;
        converter
            .link(&sink)
            .map_err(|error| format!("could not link video sink: {error}"))?;
        let converter_sink_pad = converter
            .static_pad("sink")
            .ok_or_else(|| "video converter has no sink pad".to_string())?;
        let ghost_pad = gst::GhostPad::builder_with_target(&converter_sink_pad)
            .map_err(|error| format!("could not create video sink pad: {error}"))?
            .name("sink")
            .build();
        ghost_pad
            .set_active(true)
            .map_err(|error| format!("could not activate video sink pad: {error}"))?;
        video_sink
            .add_pad(&ghost_pad)
            .map_err(|error| format!("could not expose video sink pad: {error}"))?;
        let playbin = gst::ElementFactory::make("playbin")
            .property("uri", uri.as_str())
            .property("video-sink", &video_sink)
            .build()
            .map_err(|error| format!("could not create video pipeline: {error}"))?;
        let pipeline = playbin
            .downcast::<gst::Pipeline>()
            .map_err(|_| "video pipeline had an unexpected type".to_string())?;

        let video = Self::from_pipeline(pipeline, sink)?;
        video.set_paused(false);
        Ok(video)
    }

    pub(crate) fn from_pipeline(
        pipeline: gst::Pipeline,
        sink: gst_app::AppSink,
    ) -> Result<Self, String> {
        gst::init().map_err(|error| format!("could not initialize GStreamer: {error}"))?;
        let current_frame = Arc::new(Mutex::new(None));
        sink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample({
                    let current_frame = current_frame.clone();
                    move |sink| {
                        let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                        *current_frame.lock() = Some(sample);
                        Ok(gst::FlowSuccess::Ok)
                    }
                })
                .new_preroll({
                    let current_frame = current_frame.clone();
                    move |sink| {
                        let sample = sink.pull_preroll().map_err(|_| gst::FlowError::Eos)?;
                        *current_frame.lock() = Some(sample);
                        Ok(gst::FlowSuccess::Ok)
                    }
                })
                .build(),
        );
        pipeline
            .set_state(gst::State::Paused)
            .map_err(|error| format!("could not prepare video: {error}"))?;
        if let Err(error) = pipeline.state(gst::ClockTime::from_seconds(5)).0 {
            let _ = pipeline.set_state(gst::State::Null);
            return Err(format!("video did not finish preparing: {error}"));
        }

        // GStreamer negotiates `framerate=0/1` for variable-frame-rate sources such as
        // screen recordings. That is a valid "unknown rate" marker, not a broken file, so
        // it must not fail the load — the container's nominal rate is still available from
        // the probed asset when a caller needs one.

        Ok(VideoBackend {
            current_frame,
            pipeline,
            sink,
            cached_position: Duration::ZERO,
        })
    }

    pub(crate) fn frame_size(&self) -> Option<(u32, u32)> {
        let caps = self.cap().ok()?;
        let info = gst_video::VideoInfo::from_caps(&caps)
            .expect("negotiated AppSink caps must describe raw video");
        Some((info.width(), info.height()))
    }

    /// The negotiated frame rate, or `None` for variable-frame-rate sources where
    /// GStreamer reports `0/1`.
    pub(crate) fn framerate(&self) -> Option<f64> {
        let caps = self.cap().ok()?;
        let info = match gst_video::VideoInfo::from_caps(&caps) {
            Ok(info) => info,
            Err(_) => return None,
        };
        let framerate = frame_rate_from_caps(&info);
        return framerate;
    }

    pub(crate) fn duration(&self) -> Duration {
        let duration = Duration::from_nanos(
            self.pipeline()
                .query_duration::<gst::ClockTime>()
                .map_or(0, |duration| duration.nseconds()),
        );
        return duration;
    }

    pub(crate) fn position(&self) -> Duration {
        let Some(position) = self.pipeline.query_position::<gst::ClockTime>() else {
            return self.cached_position;
        };
        Duration::from_nanos(position.nseconds())
    }

    pub(crate) fn paused(&self) -> bool {
        self.pipeline.current_state() != gst::State::Playing
    }

    pub fn current_state(&self) -> gst::State {
        self.pipeline.current_state()
    }

    pub(crate) fn set_paused(&self, paused: bool) {
        let state = if paused {
            gst::State::Paused
        } else {
            gst::State::Playing
        };
        if let Err(error) = self.pipeline.set_state(state) {
            log::error!("could not change video playback state: {error}");
        }
    }

    pub(crate) fn seek(&mut self, position: Duration, accurate: bool) -> Result<(), String> {
        let mut flags = gst::SeekFlags::FLUSH;
        if accurate {
            flags |= gst::SeekFlags::ACCURATE;
        } else {
            flags |= gst::SeekFlags::KEY_UNIT | gst::SeekFlags::SNAP_AFTER;
        }
        self.pipeline
            .seek(
                1.0,
                flags,
                gst::SeekType::Set,
                gst::ClockTime::from_nseconds(position.as_nanos() as u64),
                gst::SeekType::None,
                gst::ClockTime::NONE,
            )
            .map_err(|error| format!("could not seek video: {error}"))?;
        self.cached_position = position;
        Ok(())
    }

    #[allow(dead_code)] // Used by the player binary, but not the editor binary.
    pub(crate) fn volume(&self) -> f64 {
        self.pipeline.property("volume")
    }

    pub(crate) fn set_volume(&self, volume: f64) {
        self.pipeline.set_property("volume", volume.clamp(0.0, 1.0));
    }

    #[allow(dead_code)] // Used by the player binary, but not the editor binary.
    pub(crate) fn muted(&self) -> bool {
        self.pipeline.property("mute")
    }

    pub(crate) fn set_muted(&self, muted: bool) {
        self.pipeline.set_property("mute", muted);
    }

    pub(crate) fn pipeline(&self) -> gst::Pipeline {
        self.pipeline.clone()
    }

    pub fn get_current_frame(&self) -> Option<gst::Sample> {
        self.current_frame.lock().clone()
    }

    //////////////////////
    // Private  Methods //
    //////////////////////
    fn cap(&self) -> Result<gst::Caps, String> {
        let pad = self
            .sink
            .static_pad("sink")
            .expect("AppSink must have a static sink pad");

        let Some(caps) = pad.current_caps() else {
            let _ = self.pipeline().set_state(gst::State::Null);
            return Err("video caps were not negotiated".to_string());
        };
        return Ok(caps);
    }
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
