use gpui::{
    App, Context, CursorStyle, FocusHandle, KeyBinding, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ObjectFit, PathPromptOptions, Render, Window, actions, div, img,
    prelude::*, px, rgb,
};
use gst::prelude::*;
use gstreamer as gst;
use std::{path::PathBuf, time::Duration, time::Instant};
use url::Url;

use crate::playback_view::{DragPhase, PlaybackViewDelegate, format_duration};
use crate::video::{Video, video};

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
    video: Option<Video>,
    bitrate_bps: Option<f64>,
    history: HistoryData,
    current_media_path: Option<PathBuf>,
    title: String,
    looping: bool,
    history_open: bool,
    history_width: f32,
    is_resizing_history: bool,
    settings_open: bool,
    volume_open: bool,
    is_scrubbing: bool,
    is_adjusting_volume: bool,
    resume_after_scrub: bool,
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
        let mut history = HistoryData::load();
        let mut current_media_path = None;
        let (video, bitrate_bps, title) = match initial_media {
            Some((url, title)) => match Video::open(&url, looping) {
                Ok(video) => {
                    let bitrate = average_bitrate(&url, video.duration());
                    if let Ok(path) = url.to_file_path() {
                        let path = std::fs::canonicalize(&path).unwrap_or(path);
                        history.record(&path, title.clone());
                        current_media_path = Some(path);
                    }
                    (Some(video), bitrate, title)
                }
                Err(error) => {
                    eprintln!("{error}");
                    (None, None, "No video selected".to_string())
                }
            },
            None => (None, None, "No video selected".to_string()),
        };

        let focus_handle = cx.focus_handle();
        focus_handle.focus(window);
        Self::start_progress_updates(cx);

        Self {
            video,
            bitrate_bps,
            history,
            current_media_path,
            title,
            looping,
            history_open: true,
            history_width: load_history_width(),
            is_resizing_history: false,
            settings_open: false,
            volume_open: false,
            is_scrubbing: false,
            is_adjusting_volume: false,
            resume_after_scrub: false,
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
                            eprintln!("Could not open file picker: {error}");
                        }
                        Err(error) => {
                            eprintln!("File picker closed unexpectedly: {error}");
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
            eprintln!("Please select an MP4 video.");
            return;
        }

        let path = std::fs::canonicalize(&path).unwrap_or(path);
        let title = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let Ok(url) = Url::from_file_path(&path) else {
            eprintln!("Could not read {}", path.display());
            return;
        };

        match Video::open(&url, self.looping) {
            Ok(video) => {
                self.bitrate_bps = average_bitrate(&url, video.duration());
                self.video = Some(video);
                self.history.record(&path, title.clone());
                self.current_media_path = Some(path);
                self.title = title;
                self.volume_open = false;
                self.is_scrubbing = false;
                self.is_adjusting_volume = false;
                self.resume_after_scrub = false;
                self.scrub_fraction = None;
                self.pending_seek_started = None;
                self.last_scrub_seek = None;
            }
            Err(error) => eprintln!("{error}"),
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

    fn seek_by_frame(&mut self, direction: i8) {
        let Some(video) = &self.video else {
            return;
        };

        // Frame stepping is meaningless without a fixed rate, so skip it for
        // variable-frame-rate sources.
        let Some(frames_per_second) = video.framerate() else {
            return;
        };

        let duration = video.duration();
        if duration.is_zero() {
            return;
        }

        let position = self.scrub_fraction.map_or_else(
            || video.position(),
            |fraction| duration.mul_f64(fraction as f64),
        );
        let frame_duration = Duration::from_secs_f64(1.0 / frames_per_second);
        let target = if direction.is_negative() {
            position.saturating_sub(frame_duration)
        } else {
            position.saturating_add(frame_duration)
        }
        .min(duration);

        self.scrub_fraction = Some((target.as_secs_f64() / duration.as_secs_f64()) as f32);
        self.pending_seek_started = Some(Instant::now());
        let _ = video.seek(target, true);
    }

    fn seek_to_fraction(&self, fraction: f64, accurate: bool) {
        let Some(video) = &self.video else {
            return;
        };
        let target = video.duration().mul_f64(fraction.clamp(0.0, 1.0));
        let _ = video.seek(target, accurate);
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
        let settle_tolerance = video
            .framerate()
            .map(|frames_per_second| Duration::from_secs_f64(0.75 / frames_per_second))
            .unwrap_or_else(|| Duration::from_millis(20));
        let settled = actual.abs_diff(target) <= settle_tolerance;
        let timed_out = started.elapsed() >= Duration::from_secs(2);

        if settled || timed_out {
            self.scrub_fraction = None;
            self.pending_seek_started = None;
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
            Ok(()) => {}
            Err(error) => eprintln!("Could not change speed: {error}"),
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
        self.toggle_playback();
        cx.notify();
    }

    fn action_seek_backward(&mut self, _: &SeekBackward, _: &mut Window, cx: &mut Context<Self>) {
        self.seek_by_frame(-1);
        cx.notify();
    }

    fn action_seek_forward(&mut self, _: &SeekForward, _: &mut Window, cx: &mut Context<Self>) {
        self.seek_by_frame(1);
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
        self.toggle_playback();
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
                if let Some(video) = &self.video {
                    video.set_paused(true);
                }
                self.is_scrubbing = true;
                self.scrub_fraction = Some(fraction);
                self.pending_seek_started = None;
                self.last_scrub_seek = Some(Instant::now());
                self.seek_to_fraction(fraction as f64, false);
            }
            DragPhase::Update if self.is_scrubbing => {
                self.scrub_fraction = Some(fraction);
                let now = Instant::now();
                let should_seek = self.last_scrub_seek.is_none_or(|last_seek| {
                    now.duration_since(last_seek) >= Duration::from_millis(50)
                });
                if should_seek {
                    self.last_scrub_seek = Some(now);
                    self.seek_to_fraction(fraction as f64, false);
                }
            }
            DragPhase::End if self.is_scrubbing => {
                self.scrub_fraction = Some(fraction);
                self.pending_seek_started = Some(Instant::now());
                self.last_scrub_seek = None;
                self.is_scrubbing = false;
                self.seek_to_fraction(fraction as f64, true);
                if self.resume_after_scrub
                    && let Some(video) = &self.video
                {
                    video.set_paused(false);
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
        if let Some(video) = &self.video {
            video.set_volume(volume);
            video.set_muted(volume <= f64::EPSILON);
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

fn video_codec(video: &Video) -> Option<String> {
    let pipeline = video.pipeline();
    let stream_index = pipeline.property::<i32>("current-video");
    if stream_index < 0 {
        return None;
    }
    let tags = pipeline.emit_by_name::<Option<gst::TagList>>("get-video-tags", &[&stream_index])?;
    Some(format_codec_name(
        tags.get::<gst::tags::VideoCodec>()?.get(),
    ))
}

fn format_codec_name(codec: &str) -> String {
    let lowercase = codec.to_ascii_lowercase();
    if lowercase.contains("h.264")
        || lowercase.contains("h264")
        || lowercase.contains("avc")
        || lowercase.contains("avc1")
        || lowercase.contains("x264")
    {
        "H.264".to_string()
    } else if lowercase.contains("h.265")
        || lowercase.contains("h265")
        || lowercase.contains("hevc")
        || lowercase.contains("hvec")
        || lowercase.contains("hvc1")
        || lowercase.contains("hev1")
        || lowercase.contains("x265")
    {
        "H.265/HEVC".to_string()
    } else if lowercase.contains("av1") {
        "AV1".to_string()
    } else if lowercase.contains("vp9") {
        "VP9".to_string()
    } else if lowercase.contains("vp8") {
        "VP8".to_string()
    } else {
        codec.to_string()
    }
}

fn average_bitrate(url: &Url, duration: Duration) -> Option<f64> {
    if duration.is_zero() {
        return None;
    }

    let path = url.to_file_path().ok()?;
    let bytes = std::fs::metadata(path).ok()?.len();
    Some(bytes as f64 * 8.0 / duration.as_secs_f64())
}

fn format_speed(speed: f64) -> String {
    if (speed - speed.round()).abs() < 0.01 {
        format!("{speed:.0}.0×")
    } else {
        format!("{speed:.2}×").replace('0', "")
    }
}

fn format_source_fps(fps: f64) -> String {
    if (fps - fps.round()).abs() < 0.01 {
        format!("{fps:.0} fps")
    } else {
        format!("{fps:.2} fps")
    }
}

fn format_bitrate(bitrate_bps: Option<f64>) -> String {
    match bitrate_bps {
        Some(bitrate) if bitrate >= 1_000_000.0 => format!("{:.1} Mbps", bitrate / 1_000_000.0),
        Some(bitrate) => format!("{:.0} kbps", bitrate / 1_000.0),
        None => "bitrate unavailable".to_string(),
    }
}
