use super::*;
use super::{
    clip_render_plan::{RenderRect, resolve_visual_clip_render_plan},
    timeline_video::update_timeline_video_position,
};
use crate::playback_view::{CONTROL_HEIGHT, format_duration};
use crate::video::video;
use gpui::relative;

const TIMELINE_HORIZONTAL_PADDING: f32 = 22.0;
const TIMELINE_VOLUME_TRACK_HEIGHT: f32 = 144.0;
const TIMELINE_VOLUME_TRACK_BOTTOM_OFFSET: f32 = 102.0;
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
    position_x: f64,
    position_y: f64,
    canvas: TimelinePreviewCanvas,
    snap_x: Option<f64>,
    snap_y: Option<f64>,
    timeline_was_dirty: bool,
    last_pipeline_update: Option<Instant>,
    changed: bool,
}

fn nearest_canvas_snap(clip_anchors: [f64; 3], canvas_guides: [f64; 3]) -> Option<(f64, f64)> {
    let mut nearest = None;
    for clip_anchor in clip_anchors {
        for canvas_guide in canvas_guides {
            let delta = canvas_guide - clip_anchor;
            if delta.abs() <= f64::from(SNAP_DISTANCE_PX)
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
    timeline: &Timeline,
    clip: &TimelineClip,
    properties: VideoClipProperties,
    canvas: TimelinePreviewCanvas,
) -> Option<RenderRect> {
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
        cx: &mut Context<Self>,
    ) {
        self.preview.volume_open = false;
        let pointer_x = f32::from(event.position.x) - surface_left;
        let pointer_y = f32::from(event.position.y) - surface_top;
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let clip_id = timeline.data.tracks.iter().rev().find_map(|track| {
            timeline.data.clips_on_track(track.id).find_map(|clip| {
                if clip.timeline_start > timeline.playhead
                    || timeline.playhead >= clip.timeline_end()
                {
                    return None;
                }
                let rect = timeline_preview_clip_rect(
                    &timeline.data,
                    clip,
                    clip.video_properties,
                    canvas,
                )?;
                (f64::from(pointer_x) >= rect.left
                    && f64::from(pointer_x) <= rect.left + rect.width
                    && f64::from(pointer_y) >= rect.top
                    && f64::from(pointer_y) <= rect.top + rect.height)
                    .then_some(clip.id)
            })
        });
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
        self.preview.timeline_drag = Some(TimelinePreviewDrag {
            clip_id,
            pointer_x: f32::from(event.position.x),
            pointer_y: f32::from(event.position.y),
            position_x: clip.video_properties.position_x,
            position_y: clip.video_properties.position_y,
            canvas,
            snap_x: None,
            snap_y: None,
            timeline_was_dirty: self.preview.timeline_needs_rebuild,
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
        let position_x = drag.position_x
            + f64::from(f32::from(event.position.x) - drag.pointer_x) / drag.canvas.project_scale;
        let position_y = drag.position_y
            + f64::from(f32::from(event.position.y) - drag.pointer_y) / drag.canvas.project_scale;
        let Some(timeline) = self.timeline.as_ref() else {
            return true;
        };
        let Some(index) = timeline.data.clip_index(drag.clip_id) else {
            return true;
        };
        let current_properties = timeline.data.clips[index].video_properties;
        let mut properties = current_properties;
        properties.position_x = position_x;
        properties.position_y = position_y;
        let snap_rect = self
            .timeline
            .as_ref()
            .expect("timeline preview drag requires an active timeline")
            .interaction
            .snapping_enabled
            .then(|| {
                timeline_preview_clip_rect(
                    &timeline.data,
                    &timeline.data.clips[index],
                    properties,
                    drag.canvas,
                )
            })
            .flatten();
        if let Some(rect) = snap_rect {
            let horizontal_snap = nearest_canvas_snap(
                [
                    rect.left + rect.width * 0.5,
                    rect.left,
                    rect.left + rect.width,
                ],
                [
                    drag.canvas.left + drag.canvas.width * 0.5,
                    drag.canvas.left,
                    drag.canvas.left + drag.canvas.width,
                ],
            );
            let vertical_snap = nearest_canvas_snap(
                [
                    rect.top + rect.height * 0.5,
                    rect.top,
                    rect.top + rect.height,
                ],
                [
                    drag.canvas.top + drag.canvas.height * 0.5,
                    drag.canvas.top,
                    drag.canvas.top + drag.canvas.height,
                ],
            );
            drag.snap_x = horizontal_snap.map(|(_, guide)| guide);
            drag.snap_y = vertical_snap.map(|(_, guide)| guide);
            if let Some((delta, _)) = horizontal_snap {
                properties.position_x += delta / drag.canvas.project_scale;
            }
            if let Some((delta, _)) = vertical_snap {
                properties.position_y += delta / drag.canvas.project_scale;
            }
        } else {
            drag.snap_x = None;
            drag.snap_y = None;
        }
        if (current_properties.position_x - properties.position_x).abs() <= f64::EPSILON
            && (current_properties.position_y - properties.position_y).abs() <= f64::EPSILON
        {
            self.preview.timeline_drag = Some(drag);
            self.preview.refresh_ticks = 2;
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
        timeline.data.clips[index].video_properties.position_x = properties.position_x;
        timeline.data.clips[index].video_properties.position_y = properties.position_y;
        self.properties.transform_input_clip_id = None;
        let now = Instant::now();
        if !drag.timeline_was_dirty
            && drag.last_pipeline_update.is_none_or(|last_update| {
                now.duration_since(last_update) >= TIMELINE_TRANSFORM_UPDATE_INTERVAL
            })
        {
            drag.last_pipeline_update = Some(now);
            if let Some(video) = &self.preview.video
                && let Err(error) =
                    update_timeline_video_position(video, &timeline.data, drag.clip_id, false)
            {
                eprintln!("{error}");
            }
        }
        self.preview.timeline_drag = Some(drag);
        self.preview.refresh_ticks = 2;
        cx.notify();
        true
    }

    fn finish_timeline_preview_clip_drag(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(drag) = self.preview.timeline_drag.take() else {
            return false;
        };
        if drag.changed {
            if !drag.timeline_was_dirty
                && let Some(video) = &self.preview.video
            {
                let Some(timeline) = self.timeline.as_ref() else {
                    return true;
                };
                match update_timeline_video_position(video, &timeline.data, drag.clip_id, true) {
                    Ok(()) => self.preview.timeline_needs_rebuild = false,
                    Err(error) => eprintln!("{error}"),
                }
            }
            let Some(timeline) = self.timeline.as_ref() else {
                return true;
            };
            timeline.save(&self.global_settings.project_root);
            self.rebuild_timeline_preview_if_needed();
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
        if self.preview.volume_open {
            self.preview.volume_open = false;
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
            self.preview.volume_open = !self.preview.volume_open;
            cx.notify();
        }
    }

    pub(super) fn preview_timeline(
        &self,
        origin_x: f32,
        origin_y: f32,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let Some(timeline) = self.timeline.as_ref() else {
            return div().size_full().bg(rgb(0x000000)).into_any_element();
        };
        let surface_height = (height - CONTROL_HEIGHT).max(1.0);
        let project_width = timeline.data.settings.width.max(1) as f64;
        let project_height = timeline.data.settings.height.max(1) as f64;
        let project_scale = (f64::from(width.max(1.0)) / project_width)
            .min(f64::from(surface_height) / project_height);
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
            if clip.timeline_start > timeline.playhead || timeline.playhead >= clip.timeline_end() {
                return None;
            }
            timeline_preview_clip_rect(&timeline.data, clip, clip.video_properties, canvas)
        });
        let (snap_x, snap_y) = self
            .preview
            .timeline_drag
            .as_ref()
            .map_or((None, None), |drag| (drag.snap_x, drag.snap_y));
        let usable_width = (width - TIMELINE_HORIZONTAL_PADDING * 2.0).max(1.0);
        let timeline_left = origin_x + TIMELINE_HORIZONTAL_PADDING;
        let volume_track_bottom = origin_y + height - TIMELINE_VOLUME_TRACK_BOTTOM_OFFSET;
        let has_media = !timeline.data.clips.is_empty();
        let duration = timeline.data.duration(timeline.data.timeline_duration());
        let reported_position = timeline.data.duration(timeline.playhead);
        let reported_progress = if duration.is_zero() {
            0.0
        } else {
            (reported_position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
        };
        let progress = self
            .preview
            .scrub_fraction
            .unwrap_or(reported_progress)
            .clamp(0.0, 1.0);
        let position = self
            .preview
            .scrub_fraction
            .map_or(reported_position, |fraction| {
                duration.mul_f64(fraction as f64)
            });
        let volume = self.preview.volume.clamp(0.0, 1.0);
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
                cx.listener(Self::dismiss_timeline_preview_volume),
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
                    .when(has_media, |this| {
                        this.cursor(CursorStyle::OpenHand).on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |editor, event, _, cx| {
                                editor.begin_timeline_preview_clip_drag(
                                    event,
                                    origin_x,
                                    origin_y,
                                    canvas,
                                    cx,
                                );
                            }),
                        )
                    })
                    .child(if let Some(video_handle) = self.preview.video.as_ref() {
                        video(video_handle.clone())
                            .id("editor-timeline-video")
                            .size(px(width), px(surface_height))
                            .into_any_element()
                    } else {
                        div()
                            .size_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_color(rgb(MUTED))
                            .child("Choose a video from the project folder to begin")
                            .into_any_element()
                    })
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
                    }),
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
                                            .child(if self.preview.playing { "Ⅱ" } else { "▶" })
                                            .on_click(
                                                cx.listener(Self::toggle_timeline_preview_playback),
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
                                            .when(self.preview.volume_open && has_media, |this| {
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
                                                            cx.listener(Self::stop_timeline_preview_event_propagation),
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
                                                        cx.listener(Self::stop_timeline_preview_event_propagation),
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
                                                        cx.listener(Self::toggle_timeline_preview_volume),
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
                                                cx.listener(Self::playback_toggle_fullscreen),
                                            ),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}

#[cfg(test)]
#[path = "preview_timeline.test.rs"]
mod tests;
