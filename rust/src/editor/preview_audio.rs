use super::*;
use crate::playback_view::{CONTROL_HEIGHT, PlaybackViewProps, playback_view};
use gst::prelude::*;
use gstreamer as gst;
use std::{path::Path, time::Duration};
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
        self.seek_with_accuracy(position, true);
    }

    pub(super) fn seek_with_accuracy(&self, position: Duration, accurate: bool) {
        let nanos = position.as_nanos().min(u64::MAX as u128) as u64;
        let flags = if accurate {
            gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE
        } else {
            gst::SeekFlags::FLUSH
        };
        let _ = self
            .pipeline
            .seek_simple(flags, gst::ClockTime::from_nseconds(nanos));
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

    pub(super) fn duration(&self) -> Duration {
        Duration::from_nanos(
            self.pipeline
                .query_duration::<gst::ClockTime>()
                .map(|duration| duration.nseconds())
                .unwrap_or(0),
        )
    }

    pub(super) fn playing(&self) -> bool {
        self.pipeline.current_state() == gst::State::Playing
    }

    pub(super) fn finished(&self) -> bool {
        let duration = self.duration();
        !duration.is_zero() && self.position().saturating_add(Duration::from_millis(20)) >= duration
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

impl Editor {
    pub(super) fn preview_audio_file(
        &self,
        path: &Path,
        origin_x: f32,
        origin_y: f32,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let surface_height = (height - CONTROL_HEIGHT).max(1.0);
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let content = div()
            .id("editor-audio-file-preview-content")
            .w(px(width))
            .h(px(surface_height))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .overflow_hidden()
            .bg(rgb(0x09090b))
            .child(
                div()
                    .size(px(96.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_full()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(SURFACE))
                    .text_3xl()
                    .text_color(rgb(ACCENT))
                    .child("♪"),
            )
            .child(
                div()
                    .max_w(px((width - 48.0).max(1.0)))
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_ellipsis()
                    .child(file_name),
            )
            .child(div().text_xs().text_color(rgb(MUTED)).child(
                if self.standalone_audio.is_some() {
                    "Audio preview"
                } else {
                    "Loading audio preview…"
                },
            ))
            .into_any_element();
        let (position, duration, paused) = self.standalone_audio.as_ref().map_or(
            (Duration::ZERO, Duration::ZERO, true),
            |audio| {
                (
                    audio.position(),
                    audio.duration(),
                    !audio.playing() || audio.finished(),
                )
            },
        );
        let reported_progress = if duration.is_zero() {
            0.0
        } else {
            (position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
        };
        let progress = self.preview_scrub_fraction.unwrap_or(reported_progress);
        let position = self
            .preview_scrub_fraction
            .map_or(position, |fraction| duration.mul_f64(fraction as f64));
        let has_media = self.standalone_audio.is_some();

        playback_view(
            PlaybackViewProps {
                origin_x,
                origin_y,
                width,
                height,
                has_media,
                can_play: has_media,
                paused,
                scrubbing: self.preview_is_scrubbing,
                progress,
                position,
                duration,
                volume: self.preview_volume,
                muted: self.preview_volume <= f64::EPSILON,
                volume_open: self.preview_volume_open,
                content,
                extra_control: None,
            },
            cx,
        )
    }
}
