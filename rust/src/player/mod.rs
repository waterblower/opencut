use anyhow::{Result, bail};
use gpui::{
    App, AsyncApp, Context, CursorStyle, FocusHandle, KeyBinding, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ObjectFit, PathPromptOptions, Render, WeakEntity, Window,
    actions, div, img, prelude::*, px, rgb,
};
use std::{path::PathBuf, time::Duration, time::Instant};

use crate::playback_view::{DragPhase, PlaybackViewDelegate};
use opencut_player::video2::VideoBackend;

mod history;
mod inspector;
mod view;

use history::{HistoryData, load_history_width, save_history_width};

const HEADER_HEIGHT: f32 = 92.0;
const CONTROL_HEIGHT: f32 = crate::playback_view::CONTROL_HEIGHT;
const HORIZONTAL_PADDING: f32 = crate::playback_view::HORIZONTAL_PADDING;
const INSPECTOR_WIDTH: f32 = 320.0;
const DEFAULT_HISTORY_WIDTH: f32 = 288.0;
const MIN_HISTORY_WIDTH: f32 = 220.0;
const MAX_HISTORY_WIDTH: f32 = 480.0;
const MIN_PLAYER_WIDTH: f32 = 360.0;

const BACKGROUND: u32 = 0x070708;
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
        ToggleHistory,
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
        KeyBinding::new("cmd-b", ToggleHistory, None),
        KeyBinding::new("f", ToggleFullscreen, None),
        KeyBinding::new("escape", ExitFullscreen, None),
        KeyBinding::new("cmd-alt-i", ToggleInspector, None),
    ]);
}

pub(crate) struct Player {
    video: Option<VideoBackend>,
    history: HistoryData,
    current_media_path: Option<PathBuf>,
    title: String,
    history_open: bool,
    history_width: f32,
    is_resizing_history: bool,
    settings_open: bool,
    volume_open: bool,
    is_scrubbing: bool,
    is_adjusting_volume: bool,
    resume_after_scrub: bool,
    last_scrub_seek: Option<Instant>,
    inspector_open: bool,
    render_fps: f32,
    fps_frame_count: u32,
    fps_sample_started: Instant,
    focus_handle: FocusHandle,
}

impl Player {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let history = HistoryData::load();
        let current_media_path = None;

        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);

        Self {
            video: None,
            history,
            current_media_path,
            title: "".to_owned(),
            history_open: true,
            history_width: load_history_width(),
            is_resizing_history: false,
            settings_open: false,
            volume_open: false,
            is_scrubbing: false,
            is_adjusting_volume: false,
            resume_after_scrub: false,
            last_scrub_seek: None,
            inspector_open: false,
            render_fps: 0.0,
            fps_frame_count: 0,
            fps_sample_started: Instant::now(),
            focus_handle,
        }
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
        self.settings_open = false;

        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Open Video".into()),
        });

        cx.spawn(async move |player, cx| match selection.await {
            Ok(Ok(Some(paths))) => {
                let Some(path) = paths.into_iter().next() else {
                    return;
                };
                if let Err(error) = Self::open_path(player, path, cx).await {
                    eprintln!("Could not open video: {error:?}");
                }
            }
            Ok(Ok(None)) => {}
            Ok(Err(error)) => eprintln!("Could not open file picker: {error:?}"),
            Err(error) => eprintln!("File picker closed unexpectedly: {error:?}"),
        })
        .detach();
    }

    async fn open_path(player: WeakEntity<Self>, path: PathBuf, cx: &mut AsyncApp) -> Result<()> {
        let is_supported_video = path
            .extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("mp4") || extension.eq_ignore_ascii_case("mov")
            });

        if !is_supported_video {
            bail!("Please select an MP4 or MOV video.");
        }

        let path = std::fs::canonicalize(&path).unwrap_or(path);
        let title = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let mut video = VideoBackend::open(&path).await?;
        video.set_paused(false)?;
        player.update(cx, |player, cx| {
            player.video = Some(video);
            player.history.record(&path, title.clone());
            player.current_media_path = Some(path);
            player.title = title;
            player.volume_open = false;
            player.is_scrubbing = false;
            player.is_adjusting_volume = false;
            player.resume_after_scrub = false;
            player.last_scrub_seek = None;
            cx.notify();
        })?;
        Ok(())
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

    fn seek_by_frame(&mut self, direction: i8, cx: &mut Context<Self>) {
        let Some(video) = self.video.as_mut() else {
            return;
        };

        // Use the reported average frame interval for stepping.
        let Some(frames_per_second) = video.framerate() else {
            return;
        };

        let duration = video.duration();
        if duration.is_zero() {
            return;
        }

        let position = video.position();
        let frame_duration = Duration::from_secs_f64(1.0 / frames_per_second);
        let target = if direction.is_negative() {
            position.saturating_sub(frame_duration)
        } else {
            position.saturating_add(frame_duration)
        }
        .min(duration);

        self.seek(target, cx);
    }

    fn seek_to_fraction(&mut self, fraction: f64, cx: &mut Context<Self>) {
        let Some(video) = self.video.as_mut() else {
            return;
        };
        let target = video.duration().mul_f64(fraction.clamp(0.0, 1.0));
        self.seek(target, cx);
    }

    fn seek(&mut self, target: Duration, cx: &mut Context<Self>) {
        let Some(video) = &mut self.video else {
            return;
        };
        let completion = video.seek(target);
        cx.spawn(async move |player, cx| {
            if let Err(error) = completion.await {
                eprintln!("Could not seek video: {error:?}");
            }
            let _ = player.update(cx, |_, cx| cx.notify());
        })
        .detach();
    }

    fn set_history_width_from_x(&mut self, x: f32, window: &Window) {
        let viewport_width: f32 = window.viewport_size().width.into();
        let inspector_width = if self.inspector_open {
            INSPECTOR_WIDTH
        } else {
            0.0
        };
        let available_max = (viewport_width - inspector_width - MIN_PLAYER_WIDTH)
            .clamp(MIN_HISTORY_WIDTH, MAX_HISTORY_WIDTH);
        self.history_width = x.clamp(MIN_HISTORY_WIDTH, available_max);
    }

    fn begin_history_resize(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_resizing_history = true;
        self.set_history_width_from_x(event.position.x.into(), window);
        cx.notify();
    }

    fn resize_history(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_resizing_history && event.dragging() {
            self.set_history_width_from_x(event.position.x.into(), window);
            cx.notify();
        }
    }

    fn finish_history_resize(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_resizing_history {
            self.set_history_width_from_x(event.position.x.into(), window);
            self.is_resizing_history = false;
            save_history_width(self.history_width);
            cx.notify();
        }
    }

    fn toggle_playback(&mut self) -> Result<()> {
        let Some(video) = &mut self.video else {
            return Ok(());
        };
        video.set_paused(!video.paused())
    }

    fn toggle_mute(&mut self) {
        if let Some(video) = &mut self.video {
            if let Err(error) = video.set_muted(!video.muted()) {
                eprintln!("Could not update playback: {error:?}");
            }
        }
    }

    fn dismiss_settings(&mut self, _: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.settings_open || self.volume_open {
            self.settings_open = false;
            self.volume_open = false;
            cx.notify();
        }
    }

    fn action_toggle_playback(
        &mut self,
        _: &TogglePlayback,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = self.toggle_playback() {
            eprintln!("Could not toggle playback: {error:?}");
            return;
        }
        cx.notify();
    }

    fn action_seek_backward(&mut self, _: &SeekBackward, _: &mut Window, cx: &mut Context<Self>) {
        self.seek_by_frame(-1, cx);
        cx.notify();
    }

    fn action_seek_forward(&mut self, _: &SeekForward, _: &mut Window, cx: &mut Context<Self>) {
        self.seek_by_frame(1, cx);
        cx.notify();
    }

    fn action_toggle_mute(&mut self, _: &ToggleMute, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_mute();
        cx.notify();
    }

    fn action_toggle_history(&mut self, _: &ToggleHistory, _: &mut Window, cx: &mut Context<Self>) {
        self.history_open = !self.history_open;
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

    fn playback_toggle_fullscreen(
        &mut self,
        _: &gpui::ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.toggle_fullscreen();
        cx.notify();
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

impl PlaybackViewDelegate for Player {
    fn playback_toggle(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if let Err(error) = self.toggle_playback() {
            eprintln!("Could not toggle playback: {error:?}");
            return;
        }
        cx.notify();
    }

    fn playback_seek(
        &mut self,
        fraction: f32,
        phase: DragPhase,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match phase {
            DragPhase::Start => {
                self.resume_after_scrub = self.video.as_ref().is_some_and(|video| !video.paused());
                if let Some(video) = &mut self.video {
                    if let Err(error) = video.set_paused(true) {
                        eprintln!("Could not update playback: {error:?}");
                    }
                }
                self.is_scrubbing = true;
                self.last_scrub_seek = Some(Instant::now());
                // A press may be the complete click, so show its exact target immediately.
                // Drag updates below are throttled to limit the cost of accurate seeks.
                self.seek_to_fraction(fraction as f64, cx);
            }
            DragPhase::Update if self.is_scrubbing => {
                let now = Instant::now();
                let should_seek = self.last_scrub_seek.is_none_or(|last_seek| {
                    now.duration_since(last_seek) >= Duration::from_millis(50)
                });
                if should_seek {
                    self.last_scrub_seek = Some(now);
                    self.seek_to_fraction(fraction as f64, cx);
                }
            }
            DragPhase::End if self.is_scrubbing => {
                self.last_scrub_seek = None;
                self.is_scrubbing = false;
                self.seek_to_fraction(fraction as f64, cx);
                if self.resume_after_scrub
                    && let Some(video) = &mut self.video
                {
                    if let Err(error) = video.set_paused(false) {
                        eprintln!("Could not update playback: {error:?}");
                    }
                }
                self.resume_after_scrub = false;
            }
            _ => return,
        }
        cx.notify();
    }

    fn playback_set_volume(
        &mut self,
        volume: f64,
        phase: DragPhase,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match phase {
            DragPhase::Start => self.is_adjusting_volume = true,
            DragPhase::Update if !self.is_adjusting_volume => return,
            DragPhase::End if !self.is_adjusting_volume => return,
            DragPhase::End => self.is_adjusting_volume = false,
            DragPhase::Update => {}
        }
        if let Some(video) = &mut self.video {
            if let Err(error) = video.set_volume(volume) {
                eprintln!("Could not update playback: {error:?}");
            }
            if let Err(error) = video.set_muted(volume <= f64::EPSILON) {
                eprintln!("Could not update playback: {error:?}");
            }
        }
        cx.notify();
    }

    fn playback_toggle_volume(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.video.is_some() {
            self.volume_open = !self.volume_open;
            self.settings_open = false;
        }
        cx.notify();
    }

    fn playback_dismiss_volume(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.volume_open {
            self.volume_open = false;
            cx.notify();
        }
    }
}
