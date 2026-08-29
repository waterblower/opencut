use super::*;
use gst::prelude::*;
use gstreamer as gst;
use std::{path::Path, time::Duration};
use url::Url;

const AUDIO_CONTROL_HEIGHT: f32 = 96.0;
const AUDIO_HORIZONTAL_PADDING: f32 = 22.0;
const AUDIO_VOLUME_WIDTH: f32 = 96.0;

pub(super) struct AudioBackend {
    pipeline: gst::Element,
}

impl AudioBackend {
    pub(super) fn new(url: &Url) -> anyhow::Result<Self> {
        gst::init().map_err(|error| anyhow::anyhow!("could not initialize GStreamer: {error}"))?;
        let video_sink = gst::ElementFactory::make("fakesink")
            .build()
            .map_err(|error| anyhow::anyhow!("could not create audio preview sink: {error}"))?;
        let pipeline = gst::ElementFactory::make("playbin")
            .property("uri", url.as_str())
            .property("video-sink", &video_sink)
            .build()
            .map_err(|error| anyhow::anyhow!("could not create audio preview: {error}"))?;
        pipeline
            .set_state(gst::State::Paused)
            .map_err(|error| anyhow::anyhow!("could not prepare audio preview: {error}"))?;
        let _ = pipeline.state(gst::ClockTime::from_seconds(2));
        Ok(Self { pipeline })
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

    pub fn volume(&self) -> f64 {
        self.pipeline.property::<f64>("volume")
    }
}

fn seek_audio_to_fraction(audio: &AudioBackend, fraction: f32, accurate: bool) {
    let target = audio.duration().mul_f64(fraction.clamp(0.0, 1.0) as f64);
    audio.seek_with_accuracy(target, accurate);
}

impl Drop for AudioBackend {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

impl Editor {
    pub(super) fn preview_audio_file(
        &self,
        path: &Path,
        origin_x: f32,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let surface_height = (height - AUDIO_CONTROL_HEIGHT).max(1.0);
        let file_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let (position, duration, paused) =
            self.preview
                .target
                .audio()
                .map_or((Duration::ZERO, Duration::ZERO, true), |audio| {
                    (
                        audio.position(),
                        audio.duration(),
                        !audio.playing() || audio.finished(),
                    )
                });

        let progress = if duration.is_zero() {
            0.0
        } else {
            (position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
        };
        let has_media = self.preview.target.audio().is_some();
        let usable_width = (width - AUDIO_HORIZONTAL_PADDING * 2.0).max(1.0);
        let timeline_left = origin_x + AUDIO_HORIZONTAL_PADDING;
        let volume_left = origin_x + width - AUDIO_HORIZONTAL_PADDING - AUDIO_VOLUME_WIDTH;
        let volume = self
            .preview
            .target
            .audio()
            .map_or(0.0, |a| a.volume().clamp(0.0, 1.0)) as f32;
        let format_time = |duration: Duration| {
            let total_seconds = duration.as_secs();
            let hours = total_seconds / 3600;
            let minutes = (total_seconds % 3600) / 60;
            let seconds = total_seconds % 60;
            if hours > 0 {
                format!("{hours}:{minutes:02}:{seconds:02}")
            } else {
                format!("{minutes}:{seconds:02}")
            }
        };
        let time = format!("{} / {}", format_time(position), format_time(duration));

        div()
            .id("editor-audio-file-preview")
            .relative()
            .w(px(width))
            .h(px(height))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgb(0x000000))
            .on_mouse_move(cx.listener(move |editor, event: &MouseMoveEvent, _, cx| {
                if !event.dragging() {
                    return;
                }
                if editor.preview.is_scrubbing {
                    let fraction = ((f32::from(event.position.x) - timeline_left) / usable_width)
                        .clamp(0.0, 1.0);
                    editor.update_audio_scrub(fraction, cx);
                }
                if editor.preview.is_adjusting_volume {
                    let volume = ((f32::from(event.position.x) - volume_left) / AUDIO_VOLUME_WIDTH)
                        .clamp(0.0, 1.0) as f64;
                    editor.set_audio_preview_volume(volume, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(move |editor, event: &MouseUpEvent, _, cx| {
                    if editor.preview.is_scrubbing {
                        let fraction = ((f32::from(event.position.x) - timeline_left)
                            / usable_width)
                            .clamp(0.0, 1.0);
                        editor.finish_audio_scrub(fraction, cx);
                    }
                    if editor.preview.is_adjusting_volume {
                        let volume = ((f32::from(event.position.x) - volume_left)
                            / AUDIO_VOLUME_WIDTH)
                            .clamp(0.0, 1.0) as f64;
                        editor.preview.is_adjusting_volume = false;
                        editor.set_audio_preview_volume(volume, cx);
                    }
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(move |editor, event: &MouseUpEvent, _, cx| {
                    if editor.preview.is_scrubbing {
                        let fraction = ((f32::from(event.position.x) - timeline_left)
                            / usable_width)
                            .clamp(0.0, 1.0);
                        editor.finish_audio_scrub(fraction, cx);
                    }
                    if editor.preview.is_adjusting_volume {
                        let volume = ((f32::from(event.position.x) - volume_left)
                            / AUDIO_VOLUME_WIDTH)
                            .clamp(0.0, 1.0) as f64;
                        editor.preview.is_adjusting_volume = false;
                        editor.set_audio_preview_volume(volume, cx);
                    }
                }),
            )
            .child(
                div()
                    .id("editor-audio-file-preview-content")
                    .w_full()
                    .h(px(surface_height))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap_4()
                    .overflow_hidden()
                    .bg(rgb(0x09090b))
                    .when(has_media, |this| {
                        this.cursor(CursorStyle::PointingHand).on_click(cx.listener(
                            |editor, _, _, cx| {
                                editor.toggle_playback();
                                cx.notify();
                            },
                        ))
                    })
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
                    .child(div().text_xs().text_color(rgb(MUTED)).child(if has_media {
                        "Audio preview"
                    } else {
                        "Loading audio preview…"
                    })),
            )
            .child(
                div()
                    .h(px(AUDIO_CONTROL_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap_2()
                    .px(px(AUDIO_HORIZONTAL_PADDING))
                    .border_t_1()
                    .border_b_1()
                    .border_color(rgb(0x19191c))
                    .bg(rgb(0x0b0b0d))
                    .child(
                        div()
                            .id("editor-audio-preview-timeline")
                            .relative()
                            .h_4()
                            .flex()
                            .items_center()
                            .when(has_media, |this| {
                                this.cursor(CursorStyle::PointingHand).on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                                        let fraction = ((f32::from(event.position.x)
                                            - timeline_left)
                                            / usable_width)
                                            .clamp(0.0, 1.0);
                                        editor.begin_audio_scrub(fraction, cx);
                                    }),
                                )
                            })
                            .child(
                                div()
                                    .w_full()
                                    .h(px(3.0))
                                    .rounded_full()
                                    .bg(rgb(0x4a4a4f))
                                    .child(
                                        div()
                                            .w(gpui::relative(progress))
                                            .h_full()
                                            .flex()
                                            .items_center()
                                            .justify_end()
                                            .rounded_full()
                                            .bg(rgb(ACCENT))
                                            .child(
                                                div()
                                                    .size(px(if self.preview.is_scrubbing {
                                                        16.0
                                                    } else {
                                                        12.0
                                                    }))
                                                    .flex_shrink_0()
                                                    .rounded_full()
                                                    .bg(rgb(ACCENT)),
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .h_10()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .id("editor-audio-play-pause")
                                            .size(px(36.0))
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .rounded_full()
                                            .text_lg()
                                            .text_color(if has_media {
                                                rgb(TEXT)
                                            } else {
                                                rgb(MUTED)
                                            })
                                            .child(if paused { "▶" } else { "Ⅱ" })
                                            .when(has_media, |this| {
                                                this.cursor(CursorStyle::PointingHand)
                                                    .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                                                    .on_click(cx.listener(|editor, _, _, cx| {
                                                        editor.toggle_playback();
                                                        cx.notify();
                                                    }))
                                            }),
                                    )
                                    .child(div().text_sm().font_family("monospace").child(time)),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .text_xs()
                                    .text_color(rgb(MUTED))
                                    .child("Volume")
                                    .child(
                                        div()
                                            .id("editor-audio-volume")
                                            .relative()
                                            .w(px(AUDIO_VOLUME_WIDTH))
                                            .h_4()
                                            .flex()
                                            .items_center()
                                            .when(has_media, |this| {
                                                this.cursor(CursorStyle::PointingHand)
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            move |editor,
                                                                  event: &MouseDownEvent,
                                                                  _,
                                                                  cx| {
                                                                editor.preview.is_adjusting_volume =
                                                                    true;
                                                                let volume = ((f32::from(
                                                                    event.position.x,
                                                                ) - volume_left)
                                                                    / AUDIO_VOLUME_WIDTH)
                                                                    .clamp(0.0, 1.0)
                                                                    as f64;
                                                                editor.set_audio_preview_volume(
                                                                    volume, cx,
                                                                );
                                                            },
                                                        ),
                                                    )
                                            })
                                            .child(
                                                div()
                                                    .w_full()
                                                    .h(px(3.0))
                                                    .rounded_full()
                                                    .bg(rgb(0x4a4a4f))
                                                    .child(
                                                        div()
                                                            .w(gpui::relative(volume))
                                                            .h_full()
                                                            .rounded_full()
                                                            .bg(rgb(TEXT)),
                                                    ),
                                            ),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn begin_audio_scrub(&mut self, fraction: f32, cx: &mut Context<Self>) {
        let Some(audio) = self.preview.target.audio() else {
            return;
        };
        audio.set_playing(false);
        self.preview.is_scrubbing = true;
        self.preview.last_scrub_seek = Some(Instant::now());
        seek_audio_to_fraction(audio, fraction, false);

        cx.notify();
    }

    fn update_audio_scrub(&mut self, fraction: f32, cx: &mut Context<Self>) {
        if !self.preview.is_scrubbing {
            return;
        }
        let now = Instant::now();
        if self
            .preview
            .last_scrub_seek
            .is_none_or(|last_seek| now.duration_since(last_seek) >= SCRUB_SEEK_INTERVAL)
        {
            self.preview.last_scrub_seek = Some(now);
            if let Some(audio) = self.preview.target.audio() {
                seek_audio_to_fraction(audio, fraction, false);
            }
        }

        cx.notify();
    }

    fn finish_audio_scrub(&mut self, fraction: f32, cx: &mut Context<Self>) {
        self.preview.last_scrub_seek = None;
        self.preview.is_scrubbing = false;
        if let Some(audio) = self.preview.target.audio() {
            seek_audio_to_fraction(audio, fraction, true);
        }

        cx.notify();
    }

    fn set_audio_preview_volume(&self, volume: f64, cx: &mut Context<Self>) {
        let Some(audio) = self.preview.target.audio() else {
            return;
        };
        audio.set_volume(volume.clamp(0.0, 1.0));
        cx.notify();
    }
}
