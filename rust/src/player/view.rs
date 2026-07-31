use super::*;

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
            let playback_area = if let Some(video_handle) = &self.video {
                video(video_handle.clone())
                    .id("fullscreen-video")
                    .size(px(fullscreen_content_width), px(viewport_height))
                    .buffer_capacity(3)
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
                .on_action(cx.listener(Self::action_toggle_fullscreen))
                .on_action(cx.listener(Self::action_exit_fullscreen))
                .on_action(cx.listener(Self::action_toggle_inspector))
                .on_mouse_move(cx.listener(Self::scrub_timeline))
                .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_scrubbing))
                .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_scrubbing))
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

        let content_width = (viewport_width
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
        let reported_position = self.video.as_ref().map_or(Duration::ZERO, Video::position);
        let duration = self.video.as_ref().map_or(Duration::ZERO, Video::duration);
        let speed = self.video.as_ref().map_or(1.0, Video::speed);
        let source_metadata = self.video.as_ref().map(|video| {
            let (width, height) = video.display_size();
            format!(
                "{}×{} · {} · {}",
                width,
                height,
                format_source_fps(video.framerate()),
                format_bitrate(self.bitrate_bps)
            )
        });
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
        let metadata_text = self.error.clone().unwrap_or_else(|| {
            if has_video {
                format!(
                    "MP4 · {} · {} · Original · {}",
                    source_metadata.as_deref().unwrap_or("metadata unavailable"),
                    format_duration(duration),
                    if is_muted { "Muted" } else { "Audio enabled" }
                )
            } else {
                "No media loaded".to_string()
            }
        });

        let video_content = if let Some(video_handle) = &self.video {
            video(video_handle.clone())
                .id("main-video")
                .size(px(content_width), px(video_height))
                .buffer_capacity(3)
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

        let speed_items =
            [0.5_f64, 1.0, 1.25, 1.5, 2.0]
                .into_iter()
                .enumerate()
                .map(|(index, value)| {
                    let selected = (value - speed).abs() < 0.01;
                    div()
                        .id(("speed", index))
                        .h_9()
                        .flex()
                        .items_center()
                        .justify_between()
                        .cursor(CursorStyle::PointingHand)
                        .rounded_md()
                        .px_3()
                        .text_sm()
                        .text_color(if selected { rgb(TEXT) } else { rgb(MUTED) })
                        .hover(|style| style.bg(rgb(SURFACE_HOVER)).text_color(rgb(TEXT)))
                        .child(format_speed(value))
                        .when(selected, |this| {
                            this.child(div().size_2().rounded_full().bg(rgb(ACCENT)))
                        })
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.set_speed(value);
                            cx.notify();
                        }))
                });

        div()
            .id("player-root")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::action_toggle_playback))
            .on_action(cx.listener(Self::action_seek_backward))
            .on_action(cx.listener(Self::action_seek_forward))
            .on_action(cx.listener(Self::action_toggle_mute))
            .on_action(cx.listener(Self::action_toggle_fullscreen))
            .on_action(cx.listener(Self::action_exit_fullscreen))
            .on_action(cx.listener(Self::action_toggle_inspector))
            .on_mouse_move(cx.listener(Self::scrub_timeline))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_scrubbing))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_scrubbing))
            .size_full()
            .flex()
            .overflow_hidden()
            .bg(rgb(BACKGROUND))
            .text_color(rgb(TEXT))
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
                                    .flex_col()
                                    .gap_1()
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .child(display_title),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_family("monospace")
                                            .text_color(if self.error.is_some() {
                                                rgb(ERROR)
                                            } else {
                                                rgb(0x55555d)
                                            })
                                            .child(metadata_text),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_end()
                                    .gap_2()
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
                                            .on_click(
                                                cx.listener(|this, _, _, cx| this.open_picker(cx)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_family("monospace")
                                            .text_color(rgb(0x4b4b52))
                                            .child(
                                                "space · ←/→ 5s · f fullscreen · m mute · ⌥⌘i inspector",
                                            ),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .id("playback-area")
                            .h(px(video_height))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .overflow_hidden()
                            .bg(rgb(0x000000))
                            .when(has_video, |this| {
                                this.cursor(CursorStyle::PointingHand).on_click(cx.listener(
                                    |this, _, _, cx| {
                                        this.toggle_playback();
                                        cx.notify();
                                    },
                                ))
                            })
                            .child(video_content),
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
                            .px(px(HORIZONTAL_PADDING))
                            .border_t_1()
                            .border_b_1()
                            .border_color(rgb(0x19191c))
                            .bg(rgb(0x0b0b0d))
                            .when(has_video, |this| {
                                this.child(
                                    div()
                                        .id("timeline")
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
                                                                .size(px(if self.is_scrubbing {
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
                                            cx.listener(Self::begin_scrubbing),
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
                                                    .id("play-pause")
                                                    .w_9()
                                                    .h_9()
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .cursor(CursorStyle::PointingHand)
                                                    .rounded_full()
                                                    .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                                                    .text_lg()
                                                    .child(if is_paused { "▶" } else { "Ⅱ" })
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.toggle_playback();
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(div().text_sm().font_family("monospace").child(
                                                format!(
                                                    "{} / {}",
                                                    format_duration(position),
                                                    format_duration(duration)
                                                ),
                                            )),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .id("mute")
                                                    .cursor(CursorStyle::PointingHand)
                                                    .rounded_md()
                                                    .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                                                    .px_3()
                                                    .py_2()
                                                    .text_sm()
                                                    .child(if is_muted { "MUTED" } else { "VOL" })
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.toggle_mute();
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(
                                                div()
                                                    .id("speed")
                                                    .cursor(CursorStyle::PointingHand)
                                                    .rounded_md()
                                                    .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                                                    .px_3()
                                                    .py_2()
                                                    .text_sm()
                                                    .text_color(rgb(MUTED))
                                                    .child(format_speed(speed))
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.settings_open = !this.settings_open;
                                                        cx.notify();
                                                    })),
                                            )
                                            .child(
                                                div()
                                                    .id("fullscreen")
                                                    .cursor(CursorStyle::PointingHand)
                                                    .rounded_md()
                                                    .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                                                    .px_3()
                                                    .py_2()
                                                    .text_lg()
                                                    .child("⛶")
                                                    .on_click(cx.listener(|_, _, window, cx| {
                                                        window.toggle_fullscreen();
                                                        cx.notify();
                                                    })),
                                            ),
                                    ),
                            )
                            .when(self.settings_open && has_video, |this| {
                                this.child(
                                    div()
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
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(0x65656d))
                                                .child("QUALITY"),
                                        )
                                        .child(
                                            div()
                                                .h_10()
                                                .flex()
                                                .items_center()
                                                .justify_between()
                                                .px_3()
                                                .text_sm()
                                                .child("Original file")
                                                .child(
                                                    div().size_2().rounded_full().bg(rgb(ACCENT)),
                                                ),
                                        )
                                        .child(div().h_px().bg(rgb(BORDER)))
                                        .child(
                                            div()
                                                .mt_2()
                                                .text_xs()
                                                .text_color(rgb(0x65656d))
                                                .child("PLAYBACK SPEED"),
                                        )
                                        .children(speed_items)
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
                                                .child(
                                                    div().size_2().rounded_full().bg(rgb(ACCENT)),
                                                )
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.toggle_mute();
                                                    cx.notify();
                                                })),
                                        ),
                                )
                            }),
                    )
            )
            .when(self.inspector_open, |this| {
                this.child(self.inspector_panel(cx))
            })
    }
}
