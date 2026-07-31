use gpui::{
    App, Application, Bounds, Context, CursorStyle, FocusHandle, KeyBinding, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PathPromptOptions, Render, Window, WindowBounds,
    WindowOptions, actions, div, prelude::*, px, relative, rgb, size,
};
use gpui_video_player::{Video, VideoOptions, video};
use std::{env, path::PathBuf, process, time::Duration, time::Instant};
use url::Url;

const HEADER_HEIGHT: f32 = 72.0;
const CONTROL_HEIGHT: f32 = 116.0;
const FOOTER_HEIGHT: f32 = 92.0;
const HORIZONTAL_PADDING: f32 = 22.0;

const BACKGROUND: u32 = 0x070708;
const SURFACE: u32 = 0x101012;
const SURFACE_HOVER: u32 = 0x1b1b1f;
const BORDER: u32 = 0x29292e;
const TEXT: u32 = 0xf0f0f2;
const MUTED: u32 = 0x77777f;
const ACCENT: u32 = 0xf0b75e;
const ERROR: u32 = 0xff8b8b;

actions!(
    opencut,
    [
        TogglePlayback,
        SeekBackward,
        SeekForward,
        ToggleMute,
        ToggleFullscreen,
        ExitFullscreen
    ]
);

struct Player {
    video: Option<Video>,
    title: String,
    error: Option<String>,
    looping: bool,
    settings_open: bool,
    is_scrubbing: bool,
    scrub_fraction: Option<f32>,
    pending_seek_started: Option<Instant>,
    last_scrub_seek: Option<Instant>,
    focus_handle: FocusHandle,
}

impl Player {
    fn start_progress_updates(cx: &mut Context<Self>) {
        cx.spawn(async move |player, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                if player
                    .update(cx, |player, cx| {
                        player.reconcile_pending_seek();
                        cx.notify();
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    fn open_picker(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        self.settings_open = false;

        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open MP4".into()),
        });

        cx.spawn(async move |player, cx| {
            let result = selection.await;
            player
                .update(cx, |player, cx| {
                    match result {
                        Ok(Ok(Some(paths))) => {
                            if let Some(path) = paths.into_iter().next() {
                                player.open_path(path);
                            }
                        }
                        Ok(Ok(None)) => {}
                        Ok(Err(error)) => {
                            player.error = Some(format!("Could not open file picker: {error}"));
                        }
                        Err(error) => {
                            player.error =
                                Some(format!("File picker closed unexpectedly: {error}"));
                        }
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    fn open_path(&mut self, path: PathBuf) {
        let is_mp4 = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"));

        if !is_mp4 {
            self.error = Some("Please select an MP4 video.".to_string());
            return;
        }

        let title = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let Ok(url) = Url::from_file_path(&path) else {
            self.error = Some(format!("Could not read {}", path.display()));
            return;
        };

        match create_video(&url, self.looping) {
            Ok(video) => {
                self.video = Some(video);
                self.title = title;
                self.error = None;
                self.is_scrubbing = false;
                self.scrub_fraction = None;
                self.pending_seek_started = None;
                self.last_scrub_seek = None;
            }
            Err(error) => self.error = Some(error),
        }
    }

    fn display_title(&self) -> String {
        if self.video.is_none() {
            return "OpenCut".to_string();
        }

        std::path::Path::new(&self.title)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.title.clone())
    }

    fn seek_by(&self, seconds: i64) {
        let Some(video) = &self.video else {
            return;
        };
        let position = video.position();
        let target = if seconds.is_negative() {
            position.saturating_sub(Duration::from_secs(seconds.unsigned_abs()))
        } else {
            position.saturating_add(Duration::from_secs(seconds as u64))
        };

        let _ = video.seek(target.min(video.duration()), false);
    }

    fn seek_to_fraction(&self, fraction: f64, accurate: bool) {
        let Some(video) = &self.video else {
            return;
        };
        let target = video.duration().mul_f64(fraction.clamp(0.0, 1.0));
        let _ = video.seek(target, accurate);
    }

    fn timeline_fraction_from_x(&self, x: f32, window: &Window) -> f32 {
        let window_width: f32 = window.viewport_size().width.into();
        let usable_width = (window_width - HORIZONTAL_PADDING * 2.0).max(1.0);
        ((x - HORIZONTAL_PADDING) / usable_width).clamp(0.0, 1.0)
    }

    fn reconcile_pending_seek(&mut self) {
        if self.is_scrubbing {
            return;
        }

        let (Some(target_fraction), Some(started), Some(video)) = (
            self.scrub_fraction,
            self.pending_seek_started,
            self.video.as_ref(),
        ) else {
            return;
        };

        let duration = video.duration();
        let target = duration.mul_f64(target_fraction as f64);
        let actual = video.position();
        let settled = actual.abs_diff(target) <= Duration::from_millis(750);
        let timed_out = started.elapsed() >= Duration::from_secs(2);

        if settled || timed_out {
            self.scrub_fraction = None;
            self.pending_seek_started = None;
        }
    }

    fn begin_scrubbing(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_scrubbing = true;
        let fraction = self.timeline_fraction_from_x(event.position.x.into(), window);
        self.scrub_fraction = Some(fraction);
        self.pending_seek_started = None;
        self.last_scrub_seek = Some(Instant::now());
        self.seek_to_fraction(fraction as f64, false);
        cx.notify();
    }

    fn scrub_timeline(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_scrubbing && event.dragging() {
            let fraction = self.timeline_fraction_from_x(event.position.x.into(), window);
            self.scrub_fraction = Some(fraction);

            let now = Instant::now();
            let should_seek = self
                .last_scrub_seek
                .is_none_or(|last_seek| now.duration_since(last_seek) >= Duration::from_millis(50));
            if should_seek {
                self.last_scrub_seek = Some(now);
                self.seek_to_fraction(fraction as f64, false);
            }
            cx.notify();
        }
    }

    fn finish_scrubbing(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_scrubbing {
            let fraction = self.timeline_fraction_from_x(event.position.x.into(), window);
            self.scrub_fraction = Some(fraction);
            self.pending_seek_started = Some(Instant::now());
            self.last_scrub_seek = None;
            self.is_scrubbing = false;
            self.seek_to_fraction(fraction as f64, true);
            cx.notify();
        }
    }

    fn toggle_playback(&self) {
        let Some(video) = &self.video else {
            return;
        };
        if video.eos() {
            let _ = video.restart_stream();
            video.set_paused(false);
        } else {
            video.set_paused(!video.paused());
        }
    }

    fn toggle_mute(&self) {
        if let Some(video) = &self.video {
            video.set_muted(!video.muted());
        }
    }

    fn set_speed(&mut self, speed: f64) {
        let Some(video) = &self.video else {
            return;
        };
        match video.set_speed(speed) {
            Ok(()) => self.error = None,
            Err(error) => self.error = Some(format!("Could not change speed: {error}")),
        }
        self.settings_open = false;
    }

    fn action_toggle_playback(
        &mut self,
        _: &TogglePlayback,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.toggle_playback();
        cx.notify();
    }

    fn action_seek_backward(&mut self, _: &SeekBackward, _: &mut Window, cx: &mut Context<Self>) {
        self.seek_by(-5);
        cx.notify();
    }

    fn action_seek_forward(&mut self, _: &SeekForward, _: &mut Window, cx: &mut Context<Self>) {
        self.seek_by(5);
        cx.notify();
    }

    fn action_toggle_mute(&mut self, _: &ToggleMute, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_mute();
        cx.notify();
    }

    fn action_toggle_fullscreen(
        &mut self,
        _: &ToggleFullscreen,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.toggle_fullscreen();
        cx.notify();
    }

    fn action_exit_fullscreen(
        &mut self,
        _: &ExitFullscreen,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if window.is_fullscreen() {
            window.toggle_fullscreen();
            cx.notify();
        }
    }
}

impl Render for Player {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let viewport_width: f32 = viewport.width.into();
        let viewport_height: f32 = viewport.height.into();

        if window.is_fullscreen() {
            let playback_area = if let Some(video_handle) = &self.video {
                video(video_handle.clone())
                    .id("fullscreen-video")
                    .size(px(viewport_width), px(viewport_height))
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
                .on_mouse_move(cx.listener(Self::scrub_timeline))
                .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_scrubbing))
                .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_scrubbing))
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .bg(rgb(0x000000))
                .child(playback_area);
        }

        let video_height =
            (viewport_height - HEADER_HEIGHT - CONTROL_HEIGHT - FOOTER_HEIGHT).max(140.0);

        let has_video = self.video.is_some();
        let is_paused = self.video.as_ref().is_none_or(Video::paused);
        let is_muted = self.video.as_ref().is_some_and(Video::muted);
        let reported_position = self.video.as_ref().map_or(Duration::ZERO, Video::position);
        let duration = self.video.as_ref().map_or(Duration::ZERO, Video::duration);
        let speed = self.video.as_ref().map_or(1.0, Video::speed);
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

        let video_content = if let Some(video_handle) = &self.video {
            video(video_handle.clone())
                .id("main-video")
                .size(px(viewport_width), px(video_height))
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
            .on_mouse_move(cx.listener(Self::scrub_timeline))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_scrubbing))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_scrubbing))
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgb(BACKGROUND))
            .text_color(rgb(TEXT))
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
                                    .child(display_title.clone()),
                            )
                            .child(div().text_xs().text_color(rgb(MUTED)).child(if has_video {
                                "LOCAL PLAYBACK · ORIGINAL FILE"
                            } else {
                                "GPUI · GSTREAMER"
                            })),
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
                    .h(px(video_height))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .bg(rgb(0x000000))
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
                                    .child(
                                        div()
                                            .id("seek-back")
                                            .cursor(CursorStyle::PointingHand)
                                            .rounded_md()
                                            .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                                            .px_3()
                                            .py_2()
                                            .text_color(rgb(MUTED))
                                            .child("‹ 5")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.seek_by(-5);
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .id("seek-forward")
                                            .cursor(CursorStyle::PointingHand)
                                            .rounded_md()
                                            .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                                            .px_3()
                                            .py_2()
                                            .text_color(rgb(MUTED))
                                            .child("5 ›")
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.seek_by(5);
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
                                            .rounded_md()
                                            .px_3()
                                            .py_2()
                                            .text_sm()
                                            .text_color(rgb(0x4f4f56))
                                            .child("CC"),
                                    )
                                    .child(
                                        div()
                                            .id("settings")
                                            .cursor(CursorStyle::PointingHand)
                                            .rounded_md()
                                            .border_1()
                                            .border_color(if self.settings_open {
                                                rgb(ACCENT)
                                            } else {
                                                rgb(BORDER)
                                            })
                                            .bg(if self.settings_open {
                                                rgb(SURFACE_HOVER)
                                            } else {
                                                rgb(SURFACE)
                                            })
                                            .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                                            .px_4()
                                            .py_2()
                                            .text_sm()
                                            .child("Original")
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
                                        .child(div().size_2().rounded_full().bg(rgb(ACCENT)))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.toggle_mute();
                                            cx.notify();
                                        })),
                                ),
                        )
                    }),
            )
            .child(
                div()
                    .h(px(FOOTER_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(HORIZONTAL_PADDING))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::MEDIUM)
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
                                    .child(self.error.clone().unwrap_or_else(|| {
                                        if has_video {
                                            format!(
                                                "MP4 · {} · Original · {}",
                                                format_duration(duration),
                                                if is_muted { "Muted" } else { "Audio enabled" }
                                            )
                                        } else {
                                            "No media loaded".to_string()
                                        }
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(rgb(0x4b4b52))
                            .child("space · ←/→ 5s · f fullscreen · m mute"),
                    ),
            )
    }
}

fn main() {
    env_logger::init();

    let (initial_media, looping) = match parse_args() {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            print_usage();
            process::exit(2);
        }
    };

    Application::new().run(move |cx: &mut App| {
        cx.bind_keys([
            KeyBinding::new("space", TogglePlayback, None),
            KeyBinding::new("left", SeekBackward, None),
            KeyBinding::new("right", SeekForward, None),
            KeyBinding::new("m", ToggleMute, None),
            KeyBinding::new("f", ToggleFullscreen, None),
            KeyBinding::new("escape", ExitFullscreen, None),
        ]);

        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let (video, title, error) = match initial_media.as_ref() {
            Some((url, title)) => match create_video(url, looping) {
                Ok(video) => (Some(video), title.clone(), None),
                Err(error) => (None, "No video selected".to_string(), Some(error)),
            },
            None => (None, "No video selected".to_string(), None),
        };

        let bounds = Bounds::centered(None, size(px(1100.0), px(760.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                ..WindowOptions::default()
            },
            move |window, cx| {
                cx.new(|cx| {
                    let focus_handle = cx.focus_handle();
                    focus_handle.focus(window);
                    Player::start_progress_updates(cx);
                    Player {
                        video,
                        title: title.clone(),
                        error: error.clone(),
                        looping,
                        settings_open: false,
                        is_scrubbing: false,
                        scrub_fraction: None,
                        pending_seek_started: None,
                        last_scrub_seek: None,
                        focus_handle,
                    }
                })
            },
        )
        .expect("failed to create the GPUI window");
        cx.activate(true);
    });
}

fn create_video(url: &Url, looping: bool) -> Result<Video, String> {
    Video::new_with_options(
        url,
        VideoOptions {
            frame_buffer_capacity: Some(3),
            looping: Some(looping),
            ..VideoOptions::default()
        },
    )
    .map_err(|error| format!("Could not open video: {error}"))
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}

fn format_speed(speed: f64) -> String {
    if (speed - speed.round()).abs() < 0.01 {
        format!("{speed:.0}.0×")
    } else {
        format!("{speed:.2}×").replace('0', "")
    }
}

fn parse_args() -> Result<(Option<(Url, String)>, bool), String> {
    let mut looping = false;
    let mut media = None;

    for argument in env::args().skip(1) {
        match argument.as_str() {
            "--loop" | "-l" => looping = true,
            "--help" | "-h" => {
                print_usage();
                process::exit(0);
            }
            _ if media.is_none() => media = Some(argument),
            _ => return Err(format!("Unexpected argument: {argument}")),
        }
    }

    let Some(media) = media else {
        return Ok((None, looping));
    };

    if let Ok(url) = Url::parse(&media)
        && matches!(url.scheme(), "file" | "http" | "https")
    {
        return Ok((Some((url, media)), looping));
    }

    let path = PathBuf::from(&media);
    if !path.is_file() {
        return Err(format!("Video does not exist: {}", path.display()));
    }

    let absolute_path = path
        .canonicalize()
        .map_err(|error| format!("Could not resolve {}: {error}", path.display()))?;
    let title = absolute_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| absolute_path.display().to_string());
    let url = Url::from_file_path(&absolute_path).map_err(|_| {
        format!(
            "Could not convert {} to a file URL",
            absolute_path.display()
        )
    })?;

    Ok((Some((url, title)), looping))
}

fn print_usage() {
    eprintln!("Usage: opencut-player [--loop] [video-path-or-url]");
}
