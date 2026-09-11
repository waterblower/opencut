use super::*;
use super::{
    clip_render_plan::{RenderRect, resolve_visual_clip_render_plan},
    timeline_video::{refresh_timeline_video_frame, try_refresh_timeline_video_frame},
};
use crate::playback_view::{CONTROL_HEIGHT, format_duration};
use crate::video::video;
use gpui::relative;

pub fn preview_timeline_view(
    editor: &Editor,
    origin_x: f32,
    origin_y: f32,
    width: f32,
    height: f32,
    cx: &mut Context<Editor>,
) -> gpui::AnyElement {
    let Some(timeline) = editor.timeline.as_ref() else {
        return div().size_full().bg(rgb(0x000000)).into_any_element();
    };
    let surface_height = (height - CONTROL_HEIGHT).max(1.0);
    let project_width = timeline.data.settings.width.max(1) as f64;
    let project_height = timeline.data.settings.height.max(1) as f64;
    let project_scale =
        (f64::from(width.max(1.0)) / project_width).min(f64::from(surface_height) / project_height);
    let output_width = project_width * project_scale;
    let output_height = project_height * project_scale;
    let output_left = (f64::from(width) - output_width) * 0.5;
    let output_top = (f64::from(surface_height) - output_height) * 0.5;
    let canvas = TimelinePreviewCanvas {
        left: output_left,
        top: output_top,
        width: output_width,
        height: output_height,
        project_scale: project_scale.max(f64::EPSILON),
    };
    let selected_rect = timeline.interaction.selected_clip_id.and_then(|clip_id| {
        let clip = timeline.data.clip(clip_id)?;
        let media = clip.media()?;
        if clip.timeline_start() > timeline.playhead()
            || timeline.playhead() >= clip.timeline_end(timeline.data.settings.frame_rate)
        {
            return None;
        }
        timeline_preview_clip_rect(&timeline.data, clip, media.video_properties, canvas)
    });
    let (snap_x, snap_y) = editor
        .preview
        .timeline_drag
        .as_ref()
        .map_or((None, None), |drag| (drag.snap_x, drag.snap_y));
    let usable_width = (width - TIMELINE_HORIZONTAL_PADDING * 2.0).max(1.0);
    let timeline_left = origin_x + TIMELINE_HORIZONTAL_PADDING;
    let volume_track_bottom = origin_y + height - TIMELINE_VOLUME_TRACK_BOTTOM_OFFSET;
    let has_media = !timeline.data.clips.is_empty();
    let mut clip_cursor_regions = Vec::new();
    for track in &timeline.data.tracks {
        for clip in timeline.data.clips_on_track(track.id) {
            if clip.timeline_start() > timeline.playhead()
                || timeline.playhead() >= clip.timeline_end(timeline.data.settings.frame_rate)
            {
                continue;
            }
            let Some(media) = clip.media() else {
                continue;
            };
            let Some(rect) =
                timeline_preview_clip_rect(&timeline.data, clip, media.video_properties, canvas)
            else {
                continue;
            };
            let left = rect.left.max(canvas.left);
            let top = rect.top.max(canvas.top);
            let right = (rect.left + rect.width).min(canvas.left + canvas.width);
            let bottom = (rect.top + rect.height).min(canvas.top + canvas.height);
            if right <= left || bottom <= top {
                continue;
            }
            clip_cursor_regions.push(
                div()
                    .absolute()
                    .left(px(left as f32))
                    .top(px(top as f32))
                    .w(px((right - left) as f32))
                    .h(px((bottom - top) as f32))
                    .cursor(if track.locked {
                        CursorStyle::Arrow
                    } else {
                        CursorStyle::OpenHand
                    }),
            );
        }
    }

    let mut resize_handles = Vec::new();
    if let Some(rect) = selected_rect
        && let Some(clip_id) = timeline.interaction.selected_clip_id
        && !timeline.data.clip_locked(clip_id)
    {
        for (horizontal, vertical) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
            let x = rect.left + (horizontal + 1.0) * rect.width * 0.5;
            let y = rect.top + (vertical + 1.0) * rect.height * 0.5;
            resize_handles.push(
                div()
                    .absolute()
                    .left(px(x as f32 - 6.0))
                    .top(px(y as f32 - 6.0))
                    .size(px(12.0))
                    .bg(rgb(ACCENT))
                    .border_1()
                    .border_color(rgb(0x101012))
                    .cursor(if horizontal == vertical {
                        CursorStyle::ResizeUpLeftDownRight
                    } else {
                        CursorStyle::ResizeUpRightDownLeft
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |editor, event, _, cx| {
                            editor.begin_timeline_preview_clip_drag(
                                event,
                                origin_x,
                                origin_y,
                                canvas,
                                PreviewDragMode::Resize {
                                    rect,
                                    horizontal,
                                    vertical,
                                },
                                cx,
                            );
                        }),
                    ),
            );
        }
    }

    let duration = timeline.data.duration(timeline.data.content_duration());
    let position = timeline.video_backend.playback().position();
    let progress = if duration.is_zero() {
        0.0
    } else {
        (position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
    };
    let volume = timeline.video_backend.playback().volume().clamp(0.0, 1.0);
    let muted = volume <= f64::EPSILON;
    let displayed_volume = if muted { 0.0 } else { volume } as f32;
    let volume_percent = (displayed_volume * 100.0).round() as u32;
    let volume_fill_height = displayed_volume * TIMELINE_VOLUME_TRACK_HEIGHT;
    let volume_thumb_bottom = displayed_volume * (TIMELINE_VOLUME_TRACK_HEIGHT - 20.0);

    div()
        .id("editor-timeline-preview")
        .relative()
        .w(px(width))
        .h(px(height))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(rgb(0x000000))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(Editor::dismiss_timeline_preview_volume),
        )
        .on_mouse_move(cx.listener(
            move |editor, event: &MouseMoveEvent, window, cx| {
                editor.update_timeline_preview_drag(
                    event,
                    timeline_left,
                    usable_width,
                    volume_track_bottom,
                    window,
                    cx,
                );
            },
        ))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(
                move |editor, event: &MouseUpEvent, window, cx| {
                    editor.finish_timeline_preview_drag(
                        event,
                        timeline_left,
                        usable_width,
                        volume_track_bottom,
                        window,
                        cx,
                    );
                },
            ),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(
                move |editor, event: &MouseUpEvent, window, cx| {
                    editor.finish_timeline_preview_drag(
                        event,
                        timeline_left,
                        usable_width,
                        volume_track_bottom,
                        window,
                        cx,
                    );
                },
            ),
        )
        .child(
            div()
                .id("editor-timeline-preview-surface")
                .relative()
                .h(px(surface_height))
                .w_full()
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .bg(rgb(0x000000))
                .cursor(CursorStyle::Arrow)
                .when(has_media, |this| {
                    this.on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |editor, event, _, cx| {
                            editor.begin_timeline_preview_clip_drag(
                                event,
                                origin_x,
                                origin_y,
                                canvas,
                                PreviewDragMode::Move,
                                cx,
                            );
                        }),
                    )
                })
                .child(
                    video(timeline.video_backend.playback())
                        .id("editor-timeline-video")
                        .size(px(width), px(surface_height))
                        .into_any_element(),
                )
                .when_some(snap_x, |this, guide| {
                    this.child(
                        div()
                            .absolute()
                            .left(px((guide - 0.5) as f32))
                            .top(px(canvas.top as f32))
                            .w(px(1.0))
                            .h(px(canvas.height as f32))
                            .bg(rgb(ACCENT)),
                    )
                })
                .when_some(snap_y, |this, guide| {
                    this.child(
                        div()
                            .absolute()
                            .left(px(canvas.left as f32))
                            .top(px((guide - 0.5) as f32))
                            .w(px(canvas.width as f32))
                            .h(px(1.0))
                            .bg(rgb(ACCENT)),
                    )
                })
                .when_some(selected_rect, |this, rect| {
                    this.child(
                        div()
                            .id("editor-timeline-preview-selection")
                            .absolute()
                            .left(px(rect.left as f32))
                            .top(px(rect.top as f32))
                            .w(px(rect.width.max(1.0) as f32))
                            .h(px(rect.height.max(1.0) as f32))
                            .border_1()
                            .border_color(rgb(ACCENT)),
                    )
                })
                .children(clip_cursor_regions)
                .children(resize_handles),
        )
        .child(
            div()
                .relative()
                .h(px(CONTROL_HEIGHT))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .justify_center()
                .gap_3()
                .px(px(TIMELINE_HORIZONTAL_PADDING))
                .border_t_1()
                .border_b_1()
                .border_color(rgb(0x19191c))
                .bg(rgb(0x0b0b0d))
                .when(has_media, |this| {
                    this.child(
                        div()
                            .id("editor-timeline-preview-scrubber")
                            .relative()
                            .h_4()
                            .flex()
                            .items_center()
                            .cursor(CursorStyle::PointingHand)
                            .child(
                                div()
                                    .w_full()
                                    .h(px(3.0))
                                    .rounded_full()
                                    .bg(rgb(0x4a4a4f))
                                    .child(
                                        div()
                                            .w(relative(progress))
                                            .h_full()
                                            .flex()
                                            .items_center()
                                            .justify_end()
                                            .rounded_full()
                                            .bg(rgb(ACCENT))
                                            .child(
                                                div()
                                                    .size(px(if editor.preview.is_scrubbing {
                                                        16.0
                                                    } else {
                                                        12.0
                                                    }))
                                                    .flex_shrink_0()
                                                    .rounded_full()
                                                    .bg(rgb(ACCENT)),
                                            ),
                                    ),
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(
                                    move |editor, event: &MouseDownEvent, window, cx| {
                                        editor.begin_timeline_preview_scrub(
                                            event,
                                            timeline_left,
                                            usable_width,
                                            window,
                                            cx,
                                        );
                                    },
                                ),
                            ),
                    )
                })
                .child(
                    div()
                        .h_12()
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
                                        .id("editor-timeline-play-pause")
                                        .w_9()
                                        .h_9()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor(CursorStyle::PointingHand)
                                        .rounded_full()
                                        .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                                        .text_lg()
                                        .text_color(if has_media {
                                            rgb(TEXT)
                                        } else {
                                            rgb(MUTED)
                                        })
                                        .child(if !timeline.video_backend.playback().paused() { "Ⅱ" } else { "▶" })
                                        .on_click(
                                            cx.listener(Editor::toggle_timeline_preview_playback),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_family("monospace")
                                        .child(format!(
                                            "{} / {}",
                                            format_duration(position),
                                            format_duration(duration)
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .id("editor-timeline-volume-control")
                                        .relative()
                                        .w(px(72.0))
                                        .h_12()
                                        .flex_shrink_0()
                                        .when(editor.preview.volume_control_open && has_media, |this| {
                                            this.child(
                                                div()
                                                    .absolute()
                                                    .left_0()
                                                    .bottom(px(58.0))
                                                    .w(px(72.0))
                                                    .h(px(232.0))
                                                    .flex()
                                                    .flex_col()
                                                    .items_center()
                                                    .rounded(px(22.0))
                                                    .border_1()
                                                    .border_color(rgb(0x35353b))
                                                    .bg(rgb(0x1a1a1d))
                                                    .shadow_lg()
                                                    .occlude()
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(Editor::stop_timeline_preview_event_propagation),
                                                    )
                                                    .on_mouse_move(cx.listener(
                                                        move |editor,
                                                              event: &MouseMoveEvent,
                                                              window,
                                                              cx| {
                                                            editor.update_timeline_preview_volume(
                                                                event,
                                                                volume_track_bottom,
                                                                window,
                                                                cx,
                                                            );
                                                        },
                                                    ))
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            move |editor,
                                                                  event: &MouseUpEvent,
                                                                  window,
                                                                  cx| {
                                                                editor.finish_timeline_preview_volume(
                                                                    event,
                                                                    volume_track_bottom,
                                                                    window,
                                                                    cx,
                                                                );
                                                            },
                                                        ),
                                                    )
                                                    .child(
                                                        div()
                                                            .absolute()
                                                            .top(px(18.0))
                                                            .font_family("monospace")
                                                            .text_lg()
                                                            .text_color(rgb(MUTED))
                                                            .child(volume_percent.to_string()),
                                                    )
                                                    .child(
                                                        div()
                                                            .id("editor-timeline-volume-track")
                                                            .absolute()
                                                            .top(px(64.0))
                                                            .w_6()
                                                            .h(px(
                                                                TIMELINE_VOLUME_TRACK_HEIGHT,
                                                            ))
                                                            .flex()
                                                            .justify_center()
                                                            .cursor(CursorStyle::PointingHand)
                                                            .child(
                                                                div()
                                                                    .w(px(5.0))
                                                                    .h_full()
                                                                    .rounded_full()
                                                                    .bg(rgb(0x55555b)),
                                                            )
                                                            .child(
                                                                div()
                                                                    .absolute()
                                                                    .bottom_0()
                                                                    .w(px(5.0))
                                                                    .h(px(volume_fill_height))
                                                                    .rounded_full()
                                                                    .bg(rgb(0xdedee2)),
                                                            )
                                                            .child(
                                                                div()
                                                                    .absolute()
                                                                    .left(px(2.0))
                                                                    .bottom(px(
                                                                        volume_thumb_bottom,
                                                                    ))
                                                                    .size(px(20.0))
                                                                    .rounded_full()
                                                                    .bg(rgb(0xffffff)),
                                                            )
                                                            .on_mouse_down(
                                                                MouseButton::Left,
                                                                cx.listener(
                                                                    move |editor,
                                                                          event: &MouseDownEvent,
                                                                          window,
                                                                          cx| {
                                                                        editor.begin_timeline_preview_volume(
                                                                            event,
                                                                            volume_track_bottom,
                                                                            window,
                                                                            cx,
                                                                        );
                                                                    },
                                                                ),
                                                            ),
                                                    ),
                                            )
                                        })
                                        .child(
                                            div()
                                                .id("editor-timeline-volume-toggle")
                                                .absolute()
                                                .left(px(12.0))
                                                .bottom_0()
                                                .size(px(48.0))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .cursor(CursorStyle::PointingHand)
                                                .rounded_xl()
                                                .border_1()
                                                .border_color(rgb(BORDER))
                                                .bg(rgb(0x1a1a1d))
                                                .hover(|style| {
                                                    style.bg(rgb(SURFACE_HOVER))
                                                })
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(Editor::stop_timeline_preview_event_propagation),
                                                )
                                                .child(
                                                    div()
                                                        .h(px(28.0))
                                                        .flex()
                                                        .items_end()
                                                        .gap_1()
                                                        .children(
                                                            [10.0_f32, 18.0, 28.0]
                                                                .into_iter()
                                                                .map(|height| {
                                                                    div()
                                                                        .w(px(5.0))
                                                                        .h(px(height))
                                                                        .rounded_full()
                                                                        .bg(if muted {
                                                                            rgb(MUTED)
                                                                        } else {
                                                                            rgb(TEXT)
                                                                        })
                                                                }),
                                                        ),
                                                )
                                                .on_click(
                                                    cx.listener(Editor::toggle_timeline_preview_volume),
                                                ),
                                        ),
                                )
                                .child(
                                    div()
                                        .id("editor-timeline-fullscreen")
                                        .cursor(CursorStyle::PointingHand)
                                        .rounded_md()
                                        .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                                        .px_3()
                                        .py_2()
                                        .text_lg()
                                        .child("⛶")
                                        .on_click(
                                            cx.listener(Editor::playback_toggle_fullscreen),
                                        ),
                                ),
                        ),
                ),
        )
        .into_any_element()
}

const TIMELINE_HORIZONTAL_PADDING: f32 = 22.0;
const TIMELINE_VOLUME_TRACK_HEIGHT: f32 = 144.0;
const TIMELINE_VOLUME_TRACK_BOTTOM_OFFSET: f32 = 102.0;
const PREVIEW_SNAP_DISTANCE_PX: f64 = 4.0;
const TIMELINE_TRANSFORM_UPDATE_INTERVAL: Duration = Duration::from_millis(16);

#[derive(Clone, Copy, Debug)]
struct TimelinePreviewCanvas {
    left: f64,
    top: f64,
    width: f64,
    height: f64,
    project_scale: f64,
}

pub(super) struct TimelinePreviewDrag {
    clip_id: Ulid,
    pointer_x: f32,
    pointer_y: f32,
    properties: VideoClipProperties,
    mode: PreviewDragMode,
    canvas: TimelinePreviewCanvas,
    snap_x: Option<f64>,
    snap_y: Option<f64>,
    last_pipeline_update: Option<Instant>,
    changed: bool,
}

#[derive(Clone, Copy)]
enum PreviewDragMode {
    Move,
    Resize {
        rect: RenderRect,
        horizontal: f64,
        vertical: f64,
    },
}

fn resized_preview_properties(
    original: VideoClipProperties,
    rect: RenderRect,
    corner: (f64, f64),
    delta: (f64, f64),
    project_scale: f64,
) -> VideoClipProperties {
    let diagonal_squared = rect.width * rect.width + rect.height * rect.height;
    if diagonal_squared <= f64::EPSILON || original.scale <= f64::EPSILON {
        return original;
    }
    let factor = 1.0
        + (delta.0 * corner.0 * rect.width + delta.1 * corner.1 * rect.height) / diagonal_squared;
    let scale = (original.scale * factor).clamp(0.01, 100.0);
    let factor = scale / original.scale;
    VideoClipProperties {
        position_x: original.position_x
            + corner.0 * rect.width * (factor - 1.0) * 0.5 / project_scale,
        position_y: original.position_y
            + corner.1 * rect.height * (factor - 1.0) * 0.5 / project_scale,
        scale,
    }
}

fn snap_preview_resize(
    properties: VideoClipProperties,
    rect: RenderRect,
    corner: (f64, f64),
    project_scale: f64,
    horizontal_guides: &[f64],
    vertical_guides: &[f64],
) -> (VideoClipProperties, Option<f64>, Option<f64>) {
    let fixed_x = rect.left + (1.0 - corner.0) * rect.width * 0.5;
    let fixed_y = rect.top + (1.0 - corner.1) * rect.height * 0.5;
    let mut best_factor: Option<f64> = None;
    for (fixed, span, guides) in [
        (fixed_x, corner.0 * rect.width, horizontal_guides),
        (fixed_y, corner.1 * rect.height, vertical_guides),
    ] {
        if span.abs() <= f64::EPSILON {
            continue;
        }
        // The opposite edge is stationary; only the moving edge and center
        // can constrain the scale without moving the anchored corner.
        for fraction in [1.0, 0.5] {
            let anchor = fixed + span * fraction;
            for &guide in guides {
                if (guide - anchor).abs() > PREVIEW_SNAP_DISTANCE_PX {
                    continue;
                }
                let factor = (guide - fixed) / (span * fraction);
                if !(0.01..=100.0).contains(&(properties.scale * factor)) {
                    continue;
                }
                if best_factor.is_none_or(|best| (factor - 1.0).abs() < (best - 1.0).abs()) {
                    best_factor = Some(factor);
                }
            }
        }
    }
    let Some(factor) = best_factor else {
        return (properties, None, None);
    };
    let snapped = VideoClipProperties {
        position_x: properties.position_x
            + corner.0 * rect.width * (factor - 1.0) * 0.5 / project_scale,
        position_y: properties.position_y
            + corner.1 * rect.height * (factor - 1.0) * 0.5 / project_scale,
        scale: properties.scale * factor,
    };
    let mut snap_x = None;
    let mut snap_y = None;
    for fraction in [1.0, 0.5] {
        for &guide in horizontal_guides {
            if (fixed_x + corner.0 * rect.width * factor * fraction - guide).abs() < 0.000001 {
                snap_x = Some(guide);
            }
        }
        for &guide in vertical_guides {
            if (fixed_y + corner.1 * rect.height * factor * fraction - guide).abs() < 0.000001 {
                snap_y = Some(guide);
            }
        }
    }
    (snapped, snap_x, snap_y)
}

fn nearest_canvas_snap(clip_anchors: [f64; 3], canvas_guides: &[f64]) -> Option<(f64, f64)> {
    let mut nearest = None;
    for clip_anchor in clip_anchors {
        for &canvas_guide in canvas_guides {
            let delta = canvas_guide - clip_anchor;
            if delta.abs() <= PREVIEW_SNAP_DISTANCE_PX
                && nearest
                    .is_none_or(|(nearest_delta, _): (f64, f64)| delta.abs() < nearest_delta.abs())
            {
                nearest = Some((delta, canvas_guide));
            }
        }
    }
    nearest
}

fn timeline_preview_clip_rect(
    timeline: &TimelineSerialization,
    clip: &Clip,
    properties: VideoClipProperties,
    canvas: TimelinePreviewCanvas,
) -> Option<RenderRect> {
    let clip = clip.media()?;
    let track = timeline.track(clip.track_id)?;
    if track.kind != TrackKind::Video || !track.visible {
        return None;
    }
    let asset = timeline.asset(clip.asset_id)?;
    if asset.kind == MediaKind::Audio {
        return None;
    }
    let visible = resolve_visual_clip_render_plan(
        properties,
        asset.width,
        asset.height,
        timeline.settings.width,
        timeline.settings.height,
        canvas.width,
        canvas.height,
    )
    .visible;
    Some(RenderRect {
        left: canvas.left + visible.left,
        top: canvas.top + visible.top,
        width: visible.width,
        height: visible.height,
    })
}

impl Editor {
    fn begin_timeline_preview_clip_drag(
        &mut self,
        event: &MouseDownEvent,
        surface_left: f32,
        surface_top: f32,
        canvas: TimelinePreviewCanvas,
        mode: PreviewDragMode,
        cx: &mut Context<Self>,
    ) {
        self.preview.volume_control_open = false;
        let pointer_x = f32::from(event.position.x) - surface_left;
        let pointer_y = f32::from(event.position.y) - surface_top;
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let clip_id = if matches!(mode, PreviewDragMode::Resize { .. }) {
            timeline.interaction.selected_clip_id
        } else {
            timeline.data.tracks.iter().rev().find_map(|track| {
                timeline.data.clips_on_track(track.id).find_map(|clip| {
                    let media = clip.media()?;
                    if clip.timeline_start() > timeline.playhead()
                        || timeline.playhead()
                            >= clip.timeline_end(timeline.data.settings.frame_rate)
                    {
                        return None;
                    }
                    let rect = timeline_preview_clip_rect(
                        &timeline.data,
                        clip,
                        media.video_properties,
                        canvas,
                    )?;
                    (f64::from(pointer_x) >= rect.left
                        && f64::from(pointer_x) <= rect.left + rect.width
                        && f64::from(pointer_y) >= rect.top
                        && f64::from(pointer_y) <= rect.top + rect.height)
                        .then_some(clip.id())
                })
            })
        };
        self.select_only_clip(clip_id);
        let Some(clip_id) = clip_id else {
            self.preview.timeline_drag = None;
            cx.notify();
            cx.stop_propagation();
            return;
        };
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        if timeline.data.clip_locked(clip_id) {
            self.preview.timeline_drag = None;
            cx.notify();
            cx.stop_propagation();
            return;
        }
        let Some(clip) = timeline.data.clip(clip_id) else {
            return;
        };
        let Some(clip) = clip.media() else {
            return;
        };
        self.preview.timeline_drag = Some(TimelinePreviewDrag {
            clip_id,
            pointer_x: f32::from(event.position.x),
            pointer_y: f32::from(event.position.y),
            properties: clip.video_properties,
            mode,
            canvas,
            snap_x: None,
            snap_y: None,

            last_pipeline_update: None,
            changed: false,
        });
        cx.notify();
        cx.stop_propagation();
    }

    fn update_timeline_preview_clip_drag(
        &mut self,
        event: &MouseMoveEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(mut drag) = self.preview.timeline_drag.take() else {
            return false;
        };
        if !event.dragging() {
            self.preview.timeline_drag = Some(drag);
            return true;
        }
        let delta = (
            f64::from(f32::from(event.position.x) - drag.pointer_x),
            f64::from(f32::from(event.position.y) - drag.pointer_y),
        );
        let Some(timeline) = self.timeline.as_ref() else {
            return true;
        };
        let Some(index) = timeline.data.clip_index(drag.clip_id) else {
            return true;
        };
        let Some(current_clip) = timeline.data.clips[index].media() else {
            return true;
        };
        let current_properties = current_clip.video_properties;
        let mut properties = match drag.mode {
            PreviewDragMode::Move => VideoClipProperties {
                position_x: drag.properties.position_x + delta.0 / drag.canvas.project_scale,
                position_y: drag.properties.position_y + delta.1 / drag.canvas.project_scale,
                ..drag.properties
            },
            PreviewDragMode::Resize {
                rect,
                horizontal,
                vertical,
            } => resized_preview_properties(
                drag.properties,
                rect,
                (horizontal, vertical),
                delta,
                drag.canvas.project_scale,
            ),
        };
        drag.snap_x = None;
        drag.snap_y = None;
        if timeline.interaction.snapping_enabled {
            let mut horizontal_guides = vec![
                drag.canvas.left,
                drag.canvas.left + drag.canvas.width * 0.5,
                drag.canvas.left + drag.canvas.width,
            ];
            let mut vertical_guides = vec![
                drag.canvas.top,
                drag.canvas.top + drag.canvas.height * 0.5,
                drag.canvas.top + drag.canvas.height,
            ];
            for clip in &timeline.data.clips {
                if clip.id() == drag.clip_id
                    || clip.timeline_start() > timeline.playhead()
                    || timeline.playhead() >= clip.timeline_end(timeline.data.settings.frame_rate)
                {
                    continue;
                }
                let Some(media) = clip.media() else {
                    continue;
                };
                let Some(other) = timeline_preview_clip_rect(
                    &timeline.data,
                    clip,
                    media.video_properties,
                    drag.canvas,
                ) else {
                    continue;
                };
                horizontal_guides.extend([
                    other.left,
                    other.left + other.width * 0.5,
                    other.left + other.width,
                ]);
                vertical_guides.extend([
                    other.top,
                    other.top + other.height * 0.5,
                    other.top + other.height,
                ]);
            }
            if let Some(rect) = timeline_preview_clip_rect(
                &timeline.data,
                &timeline.data.clips[index],
                properties,
                drag.canvas,
            ) {
                match drag.mode {
                    PreviewDragMode::Move => {
                        if let Some((delta, guide)) = nearest_canvas_snap(
                            [
                                rect.left + rect.width * 0.5,
                                rect.left,
                                rect.left + rect.width,
                            ],
                            &horizontal_guides,
                        ) {
                            properties.position_x += delta / drag.canvas.project_scale;
                            drag.snap_x = Some(guide);
                        }
                        if let Some((delta, guide)) = nearest_canvas_snap(
                            [
                                rect.top + rect.height * 0.5,
                                rect.top,
                                rect.top + rect.height,
                            ],
                            &vertical_guides,
                        ) {
                            properties.position_y += delta / drag.canvas.project_scale;
                            drag.snap_y = Some(guide);
                        }
                    }
                    PreviewDragMode::Resize {
                        horizontal,
                        vertical,
                        ..
                    } => {
                        (properties, drag.snap_x, drag.snap_y) = snap_preview_resize(
                            properties,
                            rect,
                            (horizontal, vertical),
                            drag.canvas.project_scale,
                            &horizontal_guides,
                            &vertical_guides,
                        );
                    }
                }
            }
        }
        if (current_properties.position_x - properties.position_x).abs() <= f64::EPSILON
            && (current_properties.position_y - properties.position_y).abs() <= f64::EPSILON
            && (current_properties.scale - properties.scale).abs() <= f64::EPSILON
        {
            self.preview.timeline_drag = Some(drag);

            cx.notify();
            return true;
        }
        let Some(timeline) = self.timeline.as_mut() else {
            return true;
        };
        if !drag.changed {
            timeline.record_editing_history();
            drag.changed = true;
        }
        edit_and_rebuild_timeline(
            &mut self.preview,
            &self.global_settings.project_root,
            timeline,
            EditAction::SetVideoProperties {
                clip_ids: vec![drag.clip_id],
                properties,
            },
        )
        .expect("setting video properties cannot be rejected");
        self.properties.transform_input_clip_id = None;
        let now = Instant::now();
        if drag.last_pipeline_update.is_none_or(|last_update| {
            now.duration_since(last_update) >= TIMELINE_TRANSFORM_UPDATE_INTERVAL
        }) {
            drag.last_pipeline_update = Some(now);
            if let Err(error) =
                try_refresh_timeline_video_frame(timeline.video_backend.playback_mut())
            {
                eprintln!("{error}");
            }
        }
        self.preview.timeline_drag = Some(drag);

        cx.notify();
        true
    }

    fn finish_timeline_preview_clip_drag(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(drag) = self.preview.timeline_drag.take() else {
            return false;
        };
        if drag.changed {
            if let Some(timeline) = self.timeline.as_mut() {
                match refresh_timeline_video_frame(timeline.video_backend.playback_mut()) {
                    Ok(()) => {}
                    Err(error) => eprintln!("{error}"),
                }
            }
            let Some(timeline) = self.timeline.as_ref() else {
                return true;
            };
            timeline.save(&self.global_settings.project_root);
        }
        cx.notify();
        true
    }

    fn dismiss_timeline_preview_volume(
        &mut self,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.preview.volume_control_open {
            self.preview.volume_control_open = false;
            cx.notify();
        }
    }

    fn update_timeline_preview_drag(
        &mut self,
        event: &MouseMoveEvent,
        timeline_left: f32,
        usable_width: f32,
        volume_track_bottom: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !event.dragging() {
            return;
        }
        if self.update_timeline_preview_clip_drag(event, cx) {
            return;
        }
        self.playback_seek(
            ((f32::from(event.position.x) - timeline_left) / usable_width).clamp(0.0, 1.0),
            DragPhase::Update,
            window,
            cx,
        );
        self.playback_set_volume(
            ((volume_track_bottom - f32::from(event.position.y)) / TIMELINE_VOLUME_TRACK_HEIGHT)
                .clamp(0.0, 1.0) as f64,
            DragPhase::Update,
            window,
            cx,
        );
    }

    fn finish_timeline_preview_drag(
        &mut self,
        event: &MouseUpEvent,
        timeline_left: f32,
        usable_width: f32,
        volume_track_bottom: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.finish_timeline_preview_clip_drag(cx) {
            return;
        }
        self.playback_seek(
            ((f32::from(event.position.x) - timeline_left) / usable_width).clamp(0.0, 1.0),
            DragPhase::End,
            window,
            cx,
        );
        self.playback_set_volume(
            ((volume_track_bottom - f32::from(event.position.y)) / TIMELINE_VOLUME_TRACK_HEIGHT)
                .clamp(0.0, 1.0) as f64,
            DragPhase::End,
            window,
            cx,
        );
    }

    fn begin_timeline_preview_scrub(
        &mut self,
        event: &MouseDownEvent,
        timeline_left: f32,
        usable_width: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.playback_seek(
            ((f32::from(event.position.x) - timeline_left) / usable_width).clamp(0.0, 1.0),
            DragPhase::Start,
            window,
            cx,
        );
    }

    fn toggle_timeline_preview_playback(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_playback();
        cx.notify();
    }

    fn stop_timeline_preview_event_propagation(
        &mut self,
        _: &MouseDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
    }

    fn update_timeline_preview_volume(
        &mut self,
        event: &MouseMoveEvent,
        volume_track_bottom: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.dragging() {
            self.playback_set_volume(
                ((volume_track_bottom - f32::from(event.position.y)) / TIMELINE_VOLUME_TRACK_HEIGHT)
                    .clamp(0.0, 1.0) as f64,
                DragPhase::Update,
                window,
                cx,
            );
        }
        cx.stop_propagation();
    }

    fn finish_timeline_preview_volume(
        &mut self,
        event: &MouseUpEvent,
        volume_track_bottom: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.playback_set_volume(
            ((volume_track_bottom - f32::from(event.position.y)) / TIMELINE_VOLUME_TRACK_HEIGHT)
                .clamp(0.0, 1.0) as f64,
            DragPhase::End,
            window,
            cx,
        );
        cx.stop_propagation();
    }

    fn begin_timeline_preview_volume(
        &mut self,
        event: &MouseDownEvent,
        volume_track_bottom: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.playback_set_volume(
            ((volume_track_bottom - f32::from(event.position.y)) / TIMELINE_VOLUME_TRACK_HEIGHT)
                .clamp(0.0, 1.0) as f64,
            DragPhase::Start,
            window,
            cx,
        );
    }

    fn toggle_timeline_preview_volume(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .timeline
            .as_ref()
            .is_some_and(|timeline| !timeline.data.clips.is_empty())
        {
            self.preview.volume_control_open = !self.preview.volume_control_open;
            cx.notify();
        }
    }
}

#[cfg(test)]
#[path = "tests/preview_timeline.test.rs"]
mod tests;
