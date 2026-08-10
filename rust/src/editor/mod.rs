use crate::video::Video;
use gpui::{
    App, Context, CursorStyle, DragMoveEvent, Entity, FocusHandle, KeyBinding, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit, PathPromptOptions, Render,
    ScrollHandle, ScrollWheelEvent, TouchPhase, Window, actions, div, img, prelude::*, px, rgb,
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
mod timeline_clip_menu;
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
    ExplorerDropPreview, ExplorerMediaDrag, FileContextMenu, FileTreeEntry, NewTimelineDialogState,
    PendingExplorerDrop, RenameDialogState, visible_tree,
};
use explorer_filter::ExplorerFilter;
use export_dialog::ExportDialogState;
use media_probe::probe_asset;
use model::{
    AudioClipProperties, DEFAULT_IMAGE_CLIP_DURATION, FRAME_RATE_PRESETS, FrameRate, MediaAsset,
    MediaKind, TimelineClip, TimelineTime, TimelineTrack, TrackKind, VideoClipProperties,
    timeline_ranges_overlap,
};
use preview::PreviewTarget;
use preview_audio::AudioPreview;
use preview_timeline::TimelinePreviewDrag;
use properties::PropertiesPanelResizeDrag;
use properties_transform::{OpacityDrag, VideoTransformInputs};
use timeline::{Timeline, TimelineState};
use timeline_clip_menu::TimelineClipContextMenu;
use timeline_document::load_existing;
use timeline_interactions::{ClipMoveDrag, MarqueeSelection, TimelineTool, TrimDrag, TrimEdge};
use workspace::{load_active_timeline, load_project_root, save_active_timeline, save_project_root};

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
    fullscreen: bool,
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
    timeline_drag: Option<TimelinePreviewDrag>,
    refresh_ticks: u8,
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
    explorer: ExplorerState,
    preview: PreviewState,
    media_cache_jobs: HashSet<u64>,
    media_cache_ready: HashSet<u64>,
    waveform_cache: HashMap<u64, Arc<media_cache::WaveformData>>,
    properties: PropertiesPanelState,
    settings_open: bool,
    export: ExportState,
    timeline: Option<TimelineState>,
    status: Option<String>,
    focus_handle: FocusHandle,
}

impl Editor {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let project_root = load_project_root();
        let expanded_directories = HashSet::new();
        let file_tree = visible_tree(&project_root, &expanded_directories).unwrap_or_default();
        let preferred_timeline = load_active_timeline(&project_root);
        let active_timeline = match load_existing(&project_root, preferred_timeline.as_deref()) {
            Ok(active_timeline) => active_timeline,
            Err(error) => {
                eprintln!("Could not open timeline: {error}");
                None
            }
        };
        if let Some((path, _)) = active_timeline.as_ref()
            && let Err(error) = save_active_timeline(&project_root, path)
        {
            eprintln!("{error}");
        }
        let timeline = active_timeline.map(|(path, data)| TimelineState::new(path, data));
        let selected_file = timeline.as_ref().map(|timeline| timeline.path.clone());
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
            explorer: ExplorerState {
                file_tree,
                expanded_directories,
                root_expanded: true,
                filter: explorer_filter,
                search_query: None,
                search_results: Vec::new(),
                search_pending: false,
                scroll: ScrollHandle::new(),
                selected_file,
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
                fullscreen: false,
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
                timeline_drag: None,
                refresh_ticks: 0,
            },
            media_cache_jobs: HashSet::new(),
            media_cache_ready: HashSet::new(),
            waveform_cache: HashMap::new(),
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
            timeline,
            status: None,
            focus_handle,
        };
        if let Some(timeline) = editor.timeline.as_ref()
            && !timeline.data.clips.is_empty()
        {
            editor.load_timeline_position(timeline.playhead, false);
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
                    let ended_explorer_drag =
                        !cx.has_active_drag() && editor.explorer.drop_preview.take().is_some();
                    if ended_explorer_drag && let Some(timeline) = editor.timeline.as_mut() {
                        timeline.interaction.snap_guide = None;
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
            self.timeline
                .as_ref()
                .map(|timeline| timeline.data.duration(TimelineTime::ONE_FRAME))
                .unwrap_or(IDLE_UPDATE_INTERVAL)
        } else {
            IDLE_UPDATE_INTERVAL
        }
    }

    fn open_project_folder(&mut self, cx: &mut Context<Self>) {
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
                    eprintln!("Could not open project folder: {error}");
                    return;
                }
                Err(error) => {
                    eprintln!("Folder dialog failed: {error}");
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
        let active_timeline = match load_existing(&root, preferred_timeline.as_deref()) {
            Ok(active_timeline) => active_timeline,
            Err(error) => {
                eprintln!("Could not open timeline: {error}");
                return;
            }
        };
        self.save_timeline();
        self.project_root = root;
        self.explorer.expanded_directories.clear();
        self.explorer.root_expanded = true;
        self.activate_timeline(active_timeline, cx);
        if let Err(error) = save_project_root(&self.project_root) {
            eprintln!("{error}");
        }
    }

    pub(super) fn open_timeline(&mut self, relative_path: PathBuf, cx: &mut Context<Self>) {
        if self
            .timeline
            .as_ref()
            .is_some_and(|timeline| timeline.path == relative_path)
        {
            self.explorer.selected_file = Some(relative_path);
            self.preview.target = PreviewTarget::Timeline;
            cx.notify();
            return;
        }
        let path = self.project_root.join(&relative_path);
        let timeline = match Timeline::load(&path) {
            Ok(timeline) => timeline,
            Err(error) => {
                eprintln!("Could not open timeline: {error}");
                return;
            }
        };
        self.save_timeline();
        self.activate_timeline(Some((relative_path.clone(), timeline)), cx);
        self.status = Some(format!("Opened {}", relative_path.display()));
    }

    /// Saves the current timeline, switches to a freshly created one, and reveals it in
    /// the file tree. `relative_directory` is only used to expand the containing folder.
    fn activate_created_timeline(
        &mut self,
        relative_directory: PathBuf,
        relative_path: PathBuf,
        timeline: Timeline,
        cx: &mut Context<Self>,
    ) {
        self.save_timeline();
        // Expand the target folder so the new timeline is visible in the tree.
        if !relative_directory.as_os_str().is_empty() {
            self.explorer
                .expanded_directories
                .insert(relative_directory);
        }
        self.activate_timeline(Some((relative_path.clone(), timeline)), cx);
        self.refresh_file_tree();
        self.status = Some(format!("Created {}", relative_path.display()));
    }

    fn activate_timeline(
        &mut self,
        active_timeline: Option<(PathBuf, Timeline)>,
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
        self.preview.timeline_drag = None;
        self.properties.transform_input_clip_id = None;
        self.properties.opacity_drag = None;
        self.preview.refresh_ticks = 2;
        self.timeline = active_timeline.map(|(path, data)| TimelineState::new(path, data));
        self.explorer.search_query = None;
        self.explorer.search_results.clear();
        self.explorer.search_pending = false;
        self.explorer
            .filter
            .update(cx, |filter, cx| filter.clear(cx));
        self.explorer.selected_file = self.timeline.as_ref().map(|timeline| timeline.path.clone());
        self.explorer.context_menu = None;
        self.preview.target = PreviewTarget::Timeline;
        self.refresh_file_tree();
        if let Some(timeline) = self.timeline.as_ref() {
            if let Err(error) = save_active_timeline(&self.project_root, &timeline.path) {
                eprintln!("{error}");
            }
            if !timeline.data.clips.is_empty() {
                self.load_timeline_position(timeline.playhead, false);
            }
        }
    }

    /// Refreshes derived media caches and starts one missing generation/load job.
    ///
    /// Runs on the file-tree tick rather than during rendering so the timeline can read
    /// `media_cache_ready` without touching the filesystem on every frame.
    fn schedule_missing_media_cache(&mut self, cx: &mut Context<Self>) {
        let Some(timeline) = self.timeline.as_ref() else {
            self.media_cache_ready.clear();
            self.waveform_cache.clear();
            return;
        };
        let referenced_asset_ids = timeline
            .data
            .clips
            .iter()
            .map(|clip| clip.asset_id)
            .collect::<HashSet<_>>();
        self.media_cache_ready = timeline
            .data
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
        let Some(asset) = timeline
            .data
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
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preview.fullscreen = !self.preview.fullscreen;
        cx.notify();
    }

    fn action_exit_fullscreen(
        &mut self,
        _: &ExitFullscreen,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.preview.fullscreen {
            self.preview.fullscreen = false;
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
