use crate::video_backend::Video;
use gpui::{
    App, Context, CursorStyle, DragMoveEvent, Entity, FocusHandle, KeyBinding, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, ObjectFit, PathPromptOptions, Render,
    ScrollHandle, ScrollWheelEvent, Window, actions, div, img, prelude::*, px, rgb,
};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

mod editing;
mod explorer;
mod explorer_filter;
mod export;
mod media_cache;
mod model;
mod preview;
mod preview_audio;
mod preview_transform;
mod properties;
mod properties_transform;
mod settings;
mod timeline;
mod timeline_interactions;
mod timeline_video;
mod track;
mod view;
mod workspace;

use crate::playback_view::{DragPhase, PlaybackViewDelegate};
use editing::ClipClipboard;
use explorer::{ExplorerDropPreview, ExplorerMediaDrag, FileContextMenu, PendingExplorerDrop};
use explorer_filter::ExplorerFilter;
use export::export_project;
use model::{
    AudioClipProperties, DEFAULT_IMAGE_CLIP_DURATION, FrameRate, MediaAsset, MediaKind, Project,
    TimelineClip, TimelineTime, TimelineTrack, TrackKind, VideoClipProperties, probe_audio,
    probe_image, probe_media,
};
use preview::PreviewTarget;
use preview_audio::AudioPreview;
use properties::PropertiesPanelResizeDrag;
use timeline_interactions::{
    ClipMoveDrag, ClipPlacement, MarqueeSelection, TimelineTool, TrimDrag, TrimEdge,
};
use workspace::{FileTreeEntry, load_project_root, save_project_root, visible_tree};

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
const SCRUB_SEEK_INTERVAL: Duration = Duration::from_millis(50);
const IDLE_UPDATE_INTERVAL: Duration = Duration::from_millis(33);

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
    explorer_drag_assets: HashMap<PathBuf, MediaAsset>,
    explorer_drag_probe_jobs: HashSet<PathBuf>,
    explorer_drop_preview: Option<ExplorerDropPreview>,
    pending_explorer_drop: Option<PendingExplorerDrop>,
    preview_target: PreviewTarget,
    media_cache_jobs: HashSet<u64>,
    media_cache_ready: HashSet<u64>,
    waveform_cache: HashMap<u64, Arc<media_cache::WaveformData>>,
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
    clip_clipboard: Option<ClipClipboard>,
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
    properties_panel_width: f32,
    is_resizing_properties_panel: bool,
    settings_open: bool,
    pixels_per_second: f32,
    active_timeline_tool: TimelineTool,
    blade_guide_position: Option<TimelineTime>,
    snapping_enabled: bool,
    snap_guide: Option<TimelineTime>,
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
            explorer_drag_assets: HashMap::new(),
            explorer_drag_probe_jobs: HashSet::new(),
            explorer_drop_preview: None,
            pending_explorer_drop: None,
            preview_target: PreviewTarget::Timeline,
            media_cache_jobs: HashSet::new(),
            media_cache_ready: HashSet::new(),
            waveform_cache: HashMap::new(),
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
            clip_clipboard: None,
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
            properties_panel_width: DEFAULT_PROPERTIES_PANEL_WIDTH,
            is_resizing_properties_panel: false,
            settings_open: false,
            pixels_per_second: 72.0,
            active_timeline_tool: TimelineTool::Selection,
            blade_guide_position: None,
            snapping_enabled: true,
            snap_guide: None,
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
            let mut update_interval = IDLE_UPDATE_INTERVAL;
            loop {
                cx.background_executor().timer(update_interval).await;
                match editor.update(cx, |editor, cx| {
                    let refresh_tree = editor.last_tree_scan.elapsed() >= Duration::from_secs(1);
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
                    let ended_explorer_drag =
                        !cx.has_active_drag() && editor.explorer_drop_preview.take().is_some();
                    if ended_explorer_drag {
                        editor.snap_guide = None;
                    }
                    let should_render = editor.playing
                        || file_preview_playing
                        || editor.preview_refresh_ticks > 0
                        || refresh_tree
                        || pinch_zoomed
                        || ended_explorer_drag;
                    editor.preview_refresh_ticks = editor.preview_refresh_ticks.saturating_sub(1);
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
        if self.playing {
            self.project.duration(TimelineTime::ONE_FRAME)
        } else {
            IDLE_UPDATE_INTERVAL
        }
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
        self.waveform_cache.clear();
        self.explorer_drag_assets.clear();
        self.explorer_drag_probe_jobs.clear();
        self.explorer_drop_preview = None;
        self.pending_explorer_drop = None;
        self.clip_clipboard = None;
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

    /// Refreshes derived media caches and starts one missing generation/load job.
    ///
    /// Runs on the file-tree tick rather than during rendering so the timeline can read
    /// `media_cache_ready` without touching the filesystem on every frame.
    fn schedule_missing_media_cache(&mut self, cx: &mut Context<Self>) {
        let referenced_asset_ids = self
            .project
            .clips
            .iter()
            .filter_map(|clip| clip.asset_id)
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
