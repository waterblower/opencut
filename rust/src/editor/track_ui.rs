use super::*;
use crate::{asset::IconName, editor::timeline_clip::text_clip_component};
use gpui::{Bounds, canvas, fill, point, rgba, size};
use std::sync::Arc;

const CLIP_WAVEFORM_HEIGHT: f32 = 80.0;
const CLIP_WAVEFORM_VISUAL_GAIN: f32 = 2.0;

fn timeline_clip_move_preview(
    timeline: &TimelineSerialization,
    clip_id: Ulid,
    start: TimelineTime,
    invalid_reason: Option<&'static str>,
) -> gpui::AnyElement {
    let name = timeline
        .clip(clip_id)
        .and_then(Clip::media)
        .and_then(|clip| timeline.asset(clip.asset_id))
        .map(|asset| asset.name.clone())
        .unwrap_or_else(|| "Missing media".to_string());
    let left = TIMELINE_PADDING + timeline.seconds(start) as f32 * timeline.view.pixels_per_second;
    let duration = timeline
        .clip(clip_id)
        .map(|clip| clip.frame_length(timeline.settings.frame_rate))
        .unwrap_or(TimelineTime::ZERO);
    let width = (timeline.seconds(duration) as f32 * timeline.view.pixels_per_second).max(4.0);
    let valid = invalid_reason.is_none();
    let feedback_color = if valid { ACCENT } else { ERROR };

    div()
        .id(gpui::SharedString::from(format!(
            "timeline-clip-move-preview-{clip_id}"
        )))
        .absolute()
        .left(px(left))
        .top(px(5.0))
        .w(px(width))
        .h(px(TRACK_HEIGHT - 10.0))
        .overflow_hidden()
        .rounded_md()
        .border_1()
        .border_color(rgb(feedback_color))
        .bg(gpui::rgba(if valid { 0xf0b75e38 } else { 0xff8b8b38 }))
        .cursor(if valid {
            CursorStyle::ClosedHand
        } else {
            CursorStyle::OperationNotAllowed
        })
        .child(
            div()
                .absolute()
                .inset_0()
                .p_2()
                .flex()
                .flex_col()
                .justify_between()
                .text_color(rgb(feedback_color))
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_ellipsis()
                        .child(name),
                )
                .child(
                    div()
                        .font_family("monospace")
                        .text_xs()
                        .text_ellipsis()
                        .child(invalid_reason.unwrap_or("Move")),
                ),
        )
        .into_any_element()
}

impl Editor {
    pub(super) fn track_header(
        &self,
        index: usize,
        track: &Track,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let track_id = track.id;
        div()
            .id(("track-header", index))
            .h(px(TRACK_HEIGHT))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .justify_center()
            .gap_2()
            .px_3()
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(track.name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(track_kind_label(track.kind)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        track_icon_button(("track-lock", index), IconName::Lock, track.locked)
                            .on_click(cx.listener(move |editor, _, _, cx| {
                                editor.toggle_track_lock(track_id);
                                cx.notify();
                            })),
                    )
                    .child(
                        track_icon_button(("track-visible", index), IconName::Eye, track.visible)
                            .on_click(cx.listener(move |editor, _, _, cx| {
                                editor.toggle_track_visibility(track_id);
                                cx.notify();
                            })),
                    )
                    .when(track.kind != TrackKind::Text, |this| {
                        this.child(
                            track_icon_button(
                                ("track-mute", index),
                                if track.muted {
                                    IconName::Mute
                                } else {
                                    IconName::Unmute
                                },
                                track.muted,
                            )
                            .on_click(cx.listener(
                                move |editor, _, _, cx| {
                                    editor.toggle_track_mute(track_id);
                                    cx.notify();
                                },
                            )),
                        )
                    })
                    .child(track_button(("track-up", index), "↑").on_click(cx.listener(
                        move |editor, _, _, cx| {
                            editor.move_track(track_id, -1);
                            cx.notify();
                        },
                    )))
                    .child(
                        track_button(("track-down", index), "↓").on_click(cx.listener(
                            move |editor, _, _, cx| {
                                editor.move_track(track_id, 1);
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        track_icon_button(("track-delete", index), IconName::Trash, false)
                            .on_click(cx.listener(move |editor, _, _, cx| {
                                editor.delete_track(track_id);
                                cx.notify();
                            })),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn track_row(
        &self,
        index: usize,
        track: &Track,
        timeline_width: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let timeline = self
            .timeline
            .as_ref()
            .expect("track rows require an active timeline");
        let track_id = track.id;
        let clips = timeline
            .data
            .clips_on_track(track.id)
            .map(|clip| self.timeline_clip(clip, cx))
            .collect::<Vec<_>>();
        let move_previews = timeline
            .interaction
            .clip_move_drag
            .as_ref()
            .filter(|drag| drag.changed)
            .map(|drag| {
                drag.placements
                    .iter()
                    .filter(|(_, track_id, _)| *track_id == track.id)
                    .map(|(clip_id, _, start)| {
                        timeline_clip_move_preview(
                            &timeline.data,
                            *clip_id,
                            *start,
                            drag.invalid_reason,
                        )
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let drop_preview = (|| {
            let preview = timeline.preview_drop_asset.as_ref()?;
            if preview.track_id != track.id {
                return None;
            }
            preview_drop_asset(preview, &timeline.data)
        })();

        div()
            .id(("track-row", index))
            .relative()
            .w(px(timeline_width))
            .h(px(TRACK_HEIGHT))
            .flex_shrink_0()
            .border_b_1()
            .border_color(rgb(BORDER))
            .bg(rgb(if index.is_multiple_of(2) {
                0x101012
            } else {
                0x0d0d0f
            }))
            .cursor(match timeline.interaction.active_tool {
                TimelineTool::Blade => CursorStyle::Crosshair,
                TimelineTool::Selection => CursorStyle::Arrow,
            })
            .when(track.kind == TrackKind::Text, |this| {
                this.on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                        editor.show_text_track_context_menu(track_id, event, cx);
                    }),
                )
            })
            .children(clips)
            .children(move_previews)
            .when_some(drop_preview, |this, preview| this.child(preview))
            .into_any_element()
    }

    fn timeline_clip(&self, clip: &Clip, cx: &mut Context<Self>) -> gpui::AnyElement {
        match clip {
            Clip::Video(_) => self.video_clip(clip, cx),
            Clip::Audio(_) => self.audio_clip(clip, cx),
            Clip::Text(clip) => {
                let timeline = self
                    .timeline
                    .as_ref()
                    .expect("timeline clips require an active timeline");
                let clip_id = clip.id;
                let moving = timeline
                    .interaction
                    .clip_move_drag
                    .as_ref()
                    .is_some_and(|drag| {
                        drag.changed && drag.items.iter().any(|item| item.clip_id == clip_id)
                    });
                text_clip_component(
                    clip.clone(),
                    timeline.data.settings.frame_rate,
                    timeline.data.view.pixels_per_second,
                    timeline.interaction.selected_clip_ids.contains(&clip_id),
                    moving,
                )
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |editor, event, _, cx| {
                        editor.handle_clip_mouse_down(clip_id, event, cx);
                        cx.notify();
                    }),
                )
                .into_any_element()
            }
        }
    }

    fn video_clip(&self, clip: &Clip, cx: &mut Context<Self>) -> gpui::AnyElement {
        let timeline = self
            .timeline
            .as_ref()
            .expect("timeline clips require an active timeline");
        let media = clip.media().expect("video tracks contain media clips");
        let asset = timeline.data.asset(media.asset_id);
        let name = asset
            .map(|asset| asset.name.clone())
            .unwrap_or_else(|| "Missing media".to_string());

        let waveform = asset.and_then(|asset| self.waveform_cache.get(&asset.path).cloned());
        let source_start = timeline.data.seconds(media.source_in);
        let source_end = timeline.data.seconds(media.source_out);
        let content = div()
            .absolute()
            .inset_0()
            .child(div().absolute().inset_0())
            .when_some(waveform, |this, path| {
                this.child(timeline_clip_waveform(path, source_start, source_end))
            })
            .child(video_timeline_clip_label(name));

        self.timeline_clip_frame(clip, CLIP_BLUE, content.into_any_element(), cx)
    }

    fn audio_clip(&self, clip: &Clip, cx: &mut Context<Self>) -> gpui::AnyElement {
        let timeline = self
            .timeline
            .as_ref()
            .expect("timeline clips require an active timeline");
        let media = clip.media().expect("audio tracks contain media clips");
        let asset = timeline.data.asset(media.asset_id);
        let name = asset
            .map(|asset| asset.name.clone())
            .unwrap_or_else(|| "Missing media".to_string());
        let waveform = asset.and_then(|asset| self.waveform_cache.get(&asset.path).cloned());
        let source_start = timeline.data.seconds(media.source_in);
        let source_end = timeline.data.seconds(media.source_out);
        let detail = if asset.is_some_and(|asset| asset.has_audio) {
            "Audio".to_string()
        } else {
            format!(
                "{}s",
                timeline
                    .data
                    .seconds(clip.frame_length(timeline.data.settings.frame_rate))
                    .round()
            )
        };
        let content = div()
            .absolute()
            .inset_0()
            .when_some(waveform, |this, path| {
                this.child(timeline_clip_waveform(path, source_start, source_end))
            })
            .child(timeline_clip_label(name, detail));

        self.timeline_clip_frame(clip, 0x24656b, content.into_any_element(), cx)
    }

    fn timeline_clip_frame(
        &self,
        clip: &Clip,
        color: u32,
        content: gpui::AnyElement,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let timeline = self
            .timeline
            .as_ref()
            .expect("timeline clips require an active timeline");
        let clip_id = clip.id();
        let selected = timeline.interaction.selected_clip_ids.contains(&clip_id);
        let moving = timeline
            .interaction
            .clip_move_drag
            .as_ref()
            .is_some_and(|drag| {
                drag.changed && drag.items.iter().any(|item| item.clip_id == clip_id)
            });
        let left = TIMELINE_PADDING
            + timeline.data.seconds(clip.timeline_start()) as f32
                * timeline.data.view.pixels_per_second;
        let width = (timeline
            .data
            .seconds(clip.frame_length(timeline.data.settings.frame_rate))
            as f32
            * timeline.data.view.pixels_per_second)
            .max(4.0);

        div()
            .id(gpui::SharedString::from(format!("timeline-clip-{clip_id}")))
            .absolute()
            .left(px(left))
            .top(px(5.0))
            .w(px(width))
            .h(px(TRACK_HEIGHT - 10.0))
            .overflow_hidden()
            .rounded_md()
            .border_1()
            .border_color(rgb(if selected { ACCENT } else { color + 0x101010 }))
            .bg(rgb(color))
            .opacity(if moving { 0.3 } else { 1.0 })
            .cursor(match timeline.interaction.active_tool {
                TimelineTool::Selection => CursorStyle::PointingHand,
                TimelineTool::Blade => CursorStyle::Crosshair,
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                    editor.handle_clip_mouse_down(clip_id, event, cx);
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                    editor.show_timeline_clip_context_menu(clip_id, event, cx);
                }),
            )
            .child(content)
            .into_any_element()
    }
}

fn timeline_clip_waveform(
    waveform: Arc<waveform::WaveformData>,
    source_start: f64,
    source_end: f64,
) -> gpui::AnyElement {
    canvas(
        move |bounds, _, _| {
            let width = f32::from(bounds.size.width).ceil().max(1.0) as usize;
            waveform.columns(source_start, source_end, width)
        },
        move |bounds: Bounds<gpui::Pixels>, columns, window, _| {
            if columns.is_empty() {
                return;
            }
            let width = f32::from(bounds.size.width);
            let height = f32::from(bounds.size.height);
            let column_width = width / columns.len() as f32;
            let center = height / 2.0;
            let amplitude = (height / 2.0 - 1.0).max(0.0);
            for (index, peak) in columns.into_iter().enumerate() {
                let visible_max = (peak.max * CLIP_WAVEFORM_VISUAL_GAIN).clamp(-1.0, 1.0);
                let visible_min = (peak.min * CLIP_WAVEFORM_VISUAL_GAIN).clamp(-1.0, 1.0);
                let top = (center - visible_max * amplitude).clamp(0.0, height);
                let bottom = (center - visible_min * amplitude).clamp(top, height);
                let bar_height = (bottom - top).max(1.0);
                window.paint_quad(fill(
                    Bounds::new(
                        point(
                            bounds.left() + px(index as f32 * column_width),
                            bounds.top() + px(top),
                        ),
                        size(px(column_width.max(1.0)), px(bar_height)),
                    ),
                    rgba(0x69c5cfd1),
                ));
            }
        },
    )
    .absolute()
    .left_0()
    .right_0()
    .bottom_0()
    .h(px(CLIP_WAVEFORM_HEIGHT))
    .into_any_element()
}

fn timeline_clip_label(name: String, detail: String) -> gpui::Div {
    div()
        .absolute()
        .inset_0()
        .p_2()
        .flex()
        .flex_col()
        .justify_between()
        .child(
            div()
                .text_xs()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_ellipsis()
                .child(name),
        )
        .child(
            div()
                .font_family("monospace")
                .text_xs()
                .text_color(rgb(0xc8d8e8))
                .child(detail),
        )
}

fn video_timeline_clip_label(name: String) -> gpui::Div {
    div()
        .absolute()
        .left_0()
        .right_0()
        .bottom_0()
        .p_2()
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_ellipsis()
        .child(name)
}

fn track_button(id: impl Into<gpui::ElementId>, label: &'static str) -> gpui::Stateful<gpui::Div> {
    track_button_base(id).text_xs().child(label)
}

fn track_icon_button(
    id: impl Into<gpui::ElementId>,
    icon: IconName,
    active: bool,
) -> gpui::Stateful<gpui::Div> {
    track_button_base(id).child(
        gpui::svg()
            .path(icon.path())
            .size_4()
            .text_color(rgb(if active { ACCENT } else { MUTED })),
    )
}

fn track_button_base(id: impl Into<gpui::ElementId>) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h_5()
        .min_w(px(24.0))
        .px_1()
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .bg(rgb(SURFACE))
        .cursor(CursorStyle::PointingHand)
        .hover(|style| style.bg(rgb(SURFACE_HOVER)))
}

fn track_kind_label(kind: TrackKind) -> &'static str {
    match kind {
        TrackKind::Video => "V",
        TrackKind::Audio => "A",
        TrackKind::Text => "T",
    }
}

fn preview_drop_asset(
    preview: &PreviewDropAsset,
    timeline: &TimelineSerialization,
) -> Option<gpui::AnyElement> {
    return match &preview.asset {
        AssetBeingDragged::Srt(srt) => {
            let left = TIMELINE_PADDING
                + timeline.seconds(preview.start_time) as f32 * timeline.view.pixels_per_second;
            Some(
                div()
                    .id("explorer-srt-drop-preview")
                    .absolute()
                    .left(px(left))
                    .top(px(5.0))
                    .w(px(160.0))
                    .h(px(TRACK_HEIGHT - 10.0))
                    .overflow_hidden()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(ACCENT))
                    .bg(gpui::rgba(0xf0b75e38))
                    .cursor(CursorStyle::DragCopy)
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .p_2()
                            .flex()
                            .flex_col()
                            .justify_between()
                            .text_color(rgb(ACCENT))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_ellipsis()
                                    .child(srt.name()),
                            )
                            .child(
                                div()
                                    .font_family("monospace")
                                    .text_xs()
                                    .text_ellipsis()
                                    .child("Subtitles"),
                            ),
                    )
                    .into_any_element(),
            )
        }
        AssetBeingDragged::V1(asset) => {
            let kind = match asset.metadata.kind {
                MediaKind::Video => "Video",
                MediaKind::Audio => "Audio",
                MediaKind::Image => return None,
            };
            let left = TIMELINE_PADDING
                + timeline.seconds(preview.start_time) as f32 * timeline.view.pixels_per_second;
            let duration = timeline
                .nearest_time(asset.metadata.duration)
                .max(TimelineTime::ONE_FRAME);
            let width =
                (timeline.seconds(duration) as f32 * timeline.view.pixels_per_second).max(4.0);

            Some(
                div()
                    .id("explorer-media-drop-preview")
                    .absolute()
                    .left(px(left))
                    .top(px(5.0))
                    .w(px(width))
                    .h(px(TRACK_HEIGHT - 10.0))
                    .overflow_hidden()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(ACCENT))
                    .bg(gpui::rgba(0xf0b75e38))
                    .cursor(CursorStyle::DragCopy)
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .p_2()
                            .flex()
                            .flex_col()
                            .justify_between()
                            .text_color(rgb(ACCENT))
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_ellipsis()
                                    .child(asset.name()),
                            )
                            .child(
                                div()
                                    .font_family("monospace")
                                    .text_xs()
                                    .text_ellipsis()
                                    .child(kind),
                            ),
                    )
                    .into_any_element(),
            )
        }
    };
}
