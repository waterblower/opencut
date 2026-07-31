use gpui::{
    App, Application, Bounds, Context, CursorStyle, PathPromptOptions, Render, Window,
    WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use gpui_video_player::{Video, VideoOptions, video};
use std::{env, path::PathBuf, process, time::Duration};
use url::Url;

const CONTROL_BAR_HEIGHT: f32 = 76.0;
const PLAYER_PADDING: f32 = 24.0;

struct Player {
    video: Option<Video>,
    title: String,
    error: Option<String>,
    looping: bool,
}

impl Player {
    fn open_picker(&mut self, cx: &mut Context<Self>) {
        self.error = None;

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
            }
            Err(error) => self.error = Some(error),
        }
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
}

impl Render for Player {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let viewport_width: f32 = viewport.width.into();
        let viewport_height: f32 = viewport.height.into();
        let video_width = (viewport_width - PLAYER_PADDING * 2.0).max(1.0);
        let video_height = (viewport_height - CONTROL_BAR_HEIGHT - PLAYER_PADDING * 2.0).max(1.0);

        let is_paused = self.video.as_ref().is_none_or(Video::paused);
        let is_muted = self.video.as_ref().is_some_and(Video::muted);
        let has_video = self.video.is_some();
        let status = self.error.clone().unwrap_or_else(|| self.title.clone());
        let status_color = if self.error.is_some() {
            rgb(0xff8b8b)
        } else {
            rgb(0xaeb4c2)
        };

        let video_content = if let Some(video_handle) = &self.video {
            video(video_handle.clone())
                .id("main-video")
                .size(px(video_width), px(video_height))
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
                .rounded_lg()
                .border_1()
                .border_color(rgb(0x252935))
                .bg(rgb(0x0d1016))
                .child(
                    div()
                        .text_xl()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("No video selected"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x8991a3))
                        .child("Choose an MP4 file to start playback."),
                )
                .child(
                    div()
                        .id("open-video-empty-state")
                        .cursor(CursorStyle::PointingHand)
                        .rounded_md()
                        .bg(rgb(0x5b7cfa))
                        .hover(|style| style.bg(rgb(0x7290ff)))
                        .px_5()
                        .py_2()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("Open MP4")
                        .on_click(cx.listener(|this, _, _, cx| this.open_picker(cx))),
                )
                .into_any_element()
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x090b10))
            .text_color(rgb(0xe8eaf0))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .overflow_hidden()
                    .p(px(PLAYER_PADDING))
                    .child(video_content),
            )
            .child(
                div()
                    .h(px(CONTROL_BAR_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .px_6()
                    .border_t_1()
                    .border_color(rgb(0x252935))
                    .bg(rgb(0x11141b))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .id("open-video")
                                    .flex_shrink_0()
                                    .cursor(CursorStyle::PointingHand)
                                    .rounded_md()
                                    .bg(rgb(0x252a36))
                                    .hover(|style| style.bg(rgb(0x343b4b)))
                                    .px_4()
                                    .py_2()
                                    .child("Open")
                                    .on_click(cx.listener(|this, _, _, cx| this.open_picker(cx))),
                            )
                            .child(
                                div()
                                    .overflow_hidden()
                                    .text_ellipsis()
                                    .whitespace_nowrap()
                                    .text_sm()
                                    .text_color(status_color)
                                    .child(status),
                            ),
                    )
                    .when(has_video, |this| {
                        this.child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .id("back-ten-seconds")
                                        .cursor(CursorStyle::PointingHand)
                                        .rounded_md()
                                        .bg(rgb(0x252a36))
                                        .hover(|style| style.bg(rgb(0x343b4b)))
                                        .px_4()
                                        .py_2()
                                        .child("-10s")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.seek_by(-10);
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    div()
                                        .id("play-pause")
                                        .cursor(CursorStyle::PointingHand)
                                        .rounded_md()
                                        .bg(rgb(0x5b7cfa))
                                        .hover(|style| style.bg(rgb(0x7290ff)))
                                        .min_w(px(84.0))
                                        .px_5()
                                        .py_2()
                                        .text_center()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .child(if is_paused { "Play" } else { "Pause" })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.toggle_playback();
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    div()
                                        .id("forward-ten-seconds")
                                        .cursor(CursorStyle::PointingHand)
                                        .rounded_md()
                                        .bg(rgb(0x252a36))
                                        .hover(|style| style.bg(rgb(0x343b4b)))
                                        .px_4()
                                        .py_2()
                                        .child("+10s")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.seek_by(10);
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    div()
                                        .id("mute")
                                        .cursor(CursorStyle::PointingHand)
                                        .rounded_md()
                                        .bg(rgb(0x252a36))
                                        .hover(|style| style.bg(rgb(0x343b4b)))
                                        .min_w(px(72.0))
                                        .px_4()
                                        .py_2()
                                        .text_center()
                                        .child(if is_muted { "Unmute" } else { "Mute" })
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            if let Some(video) = &this.video {
                                                video.set_muted(!video.muted());
                                            }
                                            cx.notify();
                                        })),
                                ),
                        )
                    })
                    .child(
                        div()
                            .flex_1()
                            .text_right()
                            .text_sm()
                            .text_color(rgb(0x72798a))
                            .child("GPUI"),
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
        let (video, title, error) = match initial_media.as_ref() {
            Some((url, title)) => match create_video(url, looping) {
                Ok(video) => (Some(video), title.clone(), None),
                Err(error) => (None, "No video selected".to_string(), Some(error)),
            },
            None => (None, "No video selected".to_string(), None),
        };

        let bounds = Bounds::centered(None, size(px(960.0), px(640.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                ..WindowOptions::default()
            },
            move |_, cx| {
                cx.new(|_| Player {
                    video,
                    title: title.clone(),
                    error: error.clone(),
                    looping,
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
