use crate::video_backend::{Video, VideoOptions, video};
use gpui::{
    App, Context, CursorStyle, FocusHandle, KeyBinding, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ObjectFit, PathPromptOptions, Render, ScrollHandle, Window,
    actions, div, img, prelude::*, px, rgb,
};
use std::{
    collections::HashSet,
    path::PathBuf,
    time::{Duration, Instant},
};
use url::Url;

mod export;
mod model;
mod view;
mod workspace;

use export::export_project;
use model::{
    MIN_CLIP_DURATION, MediaAsset, MediaKind, Project, TimelineClip, probe_image, probe_media,
};
use workspace::{FileTreeEntry, load_project_root, save_project_root, visible_tree};

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
    [
        TogglePlayback,
        DeleteSelected,
        SplitClip,
        Undo,
        Redo,
        RevealInFinder,
        OpenInDefaultApp
    ]
);

pub(crate) fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("space", TogglePlayback, None),
        KeyBinding::new("backspace", DeleteSelected, None),
        KeyBinding::new("delete", DeleteSelected, None),
        KeyBinding::new("cmd-b", SplitClip, None),
        KeyBinding::new("cmd-z", Undo, None),
        KeyBinding::new("cmd-shift-z", Redo, None),
        KeyBinding::new("cmd-alt-r", RevealInFinder, None),
        KeyBinding::new("ctrl-shift-enter", OpenInDefaultApp, None),
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

#[derive(Clone)]
struct FileContextMenu {
    relative_path: PathBuf,
    x: f32,
    y: f32,
}

pub(crate) struct Editor {
    project_root: PathBuf,
    project: Project,
    file_tree: Vec<FileTreeEntry>,
    expanded_directories: HashSet<PathBuf>,
    selected_file: Option<PathBuf>,
    file_context_menu: Option<FileContextMenu>,
    last_tree_scan: Instant,
    video: Option<Video>,
    loaded_clip_id: Option<u64>,
    still_playback_started: Option<Instant>,
    still_playback_origin: f64,
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
        let project_root = load_project_root();
        let expanded_directories = HashSet::new();
        let file_tree = visible_tree(&project_root, &expanded_directories).unwrap_or_default();
        let project = Project::load(&project_root);
        let next_id = project.next_id();
        let selected_asset_id = project.assets.first().map(|asset| asset.id);
        let selected_clip_id = project.timeline.first().map(|clip| clip.id);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window);
        Self::start_updates(cx);

        let mut editor = Self {
            project_root,
            project,
            file_tree,
            expanded_directories,
            selected_file: None,
            file_context_menu: None,
            last_tree_scan: Instant::now(),
            video: None,
            loaded_clip_id: None,
            still_playback_started: None,
            still_playback_origin: 0.0,
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
                        let refresh_tree =
                            editor.last_tree_scan.elapsed() >= Duration::from_secs(1);
                        let should_render =
                            editor.playing || editor.preview_refresh_ticks > 0 || refresh_tree;
                        editor.preview_refresh_ticks =
                            editor.preview_refresh_ticks.saturating_sub(1);
                        if refresh_tree {
                            editor.refresh_file_tree();
                        }
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
        let media_kind = self
            .project
            .asset(clip.asset_id)
            .map(|asset| asset.kind)
            .unwrap_or_default();

        if media_kind == MediaKind::Image {
            let Some(started) = self.still_playback_started else {
                self.playing = false;
                return;
            };
            let clip_end = self.project.timeline_start(index) + clip.duration();
            self.playhead =
                (self.still_playback_origin + started.elapsed().as_secs_f64()).clamp(0.0, clip_end);
            if self.playhead + 1.0 / 120.0 >= clip_end {
                if index + 1 < self.project.timeline.len() {
                    let next_start = self.project.timeline_start(index + 1);
                    self.load_timeline_position(next_start, true);
                } else {
                    self.still_playback_started = None;
                    self.playing = false;
                    self.playhead = self.project.timeline_duration();
                }
            }
            return;
        }

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
        self.selected_file = None;
        self.file_context_menu = None;
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

        if asset.kind == MediaKind::Image {
            if let Some(video) = &self.video {
                video.set_paused(true);
            }
            self.video = None;
            self.loaded_clip_id = Some(clip.id);
            self.still_playback_origin = position;
            self.still_playback_started = play.then(Instant::now);
        } else if self.loaded_clip_id != Some(clip.id) {
            let source_path = self.project_root.join(&asset.path);
            let Ok(url) = Url::from_file_path(&source_path) else {
                self.error = Some(format!("Could not open {}", source_path.display()));
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
                    self.still_playback_started = None;
                }
                Err(error) => {
                    self.error = Some(format!("Could not preview {}: {error}", asset.name));
                    return;
                }
            }
        }

        if asset.kind == MediaKind::Video
            && let Some(video) = &self.video
        {
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
            self.still_playback_started = None;
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

    fn open_project_folder(&mut self, cx: &mut Context<Self>) {
        self.error = None;
        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open project folder".into()),
        });
        cx.spawn(async move |editor, cx| {
            let root = match selection.await {
                Ok(Ok(Some(paths))) => paths.into_iter().next(),
                Ok(Ok(None)) => return,
                Ok(Err(error)) => {
                    editor
                        .update(cx, |editor, cx| {
                            editor.error = Some(format!("Could not open project folder: {error}"));
                            cx.notify();
                        })
                        .ok();
                    return;
                }
                Err(error) => {
                    editor
                        .update(cx, |editor, cx| {
                            editor.error = Some(format!("Folder dialog failed: {error}"));
                            cx.notify();
                        })
                        .ok();
                    return;
                }
            };
            if let Some(root) = root {
                editor
                    .update(cx, |editor, cx| {
                        editor.set_project_root(root);
                        cx.notify();
                    })
                    .ok();
            }
        })
        .detach();
    }

    fn set_project_root(&mut self, root: PathBuf) {
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        self.video = None;
        self.loaded_clip_id = None;
        self.still_playback_started = None;
        self.playing = false;
        self.playhead = 0.0;
        self.project_root = root;
        self.project = Project::load(&self.project_root);
        self.next_id = self.project.next_id();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.expanded_directories.clear();
        self.selected_file = None;
        self.file_context_menu = None;
        self.selected_asset_id = self.project.assets.first().map(|asset| asset.id);
        self.selected_clip_id = self.project.timeline.first().map(|clip| clip.id);
        self.refresh_file_tree();
        if let Err(error) = save_project_root(&self.project_root) {
            self.error = Some(error);
        } else {
            self.error = None;
        }
        if !self.project.timeline.is_empty() {
            self.load_timeline_position(0.0, false);
        }
    }

    fn refresh_file_tree(&mut self) {
        self.last_tree_scan = Instant::now();
        match visible_tree(&self.project_root, &self.expanded_directories) {
            Ok(entries) => self.file_tree = entries,
            Err(error) => self.error = Some(error),
        }
    }

    fn toggle_directory(&mut self, relative_path: PathBuf) {
        if !self.expanded_directories.remove(&relative_path) {
            self.expanded_directories.insert(relative_path);
        }
        self.refresh_file_tree();
    }

    fn select_file(&mut self, relative_path: PathBuf) {
        if workspace::is_image_path(&relative_path) {
            if let Some(video) = &self.video {
                video.set_paused(true);
            }
            self.playing = false;
            self.still_playback_started = None;
            self.preview_refresh_ticks = 2;
        }
        self.selected_file = Some(relative_path.clone());
        self.selected_asset_id = self
            .project
            .assets
            .iter()
            .find(|asset| asset.path == relative_path)
            .map(|asset| asset.id);
    }

    fn show_file_context_menu(
        &mut self,
        relative_path: PathBuf,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        self.selected_file = Some(relative_path.clone());
        self.file_context_menu = Some(FileContextMenu {
            relative_path,
            x: event.position.x.into(),
            y: event.position.y.into(),
        });
        cx.stop_propagation();
        cx.notify();
    }

    fn dismiss_file_context_menu(&mut self) {
        self.file_context_menu = None;
    }

    fn reveal_selected_file(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.file_action_path() else {
            return;
        };
        self.file_context_menu = None;
        cx.reveal_path(&path);
    }

    fn open_selected_file_in_default_app(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.file_action_path() else {
            return;
        };
        self.file_context_menu = None;
        cx.open_with_system(&path);
    }

    fn file_action_path(&self) -> Option<PathBuf> {
        let relative_path = self
            .file_context_menu
            .as_ref()
            .map(|menu| &menu.relative_path)
            .or(self.selected_file.as_ref())?;
        Some(self.project_root.join(relative_path))
    }

    fn add_file_to_timeline(&mut self, relative_path: PathBuf, cx: &mut Context<Self>) {
        if let Some(asset_id) = self
            .project
            .assets
            .iter()
            .find(|asset| asset.path == relative_path)
            .map(|asset| asset.id)
        {
            self.append_asset_clip(asset_id);
            cx.notify();
            return;
        }

        let project_root = self.project_root.clone();
        let absolute_path = project_root.join(&relative_path);
        let is_image = workspace::is_image_path(&relative_path);
        self.status = Some(format!("Inspecting {}…", relative_path.display()));
        cx.spawn(async move |editor, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    if is_image {
                        probe_image(&absolute_path, 0)
                    } else {
                        probe_media(&absolute_path, 0)
                    }
                })
                .await;
            editor
                .update(cx, |editor, cx| {
                    if editor.project_root != project_root {
                        return;
                    }
                    match result {
                        Ok(mut asset) => {
                            if let Some(asset_id) = editor
                                .project
                                .assets
                                .iter()
                                .find(|asset| asset.path == relative_path)
                                .map(|asset| asset.id)
                            {
                                editor.append_asset_clip(asset_id);
                                editor.status = Some("Added media to timeline.".to_string());
                                cx.notify();
                                return;
                            }
                            editor.checkpoint();
                            asset.id = editor.take_id();
                            asset.path = relative_path.clone();
                            let asset_id = asset.id;
                            editor.project.assets.push(asset);
                            editor.append_asset_clip_without_checkpoint(asset_id);
                            editor.selected_file = Some(relative_path);
                            editor.save_project();
                            editor.status = Some("Added media to timeline.".to_string());
                            editor.error = None;
                        }
                        Err(error) => editor.error = Some(error),
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    fn append_asset_clip(&mut self, asset_id: u64) {
        self.checkpoint();
        self.append_asset_clip_without_checkpoint(asset_id);
        self.save_project();
    }

    fn append_asset_clip_without_checkpoint(&mut self, asset_id: u64) {
        let Some(duration) = self.project.asset(asset_id).map(|asset| asset.duration) else {
            return;
        };
        let id = self.take_id();
        self.project.timeline.push(TimelineClip {
            id,
            asset_id,
            source_in: 0.0,
            source_out: duration,
        });
        self.selected_asset_id = Some(asset_id);
        self.selected_clip_id = Some(id);
        if self.loaded_clip_id.is_none() {
            self.load_timeline_position(0.0, false);
        }
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
        self.still_playback_started = None;
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
        self.still_playback_started = None;
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
        let directory = self.project_root.clone();
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
            let (project, project_root) = match editor.update(cx, |editor, cx| {
                editor.exporting = true;
                editor.status = Some("Exporting…".to_string());
                editor.error = None;
                cx.notify();
                (editor.project.clone(), editor.project_root.clone())
            }) {
                Ok(project) => project,
                Err(_) => return,
            };
            let export_path = path.clone();
            let result = cx
                .background_executor()
                .spawn(async move { export_project(&project, &project_root, &export_path) })
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
        self.still_playback_started = None;
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
        if let Err(error) = self.project.save(&self.project_root) {
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

    fn action_reveal_in_finder(
        &mut self,
        _: &RevealInFinder,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reveal_selected_file(cx);
        cx.notify();
    }

    fn action_open_in_default_app(
        &mut self,
        _: &OpenInDefaultApp,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_selected_file_in_default_app(cx);
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
