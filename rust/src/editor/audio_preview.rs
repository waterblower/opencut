use gst::prelude::*;
use gstreamer as gst;
use std::time::Duration;
use url::Url;

pub(super) struct AudioPreview {
    pipeline: gst::Element,
}

impl AudioPreview {
    pub(super) fn new(url: &Url) -> Result<Self, String> {
        gst::init().map_err(|error| format!("could not initialize GStreamer: {error}"))?;
        let video_sink = gst::ElementFactory::make("fakesink")
            .build()
            .map_err(|error| format!("could not create audio preview sink: {error}"))?;
        let pipeline = gst::ElementFactory::make("playbin")
            .property("uri", url.as_str())
            .property("video-sink", &video_sink)
            .build()
            .map_err(|error| format!("could not create audio preview: {error}"))?;
        pipeline
            .set_state(gst::State::Paused)
            .map_err(|error| format!("could not prepare audio preview: {error}"))?;
        let _ = pipeline.state(gst::ClockTime::from_seconds(2));
        Ok(Self { pipeline })
    }

    pub(super) fn seek(&self, position: Duration) {
        let nanos = position.as_nanos().min(u64::MAX as u128) as u64;
        let _ = self.pipeline.seek_simple(
            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
            gst::ClockTime::from_nseconds(nanos),
        );
    }

    pub(super) fn position(&self) -> Duration {
        Duration::from_nanos(
            self.pipeline
                .query_position::<gst::ClockTime>()
                .map(|position| position.nseconds())
                .unwrap_or(0),
        )
    }

    pub(super) fn set_playing(&self, playing: bool) {
        let state = if playing {
            gst::State::Playing
        } else {
            gst::State::Paused
        };
        if self.pipeline.current_state() != state {
            let _ = self.pipeline.set_state(state);
        }
    }

    pub(super) fn set_volume(&self, volume: f64) {
        self.pipeline.set_property("volume", volume.clamp(0.0, 1.0));
    }
}

impl Drop for AudioPreview {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}
