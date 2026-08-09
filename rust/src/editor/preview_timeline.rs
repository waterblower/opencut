use super::*;
use crate::playback_view::{CONTROL_HEIGHT, format_duration};
use crate::video::video;
use gpui::relative;

const TIMELINE_HORIZONTAL_PADDING: f32 = 22.0;
const TIMELINE_VOLUME_TRACK_HEIGHT: f32 = 144.0;
const TIMELINE_VOLUME_TRACK_BOTTOM_OFFSET: f32 = 102.0;

impl Editor {
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

    fn select_timeline_preview_clip(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let clip_id = self.project.tracks.iter().rev().find_map(|track| {
            if track.kind != TrackKind::Video || !track.visible {
                return None;
            }
            self.project
                .clips_on_track(track.id)
                .find(|clip| {
                    clip.timeline_start <= self.timeline.playhead
                        && self.timeline.playhead < clip.timeline_end()
                })
                .map(|clip| clip.id)
        });
        self.select_only_clip(clip_id);
        cx.notify();
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
        if !self.project.clips.is_empty() {
            self.preview.volume_open = !self.preview.volume_open;
            cx.notify();
        }
    }

    fn toggle_timeline_preview_fullscreen(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.toggle_fullscreen();
        cx.notify();
    }

    pub(super) fn preview_timeline(
        &self,
        origin_x: f32,
        origin_y: f32,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let surface_height = (height - CONTROL_HEIGHT).max(1.0);
        let usable_width = (width - TIMELINE_HORIZONTAL_PADDING * 2.0).max(1.0);
        let timeline_left = origin_x + TIMELINE_HORIZONTAL_PADDING;
        let volume_track_bottom = origin_y + height - TIMELINE_VOLUME_TRACK_BOTTOM_OFFSET;
        let has_media = !self.project.clips.is_empty();
        let duration = self.project.duration(self.project.timeline_duration());
        let reported_position = self.project.duration(self.timeline.playhead);
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
                    .h(px(surface_height))
                    .w_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .bg(rgb(0x000000))
                    .when(has_media, |this| {
                        this.cursor(CursorStyle::PointingHand)
                            .on_click(cx.listener(Self::select_timeline_preview_clip))
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
                                                cx.listener(Self::toggle_timeline_preview_fullscreen),
                                            ),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}
