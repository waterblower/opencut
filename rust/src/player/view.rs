use super::*;
use crate::playback_view::{PlaybackViewProps, playback_view};
use crate::video::video;

impl Render for Player {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.record_render_frame();

        let viewport = window.viewport_size();
        let viewport_width: f32 = viewport.width.into();
        let viewport_height: f32 = viewport.height.into();

        if window.is_fullscreen() {
            let fullscreen_content_width = (viewport_width
                - if self.inspector_open {
                    INSPECTOR_WIDTH
                } else {
                    0.0
                })
            .max(1.0);
            let playback_area = if let Some(video_handle) = self.video.as_mut() {
                video(video_handle)
                    .id("fullscreen-video")
                    .size(px(fullscreen_content_width), px(viewport_height))
                    .into_any_element()
            } else {
                div().size_full().bg(rgb(0x000000)).into_any_element()
            };

            return div()
                .id("fullscreen-player-root")
                .track_focus(&self.focus_handle)
                .on_action(cx.listener(Self::action_toggle_playback))
                .on_action(cx.listener(Self::action_seek_backward))
                .on_action(cx.listener(Self::action_seek_forward))
                .on_action(cx.listener(Self::action_toggle_mute))
                .on_action(cx.listener(Self::action_toggle_history))
                .on_action(cx.listener(Self::action_toggle_fullscreen))
                .on_action(cx.listener(Self::action_toggle_inspector))
                .size_full()
                .flex()
                .overflow_hidden()
                .bg(rgb(0x000000))
                .child(
                    div()
                        .id("fullscreen-playback-area")
                        .h_full()
                        .flex_1()
                        .flex()
                        .items_center()
                        .justify_center()
                        .overflow_hidden()
                        .bg(rgb(0x000000))
                        .when(self.video.is_some(), |this| {
                            this.cursor(CursorStyle::PointingHand).on_click(cx.listener(
                                |this, _, _, cx| {
                                    this.toggle_playback();
                                    cx.notify();
                                },
                            ))
                        })
                        .child(playback_area),
                )
                .when(self.inspector_open, |this| {
                    this.child(self.inspector_panel(cx))
                });
        }

        let history_width = if self.history_open {
            self.history_width
        } else {
            0.0
        };
        let content_width = (viewport_width
            - history_width
            - if self.inspector_open {
                INSPECTOR_WIDTH
            } else {
                0.0
            })
        .max(1.0);
        let video_height = (viewport_height - HEADER_HEIGHT - CONTROL_HEIGHT).max(140.0);

        let has_video = self.video.is_some();
        let is_paused = self.video.as_ref().is_none_or(Video::paused);
        let is_muted = self.video.as_ref().is_some_and(Video::muted);
        let volume = self
            .video
            .as_ref()
            .map_or(0.0, |video| video.volume().clamp(0.0, 1.0));
        let reported_position = self.video.as_ref().map_or(Duration::ZERO, Video::position);
        let duration = self.video.as_ref().map_or(Duration::ZERO, Video::duration);

        let reported_progress = if duration.is_zero() {
            0.0
        } else {
            (reported_position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
        };
        let progress = self.scrub_fraction.unwrap_or(reported_progress);
        let position = self.scrub_fraction.map_or(reported_position, |fraction| {
            duration.mul_f64(fraction as f64)
        });
        let display_title = self.display_title();

        let video_content = if let Some(video_handle) = self.video.as_mut() {
            video(video_handle)
                .id("main-video")
                .size(px(content_width), px(video_height))
                .into_any_element()
        } else {
            div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_3()
                .bg(rgb(0x030303))
                .child(
                    div()
                        .text_xl()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("Your screen is ready"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(MUTED))
                        .child("Open a local MP4 to begin playback."),
                )
                .child(
                    div()
                        .id("open-video-empty-state")
                        .cursor(CursorStyle::PointingHand)
                        .rounded_md()
                        .bg(rgb(ACCENT))
                        .text_color(rgb(BACKGROUND))
                        .hover(|style| style.bg(rgb(0xffc974)))
                        .px_5()
                        .py_2()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("Open MP4")
                        .on_click(cx.listener(|this, _, _, cx| this.open_picker(cx))),
                )
                .into_any_element()
        };

        let speed_control = div()
            .id("speed")
            .occlude()
            .cursor(CursorStyle::PointingHand)
            .rounded_md()
            .hover(|style| style.bg(rgb(SURFACE_HOVER)))
            .px_3()
            .py_2()
            .text_sm()
            .text_color(rgb(MUTED))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| cx.stop_propagation()),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.settings_open = !this.settings_open;
                cx.notify();
            }))
            .into_any_element();

        let settings_menu = (self.settings_open && has_video).then(|| {
            div()
                .id("player-settings-menu")
                .absolute()
                .right(px(HORIZONTAL_PADDING))
                .bottom(px(76.0))
                .w(px(270.0))
                .flex()
                .flex_col()
                .gap_2()
                .rounded_xl()
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(0x111113))
                .p_4()
                .shadow_lg()
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_, _, _, cx| cx.stop_propagation()),
                )
                .child(div().text_xs().text_color(rgb(0x65656d)).child("QUALITY"))
                .child(
                    div()
                        .h_10()
                        .flex()
                        .items_center()
                        .justify_between()
                        .px_3()
                        .text_sm()
                        .child("Original file")
                        .child(div().size_2().rounded_full().bg(rgb(ACCENT))),
                )
                .child(div().h_px().bg(rgb(BORDER)))
                .child(
                    div()
                        .mt_2()
                        .text_xs()
                        .text_color(rgb(0x65656d))
                        .child("PLAYBACK SPEED"),
                )
                .child(div().h_px().bg(rgb(BORDER)))
                .child(
                    div()
                        .mt_2()
                        .text_xs()
                        .text_color(rgb(0x65656d))
                        .child("AUDIO"),
                )
                .child(
                    div()
                        .id("settings-audio")
                        .h_10()
                        .flex()
                        .items_center()
                        .justify_between()
                        .cursor(CursorStyle::PointingHand)
                        .rounded_md()
                        .px_3()
                        .text_sm()
                        .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                        .child(if is_muted { "Muted" } else { "Enabled" })
                        .child(div().size_2().rounded_full().bg(rgb(ACCENT)))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.toggle_mute();
                            cx.notify();
                        })),
                )
                .into_any_element()
        });

        let shared_playback = playback_view(
            PlaybackViewProps {
                origin_x: history_width,
                origin_y: HEADER_HEIGHT,
                width: content_width,
                height: video_height + CONTROL_HEIGHT,
                has_media: has_video,
                can_play: has_video,
                paused: is_paused,
                scrubbing: self.is_scrubbing,
                progress,
                position,
                duration,
                volume,
                muted: is_muted,
                volume_open: self.volume_open,
                content: video_content,
                extra_control: Some(speed_control),
                fullscreen_control: div()
                    .id("player-fullscreen")
                    .cursor(CursorStyle::PointingHand)
                    .rounded_md()
                    .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                    .px_3()
                    .py_2()
                    .text_lg()
                    .child("⛶")
                    .on_click(cx.listener(Self::playback_toggle_fullscreen))
                    .into_any_element(),
            },
            cx,
        );

        let history_panel = self.history_open.then(|| self.history_panel(cx));

        div()
            .id("player-root")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::action_toggle_playback))
            .on_action(cx.listener(Self::action_seek_backward))
            .on_action(cx.listener(Self::action_seek_forward))
            .on_action(cx.listener(Self::action_toggle_mute))
            .on_action(cx.listener(Self::action_toggle_history))
            .on_action(cx.listener(Self::action_toggle_fullscreen))
            .on_action(cx.listener(Self::action_toggle_inspector))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::dismiss_settings))
            .on_mouse_move(cx.listener(Self::resize_history))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_history_resize))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_history_resize))
            .size_full()
            .flex()
            .overflow_hidden()
            .bg(rgb(BACKGROUND))
            .text_color(rgb(TEXT))
            .when_some(history_panel, |this, panel| this.child(panel))
            .child(
                div()
                    .id("player-content")
                    .h_full()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(
                        div()
                            .h(px(HEADER_HEIGHT))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(HORIZONTAL_PADDING))
                            .border_b_1()
                            .border_color(rgb(BORDER))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        div()
                                            .id("toggle-history")
                                            .size_9()
                                            .flex_shrink_0()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .cursor(CursorStyle::PointingHand)
                                            .rounded_md()
                                            .border_1()
                                            .border_color(rgb(BORDER))
                                            .text_color(if self.history_open {
                                                rgb(TEXT)
                                            } else {
                                                rgb(MUTED)
                                            })
                                            .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                                            .child("☰")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.history_open = !this.history_open;
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div().min_w_0().flex().flex_col().gap_1().child(
                                            div()
                                                .text_lg()
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                .text_ellipsis()
                                                .child(display_title),
                                        ),
                                    ),
                            )
                            .child(
                                div()
                                    .id("open-video")
                                    .cursor(CursorStyle::PointingHand)
                                    .rounded_md()
                                    .border_1()
                                    .border_color(rgb(BORDER))
                                    .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                                    .px_4()
                                    .py_2()
                                    .text_xs()
                                    .child("OPEN MP4")
                                    .on_click(cx.listener(|this, _, _, cx| this.open_picker(cx))),
                            ),
                    )
                    .child(
                        div()
                            .relative()
                            .h(px(video_height + CONTROL_HEIGHT))
                            .flex_shrink_0()
                            .child(shared_playback)
                            .when_some(settings_menu, |this, menu| this.child(menu)),
                    ),
            )
            .when(self.inspector_open, |this| {
                this.child(self.inspector_panel(cx))
            })
    }
}
