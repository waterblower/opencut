use crate::video::Video;
use gpui::{
    App, Context, CursorStyle, DragMoveEvent, Entity, FocusHandle, KeyBinding, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit, PathPromptOptions, Render,
    ScrollHandle, ScrollWheelEvent, Window, actions, div, img, point, prelude::*, px, rgb,
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

mod clip_placement;
mod clip_render_plan;
mod editing;
mod explorer;
mod explorer_filter;
mod export;
mod export_dialog;
mod export_gstreamer;
mod media_cache;
mod media_probe;
mod model;
mod preview;
mod preview_audio;
mod preview_image;
mod preview_timeline;
mod preview_video;
mod properties;
mod properties_transform;
mod settings;
mod timeline;
mod timeline_document;
mod timeline_interactions;
mod timeline_video;
mod track;
mod view;
mod workspace;

use crate::playback_view::{DragPhase, PlaybackViewDelegate};
use clip_placement::{ClipPlacementRejection, validate_clip_placement};
use editing::ClipClipboard;
use explorer::{
    ExplorerDropPreview, ExplorerMediaDrag, FileContextMenu, NewTimelineDialogState,
    PendingExplorerDrop, RenameDialogState,
};
use explorer_filter::ExplorerFilter;
use export_dialog::ExportDialogState;
use media_probe::probe_asset;
use model::{
    AudioClipProperties, DEFAULT_IMAGE_CLIP_DURATION, FRAME_RATE_PRESETS, FrameRate, MediaAsset,
    MediaKind, Project, TimelineClip, TimelineTime, TimelineTrack, TrackKind, VideoClipProperties,
    timeline_ranges_overlap,
};
use preview::PreviewTarget;
use preview_audio::AudioPreview;
use properties::PropertiesPanelResizeDrag;
use properties_transform::{OpacityDrag, VideoTransformInputs};
use timeline_document::load_existing;
use timeline_interactions::{ClipMoveDrag, MarqueeSelection, TimelineTool, TrimDrag, TrimEdge};
use workspace::{
    FileTreeEntry, TimelineViewState, load_active_timeline, load_project_root, load_timeline_view,
    load_timeline_zoom, save_active_timeline, save_project_root, save_timeline_view,
    save_timeline_zoom, timeline_view_state, visible_tree,
};

const MEDIA_PANEL_WIDTH: f32 = 340.0;
const DEFAULT_PROPERTIES_PANEL_WIDTH: f32 = 420.0;
const MIN_PROPERTIES_PANEL_WIDTH: f32 = 240.0;
const MAX_PROPERTIES_PANEL_WIDTH: f32 = 600.0;
const MIN_PREVIEW_WIDTH: f32 = 320.0;
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
const DEFAULT_TIMELINE_PIXELS_PER_SECOND: f32 = 72.0;
const TIMELINE_ZOOM_SAVE_DELAY: Duration = Duration::from_millis(500);
const TIMELINE_VIEW_SAVE_DELAY: Duration = Duration::from_secs(1);
const SCRUB_SEEK_INTERVAL: Duration = Duration::from_millis(50);
const IDLE_UPDATE_INTERVAL: Duration = Duration::from_millis(33);

#[cfg(test)]
fn lock_gstreamer_test() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    LOCK.lock().unwrap_or_else(|error| error.into_inner())
}

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
        CopySelectedClips,
        CutSelectedClips,
        PasteClips,
        SelectAllUnlockedClips,
        ActivateSelectionTool,
        ActivateBladeTool,
        ActivateTrimTool,
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
        KeyBinding::new("cmd-c", CopySelectedClips, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("cmd-x", CutSelectedClips, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("cmd-v", PasteClips, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new(
            "cmd-a",
            SelectAllUnlockedClips,
            Some(EDITOR_SHORTCUT_CONTEXT),
        ),
        KeyBinding::new("v", ActivateSelectionTool, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("b", ActivateBladeTool, Some(EDITOR_SHORTCUT_CONTEXT)),
        KeyBinding::new("t", ActivateTrimTool, Some(EDITOR_SHORTCUT_CONTEXT)),
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
        let _ = window.update(cx, |_, window, cx| {
            crate::gpui_inspector::toggle(window, cx)
        });
    });
}

struct ExplorerState {
    file_tree: Vec<FileTreeEntry>,
    expanded_directories: HashSet<PathBuf>,
    root_expanded: bool,
    filter: Entity<ExplorerFilter>,
    search_query: Option<String>,
    search_results: Vec<FileTreeEntry>,
    search_pending: bool,
    scroll: ScrollHandle,
    selected_file: Option<PathBuf>,
    context_menu: Option<FileContextMenu>,
    rename_dialog: Option<RenameDialogState>,
    new_timeline_dialog: Option<NewTimelineDialogState>,
    drag_assets: HashMap<PathBuf, MediaAsset>,
    drag_probe_jobs: HashSet<PathBuf>,
    drop_preview: Option<ExplorerDropPreview>,
    pending_drop: Option<PendingExplorerDrop>,
    last_tree_scan: Instant,
}

struct PreviewState {
    target: PreviewTarget,
    video: Option<Video>,
    audio: Option<AudioPreview>,
    timeline_needs_rebuild: bool,
    timeline_clock: Option<(TimelineTime, Instant)>,
    playing: bool,
    volume: f64,
    volume_open: bool,
    is_scrubbing: bool,
    is_adjusting_volume: bool,
    resume_after_scrub: bool,
    scrub_fraction: Option<f32>,
    pending_seek_started: Option<Instant>,
    last_scrub_seek: Option<Instant>,
    refresh_ticks: u8,
}

struct TimelineState {
    playhead: TimelineTime,
    pixels_per_second: f32,
    zoom_save_due: Option<Instant>,
    view_state: TimelineViewState,
    view_save_due: Option<Instant>,
    active_tool: TimelineTool,
    blade_guide: Option<TimelineTime>,
    snapping_enabled: bool,
    magnet_enabled: bool,
    snap_guide: Option<TimelineTime>,
    trim_drag: Option<TrimDrag>,
    clip_move_drag: Option<ClipMoveDrag>,
    marquee_selection: Option<MarqueeSelection>,
    scrubbing_playhead: bool,
    last_scrub_seek: Option<Instant>,
    scroll: ScrollHandle,
    vertical_scroll: ScrollHandle,
    selected_clip_id: Option<u64>,
    selected_clip_ids: HashSet<u64>,
    clipboard: Option<ClipClipboard>,
    next_id: u64,
    undo_stack: Vec<Project>,
    redo_stack: Vec<Project>,
}

struct PropertiesPanelState {
    width: f32,
    resizing: bool,
    transform_inputs: VideoTransformInputs,
    transform_input_clip_id: Option<u64>,
    opacity_drag: Option<OpacityDrag>,
}

struct ExportState {
    dialog: Option<ExportDialogState>,
    running: bool,
}

pub(crate) struct Editor {
    project_root: PathBuf,
    timeline_path: Option<PathBuf>,
    project: Project,
    explorer: ExplorerState,
    preview: PreviewState,
    media_cache_jobs: HashSet<u64>,
    media_cache_ready: HashSet<u64>,
    waveform_cache: HashMap<u64, Arc<media_cache::WaveformData>>,
    selected_asset_id: Option<u64>,
    properties: PropertiesPanelState,
    settings_open: bool,
    export: ExportState,
    timeline: TimelineState,
    status: Option<String>,
    error: Option<String>,
    focus_handle: FocusHandle,
}

impl Editor {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let project_root = load_project_root();
        let pixels_per_second = load_timeline_zoom().clamp(
            MIN_TIMELINE_PIXELS_PER_SECOND,
            MAX_TIMELINE_PIXELS_PER_SECOND,
        );
        let expanded_directories = HashSet::new();
        let file_tree = visible_tree(&project_root, &expanded_directories).unwrap_or_default();
        let preferred_timeline = load_active_timeline(&project_root);
        let (timeline_path, project, mut startup_error) =
            match load_existing(&project_root, preferred_timeline.as_deref()) {
                Ok(Some((path, project))) => (Some(path), project, None),
                Ok(None) => (None, Project::default(), None),
                Err(error) => (
                    None,
                    Project::default(),
                    Some(format!("Could not open timeline: {error}")),
                ),
            };
        if let Some(timeline_path) = timeline_path.as_ref()
            && let Err(error) = save_active_timeline(&project_root, timeline_path)
        {
            startup_error = Some(error);
        }
        let timeline_view_state = timeline_path
            .as_ref()
            .map(|path| load_timeline_view(&project_root.join(path)))
            .unwrap_or_default();
        let playhead = TimelineTime::from_frames(timeline_view_state.playhead_frame)
            .clamp(TimelineTime::ZERO, project.timeline_duration());
        let timeline_scroll = ScrollHandle::new();
        timeline_scroll.set_offset(point(px(-timeline_view_state.horizontal_scroll), px(0.0)));
        let timeline_vertical_scroll = ScrollHandle::new();
        timeline_vertical_scroll
            .set_offset(point(px(0.0), px(-timeline_view_state.vertical_scroll)));
        let next_id = project.next_id();
        let selected_asset_id = project.assets.first().map(|asset| asset.id);
        let selected_clip_id = project.clips.first().map(|clip| clip.id);
        let selected_clip_ids = selected_clip_id.into_iter().collect();
        let focus_handle = cx.focus_handle();
        let explorer_filter = cx.new(|cx| ExplorerFilter::new(focus_handle.clone(), cx));
        let video_transform_inputs = VideoTransformInputs::new(focus_handle.clone(), cx);
        Self::observe_video_transform_inputs(&video_transform_inputs, cx);
        cx.observe(&explorer_filter, |editor, _, cx| {
            editor.schedule_explorer_search(cx);
            cx.notify();
        })
        .detach();
        focus_handle.focus(window);
        Self::start_updates(cx);

        let mut editor = Self {
            project_root,
            timeline_path: timeline_path.clone(),
            project,
            explorer: ExplorerState {
                file_tree,
                expanded_directories,
                root_expanded: true,
                filter: explorer_filter,
                search_query: None,
                search_results: Vec::new(),
                search_pending: false,
                scroll: ScrollHandle::new(),
                selected_file: timeline_path,
                context_menu: None,
                rename_dialog: None,
                new_timeline_dialog: None,
                drag_assets: HashMap::new(),
                drag_probe_jobs: HashSet::new(),
                drop_preview: None,
                pending_drop: None,
                last_tree_scan: Instant::now(),
            },
            preview: PreviewState {
                target: PreviewTarget::Timeline,
                video: None,
                audio: None,
                timeline_needs_rebuild: true,
                timeline_clock: None,
                playing: false,
                volume: 1.0,
                volume_open: false,
                is_scrubbing: false,
                is_adjusting_volume: false,
                resume_after_scrub: false,
                scrub_fraction: None,
                pending_seek_started: None,
                last_scrub_seek: None,
                refresh_ticks: 0,
            },
            media_cache_jobs: HashSet::new(),
            media_cache_ready: HashSet::new(),
            waveform_cache: HashMap::new(),
            selected_asset_id,
            properties: PropertiesPanelState {
                width: DEFAULT_PROPERTIES_PANEL_WIDTH,
                resizing: false,
                transform_inputs: video_transform_inputs,
                transform_input_clip_id: None,
                opacity_drag: None,
            },
            settings_open: false,
            export: ExportState {
                dialog: None,
                running: false,
            },
            timeline: TimelineState {
                playhead,
                pixels_per_second,
                zoom_save_due: None,
                view_state: timeline_view_state.clone(),
                view_save_due: None,
                active_tool: TimelineTool::Selection,
                blade_guide: None,
                snapping_enabled: timeline_view_state.snapping_enabled,
                magnet_enabled: timeline_view_state.track_magnet_enabled,
                snap_guide: None,
                trim_drag: None,
                clip_move_drag: None,
                marquee_selection: None,
                scrubbing_playhead: false,
                last_scrub_seek: None,
                scroll: timeline_scroll,
                vertical_scroll: timeline_vertical_scroll,
                selected_clip_id,
                selected_clip_ids,
                clipboard: None,
                next_id,
                undo_stack: Vec::new(),
                redo_stack: Vec::new(),
            },
            status: None,
            error: startup_error,
            focus_handle,
        };
        if !editor.project.clips.is_empty() {
            editor.load_timeline_position(editor.timeline.playhead, false);
        }
        editor
    }

    fn start_updates(cx: &mut Context<Self>) {
        cx.spawn(async move |editor, cx| {
            let mut update_interval = IDLE_UPDATE_INTERVAL;
            loop {
                cx.background_executor().timer(update_interval).await;
                match editor.update(cx, |editor, cx| {
                    let refresh_tree =
                        editor.explorer.last_tree_scan.elapsed() >= Duration::from_secs(1);
                    let file_preview_playing = match editor.preview.target {
                        PreviewTarget::VideoFile(_) => editor
                            .preview
                            .video
                            .as_ref()
                            .is_some_and(|video| !video.paused()),
                        PreviewTarget::AudioFile(_) => editor
                            .preview
                            .audio
                            .as_ref()
                            .is_some_and(AudioPreview::playing),
                        _ => false,
                    };
                    let pinch_zoomed = editor.apply_timeline_pinch();
                    if editor
                        .timeline
                        .zoom_save_due
                        .is_some_and(|due| Instant::now() >= due)
                    {
                        if let Err(error) = save_timeline_zoom(editor.timeline.pixels_per_second) {
                            editor.error = Some(error);
                        }
                        editor.timeline.zoom_save_due = None;
                    }
                    let ended_explorer_drag =
                        !cx.has_active_drag() && editor.explorer.drop_preview.take().is_some();
                    if ended_explorer_drag {
                        editor.timeline.snap_guide = None;
                    }
                    let should_render = editor.preview.playing
                        || file_preview_playing
                        || editor.export.running
                        || editor.preview.refresh_ticks > 0
                        || refresh_tree
                        || pinch_zoomed
                        || ended_explorer_drag;
                    editor.preview.refresh_ticks = editor.preview.refresh_ticks.saturating_sub(1);
                    if refresh_tree {
                        editor.refresh_file_tree();
                        editor.schedule_missing_media_cache(cx);
                    }
                    editor.update_playback();
                    editor.reconcile_preview_seek();
                    let current_timeline_view = editor.current_timeline_view_state();
                    if current_timeline_view
                        .as_ref()
                        .is_some_and(|view| view != &editor.timeline.view_state)
                    {
                        let current_timeline_view = current_timeline_view.unwrap();
                        editor.timeline.view_state = current_timeline_view;
                        if editor.timeline.view_save_due.is_none() {
                            editor.timeline.view_save_due =
                                Some(Instant::now() + TIMELINE_VIEW_SAVE_DELAY);
                        }
                    }
                    if editor
                        .timeline
                        .view_save_due
                        .is_some_and(|due| Instant::now() >= due)
                    {
                        if let Err(error) = save_timeline_view(&editor.timeline.view_state) {
                            editor.error = Some(error);
                        }
                        editor.timeline.view_save_due = None;
                    }
                    if should_render {
                        cx.notify();
                    }
                    editor.update_interval()
                }) {
                    Ok(next_interval) => update_interval = next_interval,
                    Err(_) => break,
                }
            }
        })
        .detach();
    }

    fn update_interval(&self) -> Duration {
        if self.preview.playing {
            self.project.duration(TimelineTime::ONE_FRAME)
        } else {
            IDLE_UPDATE_INTERVAL
        }
    }

    fn current_timeline_view_state(&self) -> Option<TimelineViewState> {
        let horizontal_scroll = (-f32::from(self.timeline.scroll.offset().x)).max(0.0);
        let vertical_scroll = (-f32::from(self.timeline.vertical_scroll.offset().y)).max(0.0);
        Some(timeline_view_state(
            &self.timeline_file_path()?,
            self.timeline.playhead.frames(),
            horizontal_scroll,
            vertical_scroll,
            self.timeline.snapping_enabled,
            self.timeline.magnet_enabled,
        ))
    }

    pub(super) fn timeline_file_path(&self) -> Option<PathBuf> {
        self.timeline_path
            .as_ref()
            .map(|path| self.project_root.join(path))
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
        let preferred_timeline = load_active_timeline(&root);
        let (timeline_path, project) = match load_existing(&root, preferred_timeline.as_deref()) {
            Ok(Some((path, project))) => (Some(path), project),
            Ok(None) => (None, Project::default()),
            Err(error) => {
                self.error = Some(format!("Could not open timeline: {error}"));
                return;
            }
        };
        if let Some(view_state) = self.current_timeline_view_state()
            && let Err(error) = save_timeline_view(&view_state)
        {
            self.error = Some(error);
        }
        self.save_project();
        self.project_root = root;
        self.explorer.expanded_directories.clear();
        self.explorer.root_expanded = true;
        self.activate_timeline(timeline_path, project, cx);
        if let Err(error) = save_project_root(&self.project_root) {
            self.error = Some(error);
        }
    }

    pub(super) fn open_timeline(&mut self, relative_path: PathBuf, cx: &mut Context<Self>) {
        if self.timeline_path.as_ref() == Some(&relative_path) {
            self.explorer.selected_file = Some(relative_path);
            self.preview.target = PreviewTarget::Timeline;
            cx.notify();
            return;
        }
        let path = self.project_root.join(&relative_path);
        let project = match Project::load(&path) {
            Ok(project) => project,
            Err(error) => {
                self.error = Some(format!("Could not open timeline: {error}"));
                return;
            }
        };
        self.save_project();
        if let Some(view_state) = self.current_timeline_view_state()
            && let Err(error) = save_timeline_view(&view_state)
        {
            self.error = Some(error);
        }
        self.activate_timeline(Some(relative_path.clone()), project, cx);
        self.status = Some(format!("Opened {}", relative_path.display()));
    }

    /// Saves the current timeline, switches to a freshly created one, and reveals it in
    /// the file tree. `relative_directory` is only used to expand the containing folder.
    fn activate_created_timeline(
        &mut self,
        relative_directory: PathBuf,
        relative_path: PathBuf,
        project: Project,
        cx: &mut Context<Self>,
    ) {
        self.save_project();
        if let Some(view_state) = self.current_timeline_view_state()
            && let Err(error) = save_timeline_view(&view_state)
        {
            self.error = Some(error);
        }
        // Expand the target folder so the new timeline is visible in the tree.
        if !relative_directory.as_os_str().is_empty() {
            self.explorer
                .expanded_directories
                .insert(relative_directory);
        }
        self.activate_timeline(Some(relative_path.clone()), project, cx);
        self.refresh_file_tree();
        self.status = Some(format!("Created {}", relative_path.display()));
    }

    fn activate_timeline(
        &mut self,
        timeline_path: Option<PathBuf>,
        project: Project,
        cx: &mut Context<Self>,
    ) {
        self.preview.video = None;
        self.preview.audio = None;
        self.media_cache_jobs.clear();
        self.media_cache_ready.clear();
        self.waveform_cache.clear();
        self.explorer.drag_assets.clear();
        self.explorer.drag_probe_jobs.clear();
        self.explorer.drop_preview = None;
        self.explorer.pending_drop = None;
        self.timeline.clipboard = None;
        self.preview.timeline_needs_rebuild = true;
        self.preview.timeline_clock = None;
        self.preview.playing = false;
        self.preview.volume_open = false;
        self.preview.is_scrubbing = false;
        self.preview.is_adjusting_volume = false;
        self.preview.resume_after_scrub = false;
        self.preview.scrub_fraction = None;
        self.preview.pending_seek_started = None;
        self.preview.last_scrub_seek = None;
        self.properties.transform_input_clip_id = None;
        self.properties.opacity_drag = None;
        self.timeline.trim_drag = None;
        self.timeline.clip_move_drag = None;
        self.timeline.marquee_selection = None;
        self.timeline.scrubbing_playhead = false;
        self.timeline.last_scrub_seek = None;
        self.timeline.blade_guide = None;
        self.timeline.snap_guide = None;
        self.timeline_path = timeline_path;
        self.project = project;
        self.timeline.view_state = self
            .timeline_file_path()
            .as_deref()
            .map(load_timeline_view)
            .unwrap_or_default();
        self.timeline.view_save_due = None;
        self.timeline.playhead = TimelineTime::from_frames(self.timeline.view_state.playhead_frame)
            .clamp(TimelineTime::ZERO, self.project.timeline_duration());
        self.preview.refresh_ticks = 2;
        self.timeline.snapping_enabled = self.timeline.view_state.snapping_enabled;
        self.timeline.magnet_enabled = self.timeline.view_state.track_magnet_enabled;
        self.timeline.scroll.set_offset(point(
            px(-self.timeline.view_state.horizontal_scroll),
            px(0.0),
        ));
        self.timeline.vertical_scroll.set_offset(point(
            px(0.0),
            px(-self.timeline.view_state.vertical_scroll),
        ));
        self.timeline.next_id = self.project.next_id();
        self.timeline.undo_stack.clear();
        self.timeline.redo_stack.clear();
        self.explorer.search_query = None;
        self.explorer.search_results.clear();
        self.explorer.search_pending = false;
        self.explorer
            .filter
            .update(cx, |filter, cx| filter.clear(cx));
        self.explorer.selected_file = self.timeline_path.clone();
        self.explorer.context_menu = None;
        self.preview.target = PreviewTarget::Timeline;
        self.selected_asset_id = self.project.assets.first().map(|asset| asset.id);
        self.select_only_clip(self.project.clips.first().map(|clip| clip.id));
        self.refresh_file_tree();
        self.error = None;
        if let Some(timeline_path) = self.timeline_path.as_ref()
            && let Err(error) = save_active_timeline(&self.project_root, timeline_path)
        {
            self.error = Some(error);
        }
        if !self.project.clips.is_empty() {
            self.load_timeline_position(self.timeline.playhead, false);
        }
    }

    /// Refreshes derived media caches and starts one missing generation/load job.
    ///
    /// Runs on the file-tree tick rather than during rendering so the timeline can read
    /// `media_cache_ready` without touching the filesystem on every frame.
    fn schedule_missing_media_cache(&mut self, cx: &mut Context<Self>) {
        let referenced_asset_ids = self
            .project
            .clips
            .iter()
            .map(|clip| clip.asset_id)
            .collect::<HashSet<_>>();
        self.media_cache_ready = self
            .project
            .assets
            .iter()
            .filter(|asset| {
                referenced_asset_ids.contains(&asset.id)
                    && media_cache::cache_is_ready(&self.project_root, asset)
            })
            .map(|asset| asset.id)
            .collect();
        self.waveform_cache.retain(|asset_id, _| {
            referenced_asset_ids.contains(asset_id) && self.media_cache_ready.contains(asset_id)
        });
        let Some(asset) = self
            .project
            .assets
            .iter()
            .find(|asset| {
                referenced_asset_ids.contains(&asset.id)
                    && !self.media_cache_jobs.contains(&asset.id)
                    && (!self.media_cache_ready.contains(&asset.id)
                        || asset.has_audio && !self.waveform_cache.contains_key(&asset.id))
            })
            .cloned()
        else {
            return;
        };
        let asset_id = asset.id;
        self.media_cache_jobs.insert(asset_id);
        let project_root = self.project_root.clone();
        cx.spawn(async move |editor, cx| {
            let cache_root = project_root.clone();
            let result = cx
                .background_executor()
                .spawn(async move { media_cache::prepare(&cache_root, &asset) })
                .await;
            editor
                .update(cx, |editor, cx| {
                    if editor.project_root == project_root {
                        editor.media_cache_jobs.remove(&asset_id);
                        match result {
                            Ok(Some(waveform)) => {
                                editor.waveform_cache.insert(asset_id, Arc::new(waveform));
                                editor.media_cache_ready.insert(asset_id);
                            }
                            Ok(None) => {
                                editor.media_cache_ready.insert(asset_id);
                            }
                            Err(error) => eprintln!("Media cache: {error}"),
                        }
                        cx.notify();
                    }
                })
                .ok();
        })
        .detach();
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
        self.blade_at_playhead();
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

    fn action_copy_selected_clips(
        &mut self,
        _: &CopySelectedClips,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.copy_selected_clips();
        cx.notify();
    }

    fn action_cut_selected_clips(
        &mut self,
        _: &CutSelectedClips,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cut_selected_clips();
        cx.notify();
    }

    fn action_paste_clips(&mut self, _: &PasteClips, _: &mut Window, cx: &mut Context<Self>) {
        self.paste_clips();
        cx.notify();
    }

    fn action_select_all_unlocked_clips(
        &mut self,
        _: &SelectAllUnlockedClips,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.select_all_unlocked_clips();
        cx.notify();
    }

    fn action_activate_selection_tool(
        &mut self,
        _: &ActivateSelectionTool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_timeline_tool(TimelineTool::Selection);
        cx.notify();
    }

    fn action_activate_blade_tool(
        &mut self,
        _: &ActivateBladeTool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_timeline_tool(TimelineTool::Blade);
        cx.notify();
    }

    fn action_activate_trim_tool(
        &mut self,
        _: &ActivateTrimTool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.activate_timeline_tool(TimelineTool::Trim);
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
        crate::gpui_inspector::toggle(window, cx);
    }
}

impl Drop for Editor {
    fn drop(&mut self) {
        if let Some(view_state) = self.current_timeline_view_state()
            && let Err(error) = save_timeline_view(&view_state)
        {
            eprintln!("Could not save timeline view state: {error}");
        }
    }
}

fn format_time(seconds: f64, padded_minutes: bool) -> String {
    let total = seconds.max(0.0).round() as u64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    let minutes_text = if hours > 0 || padded_minutes {
        format!("{minutes:02}")
    } else {
        format!("{minutes}")
    };
    if hours > 0 {
        format!("{hours}:{minutes_text}:{seconds:02}")
    } else {
        format!("{minutes_text}:{seconds:02}")
    }
}

#[cfg(test)]
#[path = "editor.test.rs"]
mod tests;
