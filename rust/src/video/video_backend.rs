use std::ops::{Deref, DerefMut};
use std::sync::Arc;

use anyhow::{Context as _, Error, Result, anyhow, bail};
use gst::prelude::*;

use gst_audio::prelude::StreamVolumeExt as _;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_audio as gst_audio;

use gstreamer_video as gst_video;

use parking_lot::Mutex;
use std::time::Duration;
use url::Url;

#[derive(Debug)]
pub(crate) struct VideoBackend {
    pipeline: gst::Pipeline,
    sink: gst_app::AppSink,
    volume_control: gst_audio::StreamVolume,
    current_frame: Arc<Mutex<Option<gst::Sample>>>,
    cached_position: Duration,
    _frame_size: (u32, u32),
}

impl Drop for VideoBackend {
    fn drop(&mut self) {
        if let Err(error) = self.pipeline.set_state(gst::State::Null) {
            log::error!("could not stop video pipeline: {error}");
        }
    }
}

impl VideoBackend {
    pub(crate) fn from_pipeline(
        pipeline: gst::Pipeline,
        sink: gst_app::AppSink,
        volume_control: gst_audio::StreamVolume,
    ) -> Result<Self> {
        gst::init().context("could not initialize GStreamer")?;
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
            .context("could not prepare video")?;
        if let Err(error) = pipeline.state(gst::ClockTime::from_seconds(5)).0 {
            let _ = pipeline.set_state(gst::State::Null);
            return Err(Error::new(error).context("video did not finish preparing"));
        }
        let Some(caps) = sink
            .static_pad("sink")
            .expect("AppSink must have a static sink pad")
            .current_caps()
        else {
            if let Err(err) = pipeline.set_state(gst::State::Null) {
                return Err(Error::new(err).context("could not set pipeline to null state"));
            }
            bail!("video caps were not negotiated at {}:{}", file!(), line!());
        };
        let info = match gst_video::VideoInfo::from_caps(&caps) {
            Ok(info) => info,
            Err(error) => {
                let _ = pipeline.set_state(gst::State::Null);
                return Err(Error::new(error).context("video caps did not describe raw video"));
            }
        };
        let frame_size = (info.width(), info.height());

        // GStreamer negotiates `framerate=0/1` for variable-frame-rate sources such as
        // screen recordings. That is a valid "unknown rate" marker, not a broken file, so
        // it must not fail the load — the container's nominal rate is still available from
        // the probed asset when a caller needs one.

        Ok(Self {
            current_frame,
            pipeline,
            sink,
            volume_control,
            cached_position: Duration::ZERO,
            _frame_size: frame_size,
        })
    }

    fn cap(&self) -> Result<gst::Caps, String> {
        let pad = self
            .sink
            .static_pad("sink")
            .expect("AppSink must have a static sink pad");

        let Some(caps) = pad.current_caps() else {
            return Err("video caps were not negotiated".to_string());
        };
        Ok(caps)
    }

    pub(crate) fn frame_size(&self) -> (u32, u32) {
        return self._frame_size;
    }

    /// The negotiated frame rate, or `None` for variable-frame-rate sources where
    /// GStreamer reports `0/1`.
    pub fn framerate(&self) -> Option<f64> {
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

    pub(crate) fn seek(&mut self, position: Duration) -> Result<(), String> {
        let mut flags = gst::SeekFlags::FLUSH;
        flags |= gst::SeekFlags::ACCURATE;

        let stop = self.duration();
        let final_frame_start = self
            .framerate()
            .map(|frame_rate| {
                let frame_duration = Duration::from_secs_f64(1.0 / frame_rate);
                stop.saturating_sub(frame_duration)
            })
            .unwrap_or_else(|| stop.saturating_sub(Duration::from_nanos(1)));
        let position = position.min(final_frame_start);

        self.pipeline
            .seek(
                1.0,
                flags,
                gst::SeekType::Set,
                gst::ClockTime::from_nseconds(position.as_nanos() as u64),
                gst::SeekType::Set,
                gst::ClockTime::from_nseconds(stop.as_nanos() as u64),
            )
            .map_err(|error| format!("could not seek video: {error}"))?;
        self.cached_position = position;
        Ok(())
    }

    #[allow(dead_code)] // Used by the player binary, but not the editor binary.
    pub(crate) fn volume(&self) -> f64 {
        self.volume_control
            .volume(gst_audio::StreamVolumeFormat::Linear)
    }

    pub(crate) fn set_volume(&self, volume: f64) {
        self.volume_control.set_volume(
            gst_audio::StreamVolumeFormat::Linear,
            volume.clamp(0.0, 1.0),
        );
    }

    #[allow(dead_code)] // Used by the player binary, but not the editor binary.
    pub(crate) fn muted(&self) -> bool {
        self.volume_control.is_muted()
    }

    pub(crate) fn set_muted(&self, muted: bool) {
        self.volume_control.set_mute(muted);
    }

    pub(crate) fn pipeline(&self) -> gst::Pipeline {
        self.pipeline.clone()
    }

    pub fn get_current_frame(&self) -> Option<gst::Sample> {
        self.current_frame.lock().clone()
    }
}

#[derive(Debug)]
pub(crate) struct FileVideoBackend {
    playback: VideoBackend,
}

impl FileVideoBackend {
    pub(crate) fn open(uri: &Url) -> Result<Self> {
        gst::init().context("could not initialize GStreamer")?;
        let caps = gst_video::VideoCapsBuilder::new()
            .format(gst_video::VideoFormat::Nv12)
            .pixel_aspect_ratio(gst::Fraction::new(1, 1))
            .build();
        let converter = gst::ElementFactory::make("videoconvert")
            .build()
            .context("could not create video converter")?;
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
            .context("could not add video converter")?;
        video_sink.add(&sink).context("could not add video sink")?;
        converter.link(&sink).context("could not link video sink")?;
        let converter_sink_pad = converter
            .static_pad("sink")
            .ok_or_else(|| anyhow!("video converter has no sink pad"))?;
        let ghost_pad = gst::GhostPad::builder_with_target(&converter_sink_pad)
            .context("could not create video sink pad")?
            .name("sink")
            .build();
        ghost_pad
            .set_active(true)
            .context("could not activate video sink pad")?;
        video_sink
            .add_pad(&ghost_pad)
            .context("could not expose video sink pad")?;
        let playbin = gst::ElementFactory::make("playbin")
            .property("uri", uri.as_str())
            .property("video-sink", &video_sink)
            .build()
            .context("could not create video pipeline")?;
        let pipeline = playbin
            .downcast::<gst::Pipeline>()
            .map_err(|_| anyhow!("video pipeline had an unexpected type"))?;

        let volume_control = pipeline
            .clone()
            .upcast::<gst::Element>()
            .dynamic_cast::<gst_audio::StreamVolume>()
            .map_err(|_| anyhow!("video pipeline does not support stream volume"))?;
        let video = VideoBackend::from_pipeline(pipeline, sink, volume_control)?;
        video.set_paused(false);
        Ok(Self { playback: video })
    }
}

impl Deref for FileVideoBackend {
    type Target = VideoBackend;

    fn deref(&self) -> &Self::Target {
        &self.playback
    }
}

impl DerefMut for FileVideoBackend {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.playback
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

#[cfg(test)]
#[path = "video_backend.test.rs"]
mod tests;
