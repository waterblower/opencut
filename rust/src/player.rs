use gpui::{
    App, Context, CursorStyle, FocusHandle, KeyBinding, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PathPromptOptions, Render, Window, actions, div, prelude::*, px,
    relative, rgb,
};
use gpui_video_player::{Video, VideoOptions, video};
use std::{path::PathBuf, time::Duration, time::Instant};
use url::Url;

mod inspector;
mod view;

const HEADER_HEIGHT: f32 = 72.0;
const CONTROL_HEIGHT: f32 = 116.0;
const FOOTER_HEIGHT: f32 = 92.0;
const HORIZONTAL_PADDING: f32 = 22.0;
const INSPECTOR_WIDTH: f32 = 320.0;

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
        ExitFullscreen,
        ToggleInspector
    ]
);

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("space", TogglePlayback, None),
        KeyBinding::new("left", SeekBackward, None),
        KeyBinding::new("right", SeekForward, None),
        KeyBinding::new("m", ToggleMute, None),
        KeyBinding::new("f", ToggleFullscreen, None),
        KeyBinding::new("escape", ExitFullscreen, None),
        KeyBinding::new("cmd-alt-i", ToggleInspector, None),
    ]);
}

pub(crate) struct Player {
    video: Option<Video>,
    title: String,
    error: Option<String>,
    looping: bool,
    settings_open: bool,
    is_scrubbing: bool,
    scrub_fraction: Option<f32>,
    pending_seek_started: Option<Instant>,
    last_scrub_seek: Option<Instant>,
    inspector_open: bool,
    render_fps: f32,
    fps_frame_count: u32,
    fps_sample_started: Instant,
    focus_handle: FocusHandle,
}

impl Player {
    pub(crate) fn new(
        initial_media: Option<(Url, String)>,
        looping: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let (video, title, error) = match initial_media {
            Some((url, title)) => match create_video(&url, looping) {
                Ok(video) => (Some(video), title, None),
                Err(error) => (None, "No video selected".to_string(), Some(error)),
            },
            None => (None, "No video selected".to_string(), None),
        };

        let focus_handle = cx.focus_handle();
        focus_handle.focus(window);
        Self::start_progress_updates(cx);

        Self {
            video,
            title,
            error,
            looping,
            settings_open: false,
            is_scrubbing: false,
            scrub_fraction: None,
            pending_seek_started: None,
            last_scrub_seek: None,
            inspector_open: false,
            render_fps: 0.0,
            fps_frame_count: 0,
            fps_sample_started: Instant::now(),
            focus_handle,
        }
    }

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

    fn record_render_frame(&mut self) {
        self.fps_frame_count = self.fps_frame_count.saturating_add(1);
        let elapsed = self.fps_sample_started.elapsed();
        if elapsed >= Duration::from_secs(1) {
            self.render_fps = self.fps_frame_count as f32 / elapsed.as_secs_f32();
            self.fps_frame_count = 0;
            self.fps_sample_started = Instant::now();
        }
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
        let content_width = window_width
            - if self.inspector_open {
                INSPECTOR_WIDTH
            } else {
                0.0
            };
        let usable_width = (content_width - HORIZONTAL_PADDING * 2.0).max(1.0);
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

    fn action_toggle_inspector(
        &mut self,
        _: &ToggleInspector,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.inspector_open = !self.inspector_open;
        self.render_fps = 0.0;
        self.fps_frame_count = 0;
        self.fps_sample_started = Instant::now();
        cx.notify();
    }
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
