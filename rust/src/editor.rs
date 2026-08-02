use crate::video_backend::Video;
use gpui::{
    App, Context, CursorStyle, Entity, FocusHandle, KeyBinding, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, ObjectFit, PathPromptOptions, Render, ScrollHandle,
    ScrollWheelEvent, Window, actions, div, img, prelude::*, px, rgb,
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    time::{Duration, Instant},
};

mod explorer;
mod explorer_filter;
mod export;
mod media_cache;
mod model;
mod preview;
mod preview_audio;
mod properties;
mod settings;
mod timeline;
mod track;
mod view;
mod workspace;

use crate::playback_view::{DragPhase, PlaybackViewDelegate};
use explorer::FileContextMenu;
use explorer_filter::ExplorerFilter;
use export::export_project;
use model::{
    FrameRate, MediaAsset, MediaKind, Project, TimelineClip, TimelineMarker, TimelineTime,
    TimelineTrack, TrackKind, probe_audio, probe_image, probe_media,
};
use preview::PreviewTarget;
use preview_audio::AudioPreview;
use workspace::{FileTreeEntry, load_project_root, save_project_root, visible_tree};

const MEDIA_PANEL_WIDTH: f32 = 340.0;
const PROPERTIES_PANEL_WIDTH: f32 = 292.0;
const TOPBAR_HEIGHT: f32 = 64.0;
const TIMELINE_HEIGHT: f32 = 420.0;
const TIMELINE_HEADER_HEIGHT: f32 = 46.0;
const TIMELINE_PADDING: f32 = 20.0;
const TRACK_HEADER_WIDTH: f32 = 190.0;
const TRACK_HEIGHT: f32 = 74.0;
const RULER_HEIGHT: f32 = 28.0;
const SNAP_DISTANCE_PX: f32 = 8.0;
const MIN_TIMELINE_PIXELS_PER_SECOND: f32 = 1.0;
const MAX_TIMELINE_PIXELS_PER_SECOND: f32 = 1000.0;
const SCRUB_SEEK_INTERVAL: Duration = Duration::from_millis(50);

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
const EDITOR_KEY_CONTEXT: &str = "Editor";
const EDITOR_SHORTCUT_CONTEXT: &str = "!ExplorerFilter";

actions!(
    opencut_editor,
    [
        TogglePlayback,
        StepBackwardFrame,
        StepForwardFrame,
        DeleteSelected,
        SplitClip,
        Undo,
        Redo,
        DuplicateSelected,
        AddMarker,
        ToggleFullscreen,
        ExitFullscreen,
        ToggleInspector,
        RevealInFinder,
        OpenInDefaultApp
    ]
);

pub(crate) fn bind_keys(cx: &mut App) {
    explorer_filter::bind_keys(cx);
    cx.bind_keys([
        KeyBinding::new("space", TogglePlayback, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("left", StepBackwardFrame, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("right", StepForwardFrame, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("backspace", DeleteSelected, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("delete", DeleteSelected, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("cmd-b", SplitClip, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("cmd-z", Undo, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("cmd-shift-z", Redo, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("cmd-d", DuplicateSelected, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("shift-m", AddMarker, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("f", ToggleFullscreen, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("escape", ExitFullscreen, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("cmd-alt-i", ToggleInspector, None),
        KeyBinding::new("cmd-alt-r", RevealInFinder, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new(
            "ctrl-shift-enter",
            OpenInDefaultApp,
            Some(EDITOR_SHORTCUT_CONTEXT),
        ),
    ]);
    cx.on_action::<ToggleInspector>(|_, cx| {
        let Some(window) = cx.active_window() else {
            return;
        };
        let _ = window.update(cx, |_, window, cx| window.toggle_inspector(cx));
    });
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
    original_in: TimelineTime,
    original_out: TimelineTime,
    original_timeline_start: TimelineTime,
    asset_duration: TimelineTime,
    changed: bool,
}

struct ClipMoveDrag {
    clip_id: u64,
    start_x: f32,
    original_timeline_start: TimelineTime,
    original_track_id: u64,
    changed: bool,
}

#[derive(Clone, Copy)]
struct MarqueeSelection {
    start_x: f32,
    start_y: f32,
    current_x: f32,
    current_y: f32,
}

pub(crate) struct Editor {
    project_root: PathBuf,
    project: Project,
    file_tree: Vec<FileTreeEntry>,
    expanded_directories: HashSet<PathBuf>,
    explorer_root_expanded: bool,
    explorer_filter: Entity<ExplorerFilter>,
    explorer_search_query: Option<String>,
    explorer_search_results: Vec<FileTreeEntry>,
    explorer_search_pending: bool,
    explorer_scroll: ScrollHandle,
    selected_file: Option<PathBuf>,
    file_context_menu: Option<FileContextMenu>,
    preview_target: PreviewTarget,
    media_cache_jobs: HashSet<u64>,
    media_cache_ready: HashSet<u64>,
    last_tree_scan: Instant,
    video: Option<Video>,
    standalone_audio: Option<AudioPreview>,
    audio_previews: HashMap<u64, AudioPreview>,
    loaded_clip_id: Option<u64>,
    still_playback_started: Option<Instant>,
    still_playback_origin: TimelineTime,
    selected_asset_id: Option<u64>,
    selected_clip_id: Option<u64>,
    selected_clip_ids: HashSet<u64>,
    playhead: TimelineTime,
    playing: bool,
    preview_volume: f64,
    preview_volume_open: bool,
    preview_is_scrubbing: bool,
    preview_is_adjusting_volume: bool,
    preview_resume_after_scrub: bool,
    preview_scrub_fraction: Option<f32>,
    preview_pending_seek_started: Option<Instant>,
    preview_last_scrub_seek: Option<Instant>,
    preview_refresh_ticks: u8,
    settings_open: bool,
    pixels_per_second: f32,
    next_id: u64,
    undo_stack: Vec<Project>,
    redo_stack: Vec<Project>,
    trim_drag: Option<TrimDrag>,
    clip_move_drag: Option<ClipMoveDrag>,
    marquee_selection: Option<MarqueeSelection>,
    is_scrubbing_playhead: bool,
    last_playhead_scrub_seek: Option<Instant>,
    timeline_scroll: ScrollHandle,
    timeline_vertical_scroll: ScrollHandle,
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
        let selected_clip_id = project.clips.first().map(|clip| clip.id);
        let selected_clip_ids = selected_clip_id.into_iter().collect();
        let focus_handle = cx.focus_handle();
        let explorer_filter = cx.new(|cx| ExplorerFilter::new(focus_handle.clone(), cx));
        cx.observe(&explorer_filter, |editor, _, cx| {
            editor.schedule_explorer_search(cx);
            cx.notify();
        })
        .detach();
        focus_handle.focus(window);
        Self::start_updates(cx);

        let mut editor = Self {
            project_root,
            project,
            file_tree,
            expanded_directories,
            explorer_root_expanded: true,
            explorer_filter,
            explorer_search_query: None,
            explorer_search_results: Vec::new(),
            explorer_search_pending: false,
            explorer_scroll: ScrollHandle::new(),
            selected_file: None,
            file_context_menu: None,
            preview_target: PreviewTarget::Timeline,
            media_cache_jobs: HashSet::new(),
            media_cache_ready: HashSet::new(),
            last_tree_scan: Instant::now(),
            video: None,
            standalone_audio: None,
            audio_previews: HashMap::new(),
            loaded_clip_id: None,
            still_playback_started: None,
            still_playback_origin: TimelineTime::ZERO,
            selected_asset_id,
            selected_clip_id,
            selected_clip_ids,
            playhead: TimelineTime::ZERO,
            playing: false,
            preview_volume: 1.0,
            preview_volume_open: false,
            preview_is_scrubbing: false,
            preview_is_adjusting_volume: false,
            preview_resume_after_scrub: false,
            preview_scrub_fraction: None,
            preview_pending_seek_started: None,
            preview_last_scrub_seek: None,
            preview_refresh_ticks: 0,
            settings_open: false,
            pixels_per_second: 72.0,
            next_id,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            trim_drag: None,
            clip_move_drag: None,
            marquee_selection: None,
            is_scrubbing_playhead: false,
            last_playhead_scrub_seek: None,
            timeline_scroll: ScrollHandle::new(),
            timeline_vertical_scroll: ScrollHandle::new(),
            exporting: false,
            status: None,
            error: None,
            focus_handle,
        };
        if !editor.project.clips.is_empty() {
            editor.load_timeline_position(TimelineTime::ZERO, false);
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
                        let file_preview_playing = match editor.preview_target {
                            PreviewTarget::VideoFile(_) => {
                                editor.video.as_ref().is_some_and(|video| !video.paused())
                            }
                            PreviewTarget::AudioFile(_) => editor
                                .standalone_audio
                                .as_ref()
                                .is_some_and(AudioPreview::playing),
                            _ => false,
                        };
                        let pinch_zoomed = editor.apply_timeline_pinch();
                        let should_render = editor.playing
                            || file_preview_playing
                            || editor.preview_refresh_ticks > 0
                            || refresh_tree
                            || pinch_zoomed;
                        editor.preview_refresh_ticks =
                            editor.preview_refresh_ticks.saturating_sub(1);
                        if refresh_tree {
                            editor.refresh_file_tree();
                            editor.schedule_missing_media_cache(cx);
                        }
                        editor.update_playback();
                        editor.reconcile_preview_seek();
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
                        editor.set_project_root(root, cx);
                        cx.notify();
                    })
                    .ok();
            }
        })
        .detach();
    }

    fn set_project_root(&mut self, root: PathBuf, cx: &mut Context<Self>) {
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        self.video = None;
        self.standalone_audio = None;
        self.audio_previews.clear();
        self.media_cache_jobs.clear();
        self.media_cache_ready.clear();
        self.loaded_clip_id = None;
        self.still_playback_started = None;
        self.playing = false;
        self.playhead = TimelineTime::ZERO;
        self.project_root = root;
        self.project = Project::load(&self.project_root);
        self.next_id = self.project.next_id();
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.expanded_directories.clear();
        self.explorer_root_expanded = true;
        self.explorer_search_query = None;
        self.explorer_search_results.clear();
        self.explorer_search_pending = false;
        self.explorer_filter
            .update(cx, |filter, cx| filter.clear(cx));
        self.selected_file = None;
        self.file_context_menu = None;
        self.preview_target = PreviewTarget::Timeline;
        self.selected_asset_id = self.project.assets.first().map(|asset| asset.id);
        self.select_only_clip(self.project.clips.first().map(|clip| clip.id));
        self.refresh_file_tree();
        if let Err(error) = save_project_root(&self.project_root) {
            self.error = Some(error);
        } else {
            self.error = None;
        }
        if !self.project.clips.is_empty() {
            self.load_timeline_position(TimelineTime::ZERO, false);
        }
    }

    /// Refreshes which assets already have artwork on disk and starts one missing job.
    ///
    /// Runs on the file-tree tick rather than during rendering so the timeline can read
    /// `media_cache_ready` without touching the filesystem on every frame.
    fn schedule_missing_media_cache(&mut self, cx: &mut Context<Self>) {
        self.media_cache_ready = self
            .project
            .assets
            .iter()
            .filter(|asset| media_cache::cache_is_ready(&self.project_root, asset))
            .map(|asset| asset.id)
            .collect();
        let Some(asset) = self
            .project
            .assets
            .iter()
            .find(|asset| {
                !self.media_cache_jobs.contains(&asset.id)
                    && !self.media_cache_ready.contains(&asset.id)
            })
            .cloned()
        else {
            return;
        };
        self.media_cache_jobs.insert(asset.id);
        let project_root = self.project_root.clone();
        cx.spawn(async move |editor, cx| {
            let cache_root = project_root.clone();
            let result = cx
                .background_executor()
                .spawn(async move { media_cache::generate(&cache_root, &asset) })
                .await;
            editor
                .update(cx, |editor, cx| {
                    if editor.project_root == project_root {
                        if let Err(error) = result {
                            eprintln!("Media cache: {error}");
                        }
                        cx.notify();
                    }
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
        let target_kind = if self
            .project
            .asset(asset_id)
            .is_some_and(|asset| asset.kind == MediaKind::Audio)
        {
            TrackKind::Audio
        } else {
            TrackKind::Video
        };
        let track_id = self
            .project
            .tracks
            .iter()
            .find(|track| track.kind == target_kind && !track.locked)
            .map(|track| track.id)
            .unwrap_or_else(|| {
                let id = self.take_id();
                let number = self
                    .project
                    .tracks
                    .iter()
                    .filter(|track| track.kind == target_kind)
                    .count()
                    + 1;
                let prefix = match target_kind {
                    TrackKind::Video => "Video",
                    TrackKind::Audio => "Audio",
                };
                self.project.tracks.push(TimelineTrack {
                    id,
                    name: format!("{prefix} {number}"),
                    kind: target_kind,
                    locked: false,
                    muted: false,
                    visible: true,
                });
                id
            });
        let id = self.take_id();
        self.project.clips.push(TimelineClip {
            id,
            track_id,
            asset_id: Some(asset_id),
            timeline_start: self.project.content_duration(),
            source_in: TimelineTime::ZERO,
            source_out: self.project.ceil_time(duration),
        });
        self.selected_asset_id = Some(asset_id);
        self.select_only_clip(Some(id));
        if self.loaded_clip_id.is_none() {
            self.load_timeline_position(TimelineTime::ZERO, false);
        }
    }

    fn split_selected(&mut self) {
        let Some(clip_id) = self.selected_clip_id else {
            return;
        };
        let Some(index) = self.project.clip_index(clip_id) else {
            return;
        };
        if self.clip_locked(clip_id) {
            return;
        }
        let timeline_start = self.project.clips[index].timeline_start;
        let clip = self.project.clips[index].clone();
        let local = self.playhead - timeline_start;
        if local < TimelineTime::ONE_FRAME || local > clip.duration() - TimelineTime::ONE_FRAME {
            self.error =
                Some("Move the playhead inside the selected clip before splitting.".into());
            return;
        }
        let source_split = clip.source_in + local;
        self.checkpoint();
        self.project.clips[index].source_out = source_split;
        let new_id = self.take_id();
        self.project.clips.push(TimelineClip {
            id: new_id,
            track_id: clip.track_id,
            asset_id: clip.asset_id,
            timeline_start: self.playhead,
            source_in: source_split,
            source_out: clip.source_out,
        });
        self.select_only_clip(Some(new_id));
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
        if self.clip_locked(clip_id) {
            return;
        }
        self.checkpoint();
        self.project.clips.remove(index);
        self.preview_target = PreviewTarget::Timeline;
        self.video = None;
        self.standalone_audio = None;
        self.audio_previews.clear();
        self.loaded_clip_id = None;
        self.still_playback_started = None;
        self.playing = false;
        if self.project.clips.is_empty() {
            self.select_only_clip(None);
            self.playhead = TimelineTime::ZERO;
        } else {
            let next_index = index.min(self.project.clips.len() - 1);
            self.select_only_clip(Some(self.project.clips[next_index].id));
            let start = self.project.clips[next_index].timeline_start;
            self.load_timeline_position(start, false);
        }
        self.save_project();
    }

    fn move_selected(&mut self, direction: i8) {
        let Some(clip_id) = self.selected_clip_id else {
            return;
        };
        if self.clip_locked(clip_id) {
            return;
        }
        let Some(clip) = self.project.clip(clip_id) else {
            return;
        };
        let desired_start = (clip.timeline_start + TimelineTime::from_frames(i64::from(direction)))
            .max(TimelineTime::ZERO);
        let start = self.project.nearest_available_start(
            clip.track_id,
            Some(clip_id),
            desired_start,
            clip.duration(),
        );
        if start == clip.timeline_start {
            return;
        }
        self.checkpoint();
        let Some(clip) = self.project.clip_mut(clip_id) else {
            return;
        };
        clip.timeline_start = start;
        self.save_project();
        self.load_timeline_position(start, false);
    }

    fn duplicate_selected(&mut self) {
        let Some(clip_id) = self.selected_clip_id else {
            return;
        };
        let Some(mut clip) = self.project.clip(clip_id).cloned() else {
            return;
        };
        if self
            .project
            .track(clip.track_id)
            .is_some_and(|track| track.locked)
        {
            return;
        }
        let start = self.project.nearest_available_start(
            clip.track_id,
            None,
            clip.timeline_end(),
            clip.duration(),
        );
        self.checkpoint();
        clip.id = self.take_id();
        clip.timeline_start = start;
        self.select_only_clip(Some(clip.id));
        self.project.clips.push(clip);
        self.save_project();
    }

    fn add_marker(&mut self) {
        self.checkpoint();
        let id = self.take_id();
        self.project.markers.push(TimelineMarker {
            id,
            time: self.playhead,
            label: format!("Marker {}", self.project.markers.len() + 1),
        });
        self.save_project();
    }

    fn add_track(&mut self, kind: TrackKind) {
        self.checkpoint();
        let number = self
            .project
            .tracks
            .iter()
            .filter(|track| track.kind == kind)
            .count()
            + 1;
        let prefix = match kind {
            TrackKind::Video => "Video",
            TrackKind::Audio => "Audio",
        };
        let id = self.take_id();
        self.project.tracks.push(TimelineTrack {
            id,
            name: format!("{prefix} {number}"),
            kind,
            locked: false,
            muted: false,
            visible: true,
        });
        self.save_project();
    }

    fn toggle_track_lock(&mut self, track_id: u64) {
        self.checkpoint();
        if let Some(track) = self.project.track_mut(track_id) {
            track.locked = !track.locked;
        }
        self.save_project();
    }

    fn toggle_track_visibility(&mut self, track_id: u64) {
        self.checkpoint();
        if let Some(track) = self.project.track_mut(track_id) {
            track.visible = !track.visible;
        }
        self.save_project();
        self.load_timeline_position(self.playhead, false);
    }

    fn toggle_track_mute(&mut self, track_id: u64) {
        self.checkpoint();
        if let Some(track) = self.project.track_mut(track_id) {
            track.muted = !track.muted;
        }
        self.save_project();
        let muted = self
            .project
            .track(track_id)
            .is_some_and(|track| track.muted);
        if self
            .loaded_clip_id
            .and_then(|id| self.project.clip(id))
            .is_some_and(|clip| clip.track_id == track_id)
            && let Some(video) = &self.video
        {
            video.set_muted(muted);
        }
        self.sync_audio_previews(self.playhead, self.playing);
    }

    fn move_track(&mut self, track_id: u64, direction: i8) {
        let Some(index) = self
            .project
            .tracks
            .iter()
            .position(|track| track.id == track_id)
        else {
            return;
        };
        let target = if direction < 0 {
            index.checked_sub(1)
        } else if index + 1 < self.project.tracks.len() {
            Some(index + 1)
        } else {
            None
        };
        let Some(target) = target else {
            return;
        };
        self.checkpoint();
        self.project.tracks.swap(index, target);
        self.save_project();
        self.load_timeline_position(self.playhead, false);
    }

    fn delete_track(&mut self, track_id: u64) {
        let Some(index) = self
            .project
            .tracks
            .iter()
            .position(|track| track.id == track_id)
        else {
            return;
        };
        if self.project.tracks[index].locked {
            return;
        }
        self.checkpoint();
        self.project.tracks.remove(index);
        self.project.clips.retain(|clip| clip.track_id != track_id);
        self.selected_clip_ids
            .retain(|id| self.project.clip(*id).is_some());
        if self
            .selected_clip_id
            .is_some_and(|id| self.project.clip(id).is_none())
        {
            self.selected_clip_id = self
                .project
                .clips
                .iter()
                .find(|clip| self.selected_clip_ids.contains(&clip.id))
                .map(|clip| clip.id);
        }
        self.save_project();
        self.load_timeline_position(self.playhead, false);
    }

    fn select_only_clip(&mut self, clip_id: Option<u64>) {
        self.selected_clip_ids.clear();
        if let Some(clip_id) = clip_id {
            self.selected_clip_ids.insert(clip_id);
        }
        self.selected_clip_id = clip_id;
    }

    fn begin_marquee_selection(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (x, y) = Self::timeline_pointer_position(
            event.position.x.into(),
            event.position.y.into(),
            window,
        );
        self.marquee_selection = Some(MarqueeSelection {
            start_x: x,
            start_y: y,
            current_x: x,
            current_y: y,
        });
        self.select_only_clip(None);
        cx.stop_propagation();
        cx.notify();
    }

    fn update_marquee_selection(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.marquee_selection.is_none() || !event.dragging() {
            return;
        }
        let (x, y) = Self::timeline_pointer_position(
            event.position.x.into(),
            event.position.y.into(),
            window,
        );
        if let Some(selection) = self.marquee_selection.as_mut() {
            selection.current_x = x;
            selection.current_y = y;
        }
        self.select_clips_in_marquee();
        cx.notify();
    }

    fn finish_marquee_selection(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.marquee_selection.is_none() {
            return;
        }
        let (x, y) = Self::timeline_pointer_position(
            event.position.x.into(),
            event.position.y.into(),
            window,
        );
        if let Some(selection) = self.marquee_selection.as_mut() {
            selection.current_x = x;
            selection.current_y = y;
        }
        self.select_clips_in_marquee();
        self.marquee_selection = None;
        cx.notify();
    }

    fn timeline_pointer_position(x: f32, y: f32, window: &Window) -> (f32, f32) {
        let viewport = window.viewport_size();
        let viewport_width = f32::from(viewport.width);
        let viewport_height = f32::from(viewport.height);
        let timeline_top = (viewport_height - TIMELINE_HEIGHT).max(0.0);
        (
            x.clamp(TRACK_HEADER_WIDTH, viewport_width),
            (y - timeline_top).clamp(TIMELINE_HEADER_HEIGHT + RULER_HEIGHT, TIMELINE_HEIGHT),
        )
    }

    fn select_clips_in_marquee(&mut self) {
        let Some(selection) = self.marquee_selection else {
            return;
        };
        let left = selection.start_x.min(selection.current_x);
        let right = selection.start_x.max(selection.current_x);
        let top = selection.start_y.min(selection.current_y);
        let bottom = selection.start_y.max(selection.current_y);
        let scroll_x = f32::from(self.timeline_scroll.offset().x);
        let scroll_y = f32::from(self.timeline_vertical_scroll.offset().y);

        let mut selected = HashSet::new();
        for (track_index, track) in self.project.tracks.iter().enumerate() {
            let clip_top = TIMELINE_HEADER_HEIGHT
                + RULER_HEIGHT
                + track_index as f32 * TRACK_HEIGHT
                + scroll_y
                + 5.0;
            let clip_bottom = clip_top + TRACK_HEIGHT - 10.0;
            for clip in self.project.clips_on_track(track.id) {
                let clip_left = TRACK_HEADER_WIDTH
                    + scroll_x
                    + TIMELINE_PADDING
                    + self.project.seconds(clip.timeline_start) as f32 * self.pixels_per_second;
                let clip_right = clip_left
                    + (self.project.seconds(clip.duration()) as f32 * self.pixels_per_second)
                        .max(4.0);
                if clip_left <= right
                    && clip_right >= left
                    && clip_top <= bottom
                    && clip_bottom >= top
                {
                    selected.insert(clip.id);
                }
            }
        }
        self.selected_clip_id = self
            .project
            .clips
            .iter()
            .find(|clip| selected.contains(&clip.id))
            .map(|clip| clip.id);
        self.selected_clip_ids = selected;
    }

    fn begin_clip_move(&mut self, clip_id: u64, event: &MouseDownEvent, cx: &mut Context<Self>) {
        let Some(clip) = self.project.clip(clip_id).cloned() else {
            return;
        };
        if self
            .project
            .track(clip.track_id)
            .is_some_and(|track| track.locked)
        {
            return;
        }
        if let Some(video) = &self.video {
            video.set_paused(true);
        }
        self.pause_audio_previews();
        self.playing = false;
        self.still_playback_started = None;
        self.select_only_clip(Some(clip_id));
        self.clip_move_drag = Some(ClipMoveDrag {
            clip_id,
            start_x: event.position.x.into(),
            original_timeline_start: clip.timeline_start,
            original_track_id: clip.track_id,
            changed: false,
        });
        cx.stop_propagation();
    }

    fn update_clip_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !event.dragging() {
            return;
        }
        let Some(drag) = self.clip_move_drag.as_ref() else {
            return;
        };
        let clip_id = drag.clip_id;
        let raw_delta = self.project.settings.frame_rate.delta(
            (f32::from(event.position.x) - drag.start_x) as f64 / self.pixels_per_second as f64,
        );
        let raw_start = (drag.original_timeline_start + raw_delta).max(TimelineTime::ZERO);
        let clip_duration = self
            .project
            .clip(clip_id)
            .map(TimelineClip::duration)
            .unwrap_or(TimelineTime::ZERO);
        let snapped_start = self.snap_clip_start(raw_start, clip_duration, clip_id);
        let viewport_height = f32::from(window.viewport_size().height);
        let track_top = viewport_height - TIMELINE_HEIGHT + TIMELINE_HEADER_HEIGHT + RULER_HEIGHT;
        let scroll_y: f32 = self.timeline_vertical_scroll.offset().y.into();
        let track_index =
            ((f32::from(event.position.y) - track_top - scroll_y) / TRACK_HEIGHT).floor() as isize;
        let target_track_id = usize::try_from(track_index)
            .ok()
            .and_then(|index| self.project.tracks.get(index))
            .map(|track| track.id)
            .unwrap_or(drag.original_track_id);
        let compatible = self.clip_track_compatible(clip_id, target_track_id);
        let actual_track_id = if compatible {
            target_track_id
        } else {
            drag.original_track_id
        };
        let available_start = self.project.nearest_available_start(
            actual_track_id,
            Some(clip_id),
            snapped_start,
            clip_duration,
        );
        let already_there = self.project.clip(clip_id).is_some_and(|clip| {
            clip.timeline_start == available_start && clip.track_id == actual_track_id
        });
        if already_there {
            return;
        }
        let moved_from_origin = available_start != drag.original_timeline_start
            || actual_track_id != drag.original_track_id;
        if !drag.changed && moved_from_origin {
            self.checkpoint();
            if let Some(drag) = &mut self.clip_move_drag {
                drag.changed = true;
            }
        }
        if let Some(clip) = self.project.clip_mut(clip_id) {
            clip.timeline_start = available_start;
            clip.track_id = actual_track_id;
        }
        cx.notify();
    }

    fn finish_clip_move(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.clip_move_drag.take().is_some_and(|drag| drag.changed) {
            self.save_project();
            self.load_timeline_position(self.playhead, false);
            cx.notify();
        }
    }

    fn clip_track_compatible(&self, clip_id: u64, track_id: u64) -> bool {
        let Some(clip) = self.project.clip(clip_id) else {
            return false;
        };
        let Some(track) = self.project.track(track_id) else {
            return false;
        };
        if track.locked {
            return false;
        }
        let Some(asset) = clip.asset_id.and_then(|id| self.project.asset(id)) else {
            return false;
        };
        match track.kind {
            TrackKind::Video => asset.kind != MediaKind::Audio,
            TrackKind::Audio => asset.has_audio,
        }
    }

    fn snap_time(&self, time: TimelineTime, ignored_clip: Option<u64>) -> TimelineTime {
        let threshold = self
            .project
            .settings
            .frame_rate
            .ceil(SNAP_DISTANCE_PX as f64 / self.pixels_per_second as f64)
            .frames()
            .max(1) as u64;
        let mut candidates = vec![TimelineTime::ZERO, self.playhead];
        candidates.extend(self.project.markers.iter().map(|marker| marker.time));
        for clip in &self.project.clips {
            if Some(clip.id) != ignored_clip {
                candidates.push(clip.timeline_start);
                candidates.push(clip.timeline_end());
            }
        }
        candidates
            .into_iter()
            .filter(|candidate| candidate.abs_diff(time) <= threshold)
            .min_by_key(|candidate| candidate.abs_diff(time))
            .unwrap_or(time)
            .max(TimelineTime::ZERO)
    }

    fn snap_clip_start(
        &self,
        start: TimelineTime,
        duration: TimelineTime,
        clip_id: u64,
    ) -> TimelineTime {
        let start_candidate = self.snap_time(start, Some(clip_id));
        let end_candidate = self.snap_time(start + duration, Some(clip_id)) - duration;
        if end_candidate.abs_diff(start) < start_candidate.abs_diff(start) {
            end_candidate.max(TimelineTime::ZERO)
        } else {
            start_candidate.max(TimelineTime::ZERO)
        }
    }

    fn clip_locked(&self, clip_id: u64) -> bool {
        self.project
            .clip(clip_id)
            .and_then(|clip| self.project.track(clip.track_id))
            .is_some_and(|track| track.locked)
    }

    fn begin_trim(&mut self, clip_id: u64, edge: TrimEdge, x: f32) {
        let Some(clip) = self.project.clip(clip_id).cloned() else {
            return;
        };
        if self.clip_locked(clip_id) {
            return;
        }
        let asset_duration = clip
            .asset_id
            .and_then(|id| self.project.asset(id))
            .map(|asset| self.project.ceil_time(asset.duration))
            .unwrap_or(TimelineTime::MAX);
        if let Some(video) = &self.video {
            video.set_paused(true);
        }
        self.pause_audio_previews();
        self.still_playback_started = None;
        self.playing = false;
        self.select_only_clip(Some(clip_id));
        self.trim_drag = Some(TrimDrag {
            clip_id,
            edge,
            start_x: x,
            original_in: clip.source_in,
            original_out: clip.source_out,
            original_timeline_start: clip.timeline_start,
            asset_duration,
            changed: false,
        });
    }

    fn update_trim(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if !event.dragging() {
            return;
        }
        let Some(drag) = self.trim_drag.as_ref() else {
            return;
        };
        let clip_id = drag.clip_id;
        let edge = drag.edge;
        let original_in = drag.original_in;
        let original_out = drag.original_out;
        let original_timeline_start = drag.original_timeline_start;
        let asset_duration = drag.asset_duration;
        let Some((previous_end, next_start)) = self.project.trim_limits(clip_id) else {
            return;
        };
        let raw_delta = self.project.settings.frame_rate.delta(
            (f32::from(event.position.x) - drag.start_x) as f64 / self.pixels_per_second as f64,
        );
        if raw_delta == TimelineTime::ZERO {
            return;
        }
        if !drag.changed {
            self.checkpoint();
            if let Some(drag) = &mut self.trim_drag {
                drag.changed = true;
            }
        }
        let Some(index) = self.project.clip_index(clip_id) else {
            return;
        };
        match edge {
            TrimEdge::Left => {
                let raw_start = (original_timeline_start + raw_delta).max(TimelineTime::ZERO);
                let original_end = original_timeline_start + original_out - original_in;
                let earliest_start = previous_end.max(original_timeline_start - original_in);
                let latest_start = original_end - TimelineTime::ONE_FRAME;
                let start = self
                    .snap_time(raw_start, Some(clip_id))
                    .clamp(earliest_start, latest_start);
                self.project.clips[index].source_in = original_in + start - original_timeline_start;
                self.project.clips[index].timeline_start = start;
            }
            TrimEdge::Right => {
                let original_end = original_timeline_start + original_out - original_in;
                let earliest_end = original_timeline_start + TimelineTime::ONE_FRAME;
                let latest_end = next_start.min(
                    original_timeline_start
                        + (asset_duration - original_in).max(TimelineTime::ONE_FRAME),
                );
                let end = self
                    .snap_time(original_end + raw_delta, Some(clip_id))
                    .clamp(earliest_end, latest_end);
                self.project.clips[index].source_out = original_in + end - original_timeline_start;
            }
        }
        cx.notify();
    }

    fn finish_trim(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.trim_drag.take().is_some_and(|drag| drag.changed) {
            self.save_project();
            if let Some(clip_id) = self.selected_clip_id
                && let Some(index) = self.project.clip_index(clip_id)
            {
                self.load_timeline_position(self.project.clips[index].timeline_start, false);
            }
            cx.notify();
        }
    }

    fn export(&mut self, cx: &mut Context<Self>) {
        if self.project.clips.is_empty() || self.exporting {
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
        self.preview_target = PreviewTarget::Timeline;
        self.video = None;
        self.standalone_audio = None;
        self.audio_previews.clear();
        self.loaded_clip_id = None;
        self.still_playback_started = None;
        self.playing = false;
        self.playhead = TimelineTime::ZERO;
        self.select_only_clip(self.project.clips.first().map(|clip| clip.id));
        if !self.project.clips.is_empty() {
            self.load_timeline_position(TimelineTime::ZERO, false);
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
        self.pixels_per_second = (self.pixels_per_second * factor).clamp(
            MIN_TIMELINE_PIXELS_PER_SECOND,
            MAX_TIMELINE_PIXELS_PER_SECOND,
        );
    }

    fn log_timeline_trackpad_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        if !event.delta.precise() {
            return;
        }

        let delta = event.delta.pixel_delta(px(16.0));
        let horizontal = f32::from(delta.x);
        let vertical = f32::from(delta.y);
        let action = if horizontal.abs() < f32::EPSILON && vertical.abs() < f32::EPSILON {
            "idle"
        } else {
            "pan"
        };
        log::debug!(
            target: "opencut::timeline",
            "trackpad-scroll phase={:?} delta=({horizontal:.2}, {vertical:.2}) action={action}",
            event.touch_phase,
        );
    }

    fn apply_timeline_pinch(&mut self) -> bool {
        let Some(gesture) = crate::macos_pinch::take() else {
            return false;
        };
        if !(0.0..=TIMELINE_HEIGHT as f64).contains(&gesture.location_y) {
            log::debug!(
                target: "opencut::timeline",
                "trackpad-pinch magnification={:.4} location_y={:.1} action=ignored",
                gesture.magnification,
                gesture.location_y,
            );
            return false;
        }

        let previous_zoom = self.pixels_per_second;
        let factor = (gesture.magnification as f32).exp().clamp(0.5, 2.0);
        self.zoom(factor);
        log::debug!(
            target: "opencut::timeline",
            "trackpad-pinch magnification={:.4} location_y={:.1} action=zoom factor={factor:.4} px_per_second={previous_zoom:.2}->{:.2}",
            gesture.magnification,
            gesture.location_y,
            self.pixels_per_second,
        );
        self.pixels_per_second != previous_zoom
    }

    fn timeline_position_from_x(&self, x: f32) -> TimelineTime {
        let scroll_x: f32 = self.timeline_scroll.offset().x.into();
        let content_x = x - TRACK_HEADER_WIDTH - scroll_x - TIMELINE_PADDING;
        self.project
            .nearest_time(content_x as f64 / self.pixels_per_second as f64)
            .clamp(TimelineTime::ZERO, self.project.timeline_duration())
    }

    fn begin_playhead_scrub(&mut self, event: &MouseDownEvent) {
        self.is_scrubbing_playhead = true;
        if let Some(video) = &self.video {
            video.set_paused(true);
        }
        self.pause_audio_previews();
        self.playing = false;
        self.still_playback_started = None;
        let position = self.timeline_position_from_x(event.position.x.into());
        self.last_playhead_scrub_seek = Some(Instant::now());
        self.load_timeline_position_for_scrub(position, false, false);
    }

    fn update_playhead_scrub(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_scrubbing_playhead && event.dragging() {
            let position = self.timeline_position_from_x(event.position.x.into());
            self.playhead = position;

            let now = Instant::now();
            let should_seek = self
                .last_playhead_scrub_seek
                .is_none_or(|last_seek| now.duration_since(last_seek) >= SCRUB_SEEK_INTERVAL);
            if should_seek {
                self.last_playhead_scrub_seek = Some(now);
                self.load_timeline_position_for_scrub(position, false, false);
            }
            cx.notify();
        }
    }

    fn finish_playhead_scrub(
        &mut self,
        event: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_scrubbing_playhead {
            self.is_scrubbing_playhead = false;
            self.last_playhead_scrub_seek = None;
            let position = self.timeline_position_from_x(event.position.x.into());
            self.load_timeline_position_for_scrub(position, true, true);
            cx.notify();
        }
    }

    fn step_playhead(&mut self, frames: i64) {
        if self.project.clips.is_empty() {
            return;
        }
        let target = (self.playhead + TimelineTime::from_frames(frames))
            .clamp(TimelineTime::ZERO, self.project.timeline_duration());
        if target != self.playhead || self.preview_target != PreviewTarget::Timeline {
            self.load_timeline_position(target, false);
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

    fn action_step_backward_frame(
        &mut self,
        _: &StepBackwardFrame,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.step_playhead(-1);
        cx.notify();
    }

    fn action_step_forward_frame(
        &mut self,
        _: &StepForwardFrame,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.step_playhead(1);
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

    fn action_duplicate_selected(
        &mut self,
        _: &DuplicateSelected,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.duplicate_selected();
        cx.notify();
    }

    fn action_add_marker(&mut self, _: &AddMarker, _: &mut Window, cx: &mut Context<Self>) {
        self.add_marker();
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.toggle_inspector(cx);
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
