use crate::video_backend::{Video, VideoOptions, video};
use gpui::{
    App, Context, CursorStyle, FocusHandle, KeyBinding, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PathPromptOptions, Render, ScrollHandle, Window, actions, div,
    prelude::*, px, rgb,
};
use std::{path::PathBuf, time::Duration};
use url::Url;

mod export;
mod model;
mod view;

use export::export_project;
use model::{MIN_CLIP_DURATION, MediaAsset, Project, TimelineClip, probe_media};

const MEDIA_PANEL_WIDTH: f32 = 264.0;
const INSPECTOR_WIDTH: f32 = 292.0;
const TOPBAR_HEIGHT: f32 = 64.0;
const TIMELINE_HEIGHT: f32 = 238.0;
const TIMELINE_HEADER_HEIGHT: f32 = 46.0;
const TIMELINE_PADDING: f32 = 20.0;

const BACKGROUND: u32 = 0x080809;
const PANEL: u32 = 0x0d0d0f;
const SURFACE: u32 = 0x17171a;
const SURFACE_HOVER: u32 = 0x202024;
const BORDER: u32 = 0x2b2b31;
const TEXT: u32 = 0xf2f2f4;
const MUTED: u32 = 0x777780;
const ACCENT: u32 = 0xf0b75e;
const ERROR: u32 = 0xff8b8b;
const CLIP_BLUE: u32 = 0x294d75;

actions!(
    opencut_editor,
    [TogglePlayback, DeleteSelected, SplitClip, Undo, Redo]
);

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("space", TogglePlayback, None),
        KeyBinding::new("backspace", DeleteSelected, None),
        KeyBinding::new("delete", DeleteSelected, None),
        KeyBinding::new("cmd-b", SplitClip, None),
        KeyBinding::new("cmd-z", Undo, None),
        KeyBinding::new("cmd-shift-z", Redo, None),
    ]);
}

#[derive(Clone, Copy)]
enum TrimEdge {
    Left,
    Right,
}

struct TrimDrag {
    clip_id: u64,
    edge: TrimEdge,
    start_x: f32,
    original_in: f64,
    original_out: f64,
    asset_duration: f64,
}

pub(crate) struct Editor {
    project: Project,
    video: Option<Video>,
    loaded_clip_id: Option<u64>,
    selected_asset_id: Option<u64>,
    selected_clip_id: Option<u64>,
    playhead: f64,
    playing: bool,
    preview_refresh_ticks: u8,
    pixels_per_second: f32,
    next_id: u64,
    undo_stack: Vec<Project>,
    redo_stack: Vec<Project>,
    trim_drag: Option<TrimDrag>,
    timeline_scroll: ScrollHandle,
    exporting: bool,
    status: Option<String>,
    error: Option<String>,
    focus_handle: FocusHandle,
}

impl Editor {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let project = Project::load();
        let next_id = project.next_id();
        let selected_asset_id = project.assets.first().map(|asset| asset.id);
        let selected_clip_id = project.timeline.first().map(|clip| clip.id);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window);
        Self::start_updates(cx);

        let mut editor = Self {
            project,
            video: None,
            loaded_clip_id: None,
            selected_asset_id,
            selected_clip_id,
            playhead: 0.0,
            playing: false,
            preview_refresh_ticks: 0,
            pixels_per_second: 72.0,
            next_id,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            trim_drag: None,
            timeline_scroll: ScrollHandle::new(),
            exporting: false,
            status: None,
            error: None,
            focus_handle,
        };
        if !editor.project.timeline.is_empty() {
            editor.load_timeline_position(0.0, false);
        }
        editor
    }

    fn start_updates(cx: &mut Context<Self>) {
        cx.spawn(async move |editor, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(33))
                    .await;
                if editor
                    .update(cx, |editor, cx| {
                        let should_render = editor.playing || editor.preview_refresh_ticks > 0;
                        editor.preview_refresh_ticks =
                            editor.preview_refresh_ticks.saturating_sub(1);
                        editor.update_playback();
                        if should_render {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
    }

    fn update_playback(&mut self) {
        if !self.playing {
            return;
        }
        let Some(clip_id) = self.loaded_clip_id else {
            self.playing = false;
            return;
        };
        let Some(index) = self.project.clip_index(clip_id) else {
            self.playing = false;
            return;
        };
        let clip = &self.project.timeline[index];
        let Some(video) = self.video.as_ref() else {
            self.playing = false;
            return;
        };
        let source_position = video.position().as_secs_f64();
        self.playhead = self.project.timeline_start(index)
            + (source_position - clip.source_in).clamp(0.0, clip.duration());

        let tolerance = self
            .project
            .asset(clip.asset_id)
            .map(|asset| 0.5 / asset.framerate.max(1.0))
            .unwrap_or(1.0 / 60.0);
        if source_position + tolerance >= clip.source_out || video.eos() {
            if index + 1 < self.project.timeline.len() {
                let next_start = self.project.timeline_start(index + 1);
                self.load_timeline_position(next_start, true);
            } else {
                video.set_paused(true);
                self.playing = false;
                self.playhead = self.project.timeline_duration();
            }
        }
    }

    fn load_timeline_position(&mut self, position: f64, play: bool) {
        let duration = self.project.timeline_duration();
        let position = position.clamp(0.0, duration);
        let Some((index, local_position)) = self.project.clip_at_time(position) else {
            self.video = None;
            self.loaded_clip_id = None;
            self.playhead = 0.0;
            self.playing = false;
            return;
        };
        let clip = self.project.timeline[index].clone();
        let Some(asset) = self.project.asset(clip.asset_id).cloned() else {
            self.error = Some("The selected clip's source file is missing.".to_string());
            return;
        };
        let source_position = (clip.source_in + local_position).min(clip.source_out);

        if self.loaded_clip_id != Some(clip.id) {
            let Ok(url) = Url::from_file_path(&asset.path) else {
                self.error = Some(format!("Could not open {}", asset.path.display()));
                return;
            };
            match Video::new_with_options(
                &url,
                VideoOptions {
                    frame_buffer_capacity: Some(3),
                    looping: Some(false),
                    speed: Some(1.0),
                },
            ) {
                Ok(video) => {
                    self.video = Some(video);
                    self.loaded_clip_id = Some(clip.id);
                }
                Err(error) => {
                    self.error = Some(format!("Could not preview {}: {error}", asset.name));
                    return;
                }
            }
        }

        if let Some(video) = &self.video {
            let _ = video.seek(Duration::from_secs_f64(source_position), true);
            video.set_paused(!play);
        }
        self.playhead = position;
        self.playing = play;
        self.preview_refresh_ticks = 12;
        self.selected_clip_id = Some(clip.id);
        self.selected_asset_id = Some(clip.asset_id);
        self.error = None;
    }

    fn toggle_playback(&mut self) {
        if self.project.timeline.is_empty() {
            return;
        }
        if self.playing {
            if let Some(video) = &self.video {
                video.set_paused(true);
            }
            self.playing = false;
            return;
        }
        let duration = self.project.timeline_duration();
        let start = if self.playhead >= duration {
            0.0
        } else {
            self.playhead
        };
        self.load_timeline_position(start, true);
    }

    fn select_clip(&mut self, clip_id: u64) {
        let Some(index) = self.project.clip_index(clip_id) else {
            return;
        };
        self.selected_clip_id = Some(clip_id);
        self.selected_asset_id = Some(self.project.timeline[index].asset_id);
        self.load_timeline_position(self.project.timeline_start(index), false);
    }

    fn select_asset(&mut self, asset_id: u64) {
        self.selected_asset_id = Some(asset_id);
    }

    fn import_media(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Import video files".into()),
        });
        cx.spawn(async move |editor, cx| {
            let paths = match selection.await {
                Ok(Ok(Some(paths))) => paths,
                Ok(Ok(None)) => return,
                Ok(Err(error)) => {
                    editor
                        .update(cx, |editor, cx| {
                            editor.error = Some(format!("Could not import media: {error}"));
                            cx.notify();
                        })
                        .ok();
                    return;
                }
                Err(error) => {
                    editor
                        .update(cx, |editor, cx| {
                            editor.error = Some(format!("Import dialog failed: {error}"));
                            cx.notify();
                        })
                        .ok();
                    return;
                }
            };
            let results = cx
                .background_executor()
                .spawn(async move {
                    paths
                        .into_iter()
                        .map(|path| probe_media(&path, 0))
                        .collect::<Vec<_>>()
                })
                .await;
            editor
                .update(cx, |editor, cx| {
                    editor.finish_import(results);
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    fn finish_import(&mut self, results: Vec<Result<MediaAsset, String>>) {
        if results.is_empty() {
            return;
        }
        self.checkpoint();
        let mut imported = 0;
        let mut errors = Vec::new();
        for result in results {
            match result {
                Ok(mut asset) => {
                    let canonical = asset.path.clone();
                    let asset_id = if let Some(existing) = self
                        .project
                        .assets
                        .iter()
                        .find(|existing| existing.path == canonical)
                    {
                        existing.id
                    } else {
                        asset.id = self.take_id();
                        let id = asset.id;
                        self.project.assets.push(asset);
                        id
                    };
                    let duration = self.project.asset(asset_id).unwrap().duration;
                    let clip_id = self.take_id();
                    self.project.timeline.push(TimelineClip {
                        id: clip_id,
                        asset_id,
                        source_in: 0.0,
                        source_out: duration,
                    });
                    self.selected_asset_id = Some(asset_id);
                    self.selected_clip_id = Some(clip_id);
                    imported += 1;
                }
                Err(error) => errors.push(error),
            }
        }
        if imported == 0 {
            self.undo_stack.pop();
        } else {
            self.save_project();
            if self.video.is_none() {
                self.load_timeline_position(0.0, false);
            }
        }
        self.error = (!errors.is_empty()).then(|| errors.join("\n"));
        self.status = (imported > 0).then(|| format!("Imported {imported} clip(s)."));
    }

    fn add_selected_asset(&mut self) {
        let Some(asset_id) = self.selected_asset_id else {
            return;
        };
        let Some(duration) = self.project.asset(asset_id).map(|asset| asset.duration) else {
            return;
        };
        self.checkpoint();
        let id = self.take_id();
        self.project.timeline.push(TimelineClip {
            id,
            asset_id,
            source_in: 0.0,
            source_out: duration,
        });
        self.selected_clip_id = Some(id);
        self.save_project();
    }

    fn split_selected(&mut self) {
        let Some(clip_id) = self.selected_clip_id else {
            return;
        };
        let Some(index) = self.project.clip_index(clip_id) else {
            return;
        };
        let timeline_start = self.project.timeline_start(index);
        let clip = self.project.timeline[index].clone();
        let local = self.playhead - timeline_start;
        if local <= MIN_CLIP_DURATION || local >= clip.duration() - MIN_CLIP_DURATION {
            self.error =
                Some("Move the playhead inside the selected clip before splitting.".into());
            return;
        }
        let source_split = clip.source_in + local;
        self.checkpoint();
        self.project.timeline[index].source_out = source_split;
        let new_id = self.take_id();
        self.project.timeline.insert(
            index + 1,
            TimelineClip {
                id: new_id,
                asset_id: clip.asset_id,
                source_in: source_split,
                source_out: clip.source_out,
            },
        );
        self.selected_clip_id = Some(new_id);
        self.save_project();
        self.load_timeline_position(self.playhead, false);
    }

    fn delete_selected(&mut self) {
        let Some(clip_id) = self.selected_clip_id else {
            return;
        };
        let Some(index) = self.project.clip_index(clip_id) else {
            return;
        };
        self.checkpoint();
        self.project.timeline.remove(index);
        self.video = None;
        self.loaded_clip_id = None;
        self.playing = false;
        if self.project.timeline.is_empty() {
            self.selected_clip_id = None;
            self.playhead = 0.0;
        } else {
            let next_index = index.min(self.project.timeline.len() - 1);
            self.selected_clip_id = Some(self.project.timeline[next_index].id);
            let start = self.project.timeline_start(next_index);
            self.load_timeline_position(start, false);
        }
        self.save_project();
    }

    fn move_selected(&mut self, direction: i8) {
        let Some(clip_id) = self.selected_clip_id else {
            return;
        };
        let Some(index) = self.project.clip_index(clip_id) else {
            return;
        };
        let target = if direction < 0 {
            index.checked_sub(1)
        } else if index + 1 < self.project.timeline.len() {
            Some(index + 1)
        } else {
            None
        };
        let Some(target) = target else {
            return;
        };
        self.checkpoint();
        self.project.timeline.swap(index, target);
        self.save_project();
        let start = self.project.timeline_start(target);
        self.load_timeline_position(start, false);
    }

    fn begin_trim(&mut self, clip_id: u64, edge: TrimEdge, x: f32) {
        let Some(clip) = self.project.clip(clip_id).cloned() else {
            return;
        };
        let Some(asset_duration) = self
            .project
            .asset(clip.asset_id)
            .map(|asset| asset.duration)
        else {
            return;
        };
        if let Some(video) = &self.video {
            video.set_paused(true);
        }
        self.playing = false;
        self.selected_clip_id = Some(clip_id);
        self.checkpoint();
        self.trim_drag = Some(TrimDrag {
            clip_id,
            edge,
            start_x: x,
            original_in: clip.source_in,
            original_out: clip.source_out,
            asset_duration,
        });
    }

    fn update_trim(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !event.dragging() {
            return;
        }
        let Some(drag) = self.trim_drag.as_ref() else {
            return;
        };
        let delta =
            (f32::from(event.position.x) - drag.start_x) as f64 / self.pixels_per_second as f64;
        let Some(index) = self.project.clip_index(drag.clip_id) else {
            return;
        };
        match drag.edge {
            TrimEdge::Left => {
                self.project.timeline[index].source_in =
                    (drag.original_in + delta).clamp(0.0, drag.original_out - MIN_CLIP_DURATION);
            }
            TrimEdge::Right => {
                self.project.timeline[index].source_out = (drag.original_out + delta)
                    .clamp(drag.original_in + MIN_CLIP_DURATION, drag.asset_duration);
            }
        }
        cx.notify();
    }

    fn finish_trim(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.trim_drag.take().is_some() {
            self.save_project();
            if let Some(clip_id) = self.selected_clip_id
                && let Some(index) = self.project.clip_index(clip_id)
            {
                self.load_timeline_position(self.project.timeline_start(index), false);
            }
            cx.notify();
        }
    }

    fn export(&mut self, cx: &mut Context<Self>) {
        if self.project.timeline.is_empty() || self.exporting {
            return;
        }
        let directory =
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
        let selection = cx.prompt_for_new_path(&directory, Some("opencut-export.mp4"));
        cx.spawn(async move |editor, cx| {
            let path = match selection.await {
                Ok(Ok(Some(mut path))) => {
                    if !path
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"))
                    {
                        path.set_extension("mp4");
                    }
                    path
                }
                Ok(Ok(None)) => return,
                Ok(Err(error)) => {
                    editor
                        .update(cx, |editor, cx| {
                            editor.error = Some(format!("Could not open export dialog: {error}"));
                            cx.notify();
                        })
                        .ok();
                    return;
                }
                Err(error) => {
                    editor
                        .update(cx, |editor, cx| {
                            editor.error = Some(format!("Export dialog failed: {error}"));
                            cx.notify();
                        })
                        .ok();
                    return;
                }
            };
            let project = match editor.update(cx, |editor, cx| {
                editor.exporting = true;
                editor.status = Some("Exporting…".to_string());
                editor.error = None;
                cx.notify();
                editor.project.clone()
            }) {
                Ok(project) => project,
                Err(_) => return,
            };
            let export_path = path.clone();
            let result = cx
                .background_executor()
                .spawn(async move { export_project(&project, &export_path) })
                .await;
            editor
                .update(cx, |editor, cx| {
                    editor.exporting = false;
                    match result {
                        Ok(()) => {
                            editor.status = Some(format!("Exported {}", path.display()));
                            editor.error = None;
                        }
                        Err(error) => {
                            editor.status = None;
                            editor.error = Some(error);
                        }
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    fn checkpoint(&mut self) {
        self.undo_stack.push(self.project.clone());
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    fn undo(&mut self) {
        let Some(project) = self.undo_stack.pop() else {
            return;
        };
        self.redo_stack
            .push(std::mem::replace(&mut self.project, project));
        self.reset_after_history_change();
    }

    fn redo(&mut self) {
        let Some(project) = self.redo_stack.pop() else {
            return;
        };
        self.undo_stack
            .push(std::mem::replace(&mut self.project, project));
        self.reset_after_history_change();
    }

    fn reset_after_history_change(&mut self) {
        self.video = None;
        self.loaded_clip_id = None;
        self.playing = false;
        self.playhead = 0.0;
        self.selected_clip_id = self.project.timeline.first().map(|clip| clip.id);
        if !self.project.timeline.is_empty() {
            self.load_timeline_position(0.0, false);
        }
        self.next_id = self.next_id.max(self.project.next_id());
        self.save_project();
    }

    fn save_project(&mut self) {
        if let Err(error) = self.project.save() {
            self.error = Some(format!("Could not autosave project: {error}"));
        }
    }

    fn take_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn zoom(&mut self, factor: f32) {
        self.pixels_per_second = (self.pixels_per_second * factor).clamp(24.0, 240.0);
    }

    fn seek_from_timeline_x(&mut self, x: f32) {
        let scroll_x: f32 = self.timeline_scroll.offset().x.into();
        let content_x = x - MEDIA_PANEL_WIDTH - scroll_x - TIMELINE_PADDING;
        let position = content_x as f64 / self.pixels_per_second as f64;
        self.load_timeline_position(position, false);
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

    fn action_delete_selected(
        &mut self,
        _: &DeleteSelected,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.delete_selected();
        cx.notify();
    }

    fn action_split_clip(&mut self, _: &SplitClip, _: &mut Window, cx: &mut Context<Self>) {
        self.split_selected();
        cx.notify();
    }

    fn action_undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
        self.undo();
        cx.notify();
    }

    fn action_redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
        self.redo();
        cx.notify();
    }
}

fn format_time(seconds: f64) -> String {
    let total = seconds.max(0.0).round() as u64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}
