use crate::asset::IconName;
use crate::edit_action::EditAction;
use crate::editor::Editor;
use crate::event_bus::AppEvent;
use crate::explorer_drag::AssetBeingDragged;
use crate::layout::{TIMELINE_PADDING, TRACK_HEIGHT};
use crate::model::MediaKind;
use crate::srt::srt_text_clips;
use crate::theme::{ACCENT, BORDER, CLIP_BLUE, ERROR, MUTED, SURFACE, SURFACE_HOVER, TEXT};
use crate::timeline::{PreviewDropAsset, TimelineFrameIndex};
use crate::timeline_clip::{Clip, text_clip_component};
use crate::timeline_document::resolve_asset_path;
use crate::timeline_interactions::TimelineTool;
use crate::track::{Track, TrackKind};
use crate::waveform;
use ::timeline::TimelineEditingState;
use gpui::prelude::*;
use gpui::{
    Bounds, CursorStyle, ElementId, MouseButton, MouseDownEvent, PathBuilder, SharedString, canvas,
    div, fill, point, px, rgb, rgba, size,
};
use std::sync::Arc;
use ulid::Ulid;

const CLIP_WAVEFORM_HEIGHT: f32 = 80.0;
const CLIP_WAVEFORM_VISUAL_GAIN: f32 = 2.0;

impl Editor {
    pub(super) fn track_header(
        &self,
        index: usize,
        track: &Track,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let timeline = self
            .timeline
            .as_ref()
            .expect("track headers require an active timeline");
        let track_id = track.id;
        let is_first_track = index == 0;
        let is_last_track = index + 1 == timeline.editing_state.tracks.len();
        let hidden = !track.visible;
        let badge_color = match track.kind {
            TrackKind::Video => CLIP_BLUE,
            TrackKind::Audio => 0x24656b,
            TrackKind::Text => 0x7251a3,
        };
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
                    .gap_2()
                    .child(
                        div()
                            .size(px(18.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded(px(5.0))
                            .border_1()
                            .border_color(rgb(badge_color + 0x101010))
                            .bg(rgb(badge_color))
                            .text_size(px(10.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child(track_kind_label(track.kind)),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_ellipsis()
                            .text_size(px(13.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(track.name.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .gap(px(2.0))
                            .child(
                                track_action_view(
                                    ("track-up", index),
                                    IconName::ArrowUp,
                                    !is_first_track,
                                    TEXT,
                                )
                                .on_click(cx.listener(
                                    move |editor, _, _, cx| {
                                        if let Err(error) = editor.move_track(track_id, -1) {
                                            log::error!("{error:?}");
                                        }
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                track_action_view(
                                    ("track-down", index),
                                    IconName::ArrowDown,
                                    !is_last_track,
                                    TEXT,
                                )
                                .on_click(cx.listener(
                                    move |editor, _, _, cx| {
                                        if let Err(error) = editor.move_track(track_id, 1) {
                                            log::error!("{error:?}");
                                        }
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(
                                track_action_view(
                                    ("track-delete", index),
                                    IconName::Trash,
                                    true,
                                    ERROR,
                                )
                                .on_click(cx.listener(
                                    move |editor, _, _, cx| {
                                        if let Err(error) = editor.delete_track(track_id) {
                                            log::error!("{error:?}");
                                        }
                                        cx.notify();
                                    },
                                )),
                            ),
                    ),
            )
            .child(
                div().flex().child(
                    div()
                        .h(px(24.0))
                        .p(px(1.0))
                        .flex()
                        .items_center()
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .bg(rgb(SURFACE))
                        .child(
                            track_toggle_view(("track-lock", index), IconName::Lock, track.locked)
                                .on_click(cx.listener(move |editor, _, _, cx| {
                                    editor.emit_event(
                                        cx,
                                        AppEvent::Edit(EditAction::ToggleTrackLock { track_id }),
                                    );
                                })),
                        )
                        .child(track_toggle_divider_view(!track.locked && !hidden))
                        .child(
                            track_toggle_view(
                                ("track-visible", index),
                                if hidden {
                                    IconName::EyeOff
                                } else {
                                    IconName::Eye
                                },
                                hidden,
                            )
                            .on_click(cx.listener(
                                move |editor, _, _, cx| {
                                    if let Err(error) = editor.toggle_track_visibility(track_id) {
                                        log::error!("{error:?}");
                                    }
                                    cx.notify();
                                },
                            )),
                        )
                        .when(track.kind != TrackKind::Text, |this| {
                            this.child(track_toggle_divider_view(!hidden && !track.muted))
                                .child(
                                    track_toggle_view(
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
                                            if let Err(error) = editor.toggle_track_mute(track_id) {
                                                log::error!("{error:?}");
                                            }
                                            cx.notify();
                                        },
                                    )),
                                )
                        }),
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
        let viewport_width = f32::from(timeline.h_scroll.bounds().size.width);
        let visible_left = -f32::from(timeline.h_scroll.offset().x) - viewport_width;
        let visible_right = visible_left + viewport_width * 3.0; // 两侧各预留一屏，减少滚动时元素进出视口的抖动。
        let clips = timeline
            .editing_state
            .clips_on_track(track.id)
            .filter(|clip| {
                if viewport_width <= 0.0 {
                    return true; // 首次布局前尚无视口尺寸。
                }
                let left = TIMELINE_PADDING
                    + timeline.editing_state.seconds(clip.timeline_start()) as f32
                        * timeline.pixels_per_second;
                let width = (timeline
                    .editing_state
                    .seconds(clip.frame_length(timeline.editing_state.settings.frame_rate))
                    as f32
                    * timeline.pixels_per_second)
                    .max(4.0);
                left <= visible_right && left + width >= visible_left
            })
            .map(|clip| self.timeline_clip(clip, cx))
            .collect::<Vec<_>>();
        let move_previews =
            timeline
                .interaction
                .clip_move_drag
                .as_ref()
                .filter(|drag| drag.changed)
                .map(|drag| {
                    drag.placements
                        .iter()
                        .filter(|(_, track_id, _)| *track_id == track.id)
                        .filter(|(clip_id, _, start)| {
                            if viewport_width <= 0.0 {
                                return true;
                            }
                            let Some(clip) = timeline.editing_state.clip(*clip_id) else {
                                return false;
                            };
                            let left = TIMELINE_PADDING
                                + timeline.editing_state.seconds(*start) as f32
                                    * timeline.pixels_per_second;
                            let width = (timeline.editing_state.seconds(
                                clip.frame_length(timeline.editing_state.settings.frame_rate),
                            ) as f32
                                * timeline.pixels_per_second)
                                .max(4.0);
                            left <= visible_right && left + width >= visible_left
                        })
                        .map(|(clip_id, _, start)| {
                            let clip = timeline
                                .editing_state
                                .clip(*clip_id)
                                .expect("moving clip exists");
                            let (name, kind) = match clip {
                                Clip::Text(text) => (
                                    SharedString::from(text.properties.text.clone()),
                                    PlacementKind::Text,
                                ),
                                Clip::Video(_) | Clip::Audio(_) => {
                                    let name = match clip
                                        .asset_id()
                                        .and_then(|id| timeline.editing_state.asset(id))
                                    {
                                        Some(asset) => SharedString::from(asset.name.clone()),
                                        None => SharedString::from("Missing media"),
                                    };
                                    (
                                        name,
                                        if matches!(clip, Clip::Audio(_)) {
                                            PlacementKind::Media(MediaKind::Audio)
                                        } else {
                                            PlacementKind::Media(MediaKind::Video)
                                        },
                                    )
                                }
                            };
                            let left = TIMELINE_PADDING
                                + timeline.editing_state.seconds(*start) as f32
                                    * timeline.pixels_per_second;
                            let width = (timeline.editing_state.seconds(
                                clip.frame_length(timeline.editing_state.settings.frame_rate),
                            ) as f32
                                * timeline.pixels_per_second)
                                .max(4.0);
                            clip_placement_preview_view(
                                SharedString::from(format!("clip-placement-{clip_id}")),
                                name,
                                kind,
                                left,
                                width,
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
            preview_drop_asset(preview, &timeline.editing_state, timeline.pixels_per_second)
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
                let clip_id = clip.id();
                let moving = timeline
                    .interaction
                    .clip_move_drag
                    .as_ref()
                    .is_some_and(|drag| {
                        drag.changed && drag.items.iter().any(|item| item.clip_id == clip_id)
                    });
                text_clip_component(
                    clip.clone(),
                    timeline.editing_state.settings.frame_rate,
                    timeline.pixels_per_second,
                    timeline.interaction.selected_clip_ids.contains(&clip_id),
                    moving,
                )
                .children(self.clip_trim_handles(clip_id, cx))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |editor, event, _, cx| {
                        if let Err(error) = editor.handle_clip_mouse_down(clip_id, event, cx) {
                            log::error!("Could not handle clip mouse down: {error:?}");
                        }
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
        let media = clip.video().expect("video tracks contain video clips");
        let asset = timeline.editing_state.asset(media.asset_id);
        let name = asset
            .map(|asset| asset.name.clone())
            .unwrap_or_else(|| "Missing media".to_string());

        let waveform = (|| {
            let asset = asset?;
            let timeline_directory = timeline.path.parent()?;
            let media_path = resolve_asset_path(timeline_directory, &asset.path); // 波形缓存以素材的绝对路径为键。
            self.waveform_cache.get(&media_path).cloned()
        })();
        let source_start = timeline.editing_state.seconds(media.source_in);
        let source_end = timeline.editing_state.seconds(media.source_out);
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
        let media = clip.audio().expect("audio tracks contain audio clips");
        let asset = timeline.editing_state.asset(media.asset_id);
        let name = asset
            .map(|asset| asset.name.clone())
            .unwrap_or_else(|| "Missing media".to_string());
        let waveform = (|| {
            let asset = asset?;
            let timeline_directory = timeline.path.parent()?;
            let media_path = resolve_asset_path(timeline_directory, &asset.path); // 波形缓存以素材的绝对路径为键。
            self.waveform_cache.get(&media_path).cloned()
        })();
        let source_start = timeline.editing_state.seconds(media.source_in);
        let source_end = timeline.editing_state.seconds(media.source_out);
        let detail = if asset.is_some_and(|asset| asset.has_audio) {
            "Audio".to_string()
        } else {
            format!(
                "{}s",
                timeline
                    .editing_state
                    .seconds(clip.frame_length(timeline.editing_state.settings.frame_rate))
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
            + timeline.editing_state.seconds(clip.timeline_start()) as f32
                * timeline.pixels_per_second;
        let width = (timeline
            .editing_state
            .seconds(clip.frame_length(timeline.editing_state.settings.frame_rate))
            as f32
            * timeline.pixels_per_second)
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
                    if let Err(error) = editor.handle_clip_mouse_down(clip_id, event, cx) {
                        log::error!("Could not handle clip mouse down: {error:?}");
                    }
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
            .children(self.clip_trim_handles(clip_id, cx))
            .into_any_element()
    }
}

fn timeline_clip_waveform(
    waveform: Arc<waveform::WaveformData>,
    source_start: f64,
    source_end: f64,
) -> gpui::AnyElement {
    canvas(
        move |bounds, window, _| {
            let visible_bounds = bounds.intersect(&window.content_mask().bounds);
            if visible_bounds.is_empty() {
                return (visible_bounds, Vec::new());
            }

            let width = f32::from(bounds.size.width).max(1.0);
            let visible_start =
                (f32::from(visible_bounds.left() - bounds.left()) / width).clamp(0.0, 1.0);
            let visible_end =
                (f32::from(visible_bounds.right() - bounds.left()) / width).clamp(0.0, 1.0);
            let source_duration = source_end - source_start;
            let visible_source_start = source_start + source_duration * f64::from(visible_start);
            let visible_source_end = source_start + source_duration * f64::from(visible_end);
            let visible_width = f32::from(visible_bounds.size.width).ceil().max(1.0) as usize;

            (
                visible_bounds,
                waveform.columns(visible_source_start, visible_source_end, visible_width),
            )
        },
        move |bounds: Bounds<gpui::Pixels>, (visible_bounds, columns), window, _| {
            if columns.is_empty() {
                return;
            }
            let width = f32::from(visible_bounds.size.width);
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
                            visible_bounds.left() + px(index as f32 * column_width),
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

fn track_action_view(
    id: impl Into<gpui::ElementId>,
    icon: IconName,
    enabled: bool,
    hover_color: u32,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .group("track-action")
        .size(px(20.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .when(enabled, |button| {
            button
                .cursor(CursorStyle::PointingHand)
                .hover(|style| style.bg(rgb(SURFACE_HOVER)))
        })
        .child(
            gpui::svg()
                .path(icon.path())
                .size(px(14.0))
                .text_color(rgb(if enabled { MUTED } else { 0x3a3a40 }))
                .when(enabled, |svg| {
                    svg.group_hover("track-action", |style| style.text_color(rgb(hover_color)))
                }),
        )
}

fn track_toggle_view(
    id: impl Into<gpui::ElementId>,
    icon: IconName,
    active: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .group("track-toggle")
        .w(px(28.0))
        .h(px(20.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .cursor(CursorStyle::PointingHand)
        .when(active, |toggle| {
            toggle
                .bg(rgba(0xf0b75e29))
                .hover(|style| style.bg(rgba(0xf0b75e42)))
        })
        .when(!active, |toggle| {
            toggle.hover(|style| style.bg(rgb(SURFACE_HOVER)))
        })
        .child(
            gpui::svg()
                .path(icon.path())
                .size(px(14.0))
                .text_color(rgb(if active { ACCENT } else { MUTED }))
                .when(!active, |svg| {
                    svg.group_hover("track-toggle", |style| style.text_color(rgb(TEXT)))
                }),
        )
}

fn track_toggle_divider_view(visible: bool) -> gpui::Div {
    div()
        .w(px(1.0))
        .h(px(12.0))
        .when(visible, |divider| divider.bg(rgb(BORDER)))
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
    timeline: &TimelineEditingState,
    pixels_per_second: f32,
) -> Option<gpui::AnyElement> {
    let invalid_reason = match preview.validate_placement(timeline) {
        Ok(()) => None,
        Err(error) => Some(error.to_string()),
    };
    match &preview.asset {
        AssetBeingDragged::None => None,
        AssetBeingDragged::Srt(srt) => {
            let clips = srt_text_clips(&srt.srt, timeline.settings.frame_rate);
            let previews = clips
                .into_iter()
                .enumerate()
                .map(|(index, clip)| {
                    let left = TIMELINE_PADDING
                        + timeline.seconds(clip.timeline_start + preview.start_time) as f32
                            * pixels_per_second;
                    let width = (timeline.seconds(clip.frame_length(timeline.settings.frame_rate))
                        as f32
                        * pixels_per_second)
                        .max(4.0);
                    clip_placement_preview_view(
                        ("srt-placement", index),
                        SharedString::from(clip.properties.text),
                        PlacementKind::Text,
                        left,
                        width,
                        invalid_reason.as_deref(),
                    )
                })
                .collect::<Vec<_>>();
            Some(
                div()
                    .absolute()
                    .inset_0()
                    .children(previews)
                    .into_any_element(),
            )
        }
        AssetBeingDragged::V1(asset) => {
            let left =
                TIMELINE_PADDING + timeline.seconds(preview.start_time) as f32 * pixels_per_second;
            let duration = timeline
                .nearest_time(asset.metadata.duration)
                .max(TimelineFrameIndex::ONE_FRAME);
            let width = (timeline.seconds(duration) as f32 * pixels_per_second).max(4.0);
            Some(clip_placement_preview_view(
                "asset-placement",
                asset.name(),
                PlacementKind::Media(asset.metadata.kind),
                left,
                width,
                invalid_reason.as_deref(),
            ))
        }
    }
}

enum PlacementKind {
    Media(MediaKind),
    Text,
}

// Private APIs only
impl PlacementKind {
    fn label(&self) -> &'static str {
        match self {
            Self::Media(MediaKind::Video) => "Video",
            Self::Media(MediaKind::Audio) => "Audio",
            Self::Media(MediaKind::Image) => "Image",
            Self::Text => "Text",
        }
    }
}

/// Both asset drops and clip moves use the destination's placement feedback.
fn clip_placement_preview_view(
    id: impl Into<ElementId>,
    name: SharedString,
    kind: PlacementKind,
    left: f32,
    width: f32,
    invalid_reason: Option<&str>,
) -> gpui::AnyElement {
    let valid = invalid_reason.is_none();
    let feedback_color = if valid { ACCENT } else { ERROR };
    let detail = match invalid_reason {
        Some(reason) => reason.to_string(),
        None => format!("Release to place · {}", kind.label()),
    };
    div()
        .id(id)
        .absolute()
        .left(px(left))
        .top(px(5.0))
        .w(px(width))
        .h(px(TRACK_HEIGHT - 10.0))
        .overflow_hidden()
        .rounded_md()
        .border_1()
        .border_color(rgb(feedback_color))
        .bg(rgba(if valid { 0xf0b75e24 } else { 0x321b20e6 }))
        .cursor(if valid {
            CursorStyle::ClosedHand
        } else {
            CursorStyle::OperationNotAllowed
        })
        .when(!valid, |view| {
            view.child(
                canvas(
                    |_, _, _| {},
                    |bounds, (), window, _| {
                        let height = f32::from(bounds.size.height);
                        let width = f32::from(bounds.size.width);
                        let mut hatch = PathBuilder::stroke(px(1.0));
                        // 只绘制可见区域的斜纹，长素材不会按整段时长生成路径。
                        let mask = window.content_mask();
                        let visible_start = f32::from(mask.bounds.left() - bounds.left()).max(0.0);
                        let visible_end = f32::from(mask.bounds.right() - bounds.left()).min(width);
                        if visible_end <= visible_start {
                            return;
                        }
                        let mut x = ((visible_start - height) / 8.0).floor() * 8.0;
                        while x < visible_end {
                            let from_x = x.max(0.0);
                            let from_y = height - (from_x - x);
                            let to_x = (x + height).min(width);
                            let to_y = height - (to_x - x);
                            hatch.move_to(point(
                                bounds.left() + px(from_x),
                                bounds.top() + px(from_y),
                            ));
                            hatch.line_to(point(bounds.left() + px(to_x), bounds.top() + px(to_y)));
                            x += 8.0;
                        }
                        match hatch.build() {
                            Ok(path) => window.paint_path(path, rgba(0xff8b8b38)),
                            Err(error) => {
                                log::error!("Could not draw placement feedback: {error:?}")
                            }
                        }
                    },
                )
                .absolute()
                .inset_0()
                .size_full(),
            )
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
                .child(div().text_xs().text_ellipsis().child(detail)),
        )
        .into_any_element()
}
