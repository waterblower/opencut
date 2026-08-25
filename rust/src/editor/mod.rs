use crate::video::VideoBackend;
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
mod editor;
mod editor_view;
mod explorer;
mod explorer_filter;
mod export;
mod export_dialog;
pub mod export_gstreamer;
mod media_probe;
mod model;
mod preview;
mod preview_audio;
mod preview_image;
mod preview_timeline;
mod preview_video;
mod project_settings;
mod properties;
mod properties_text;
mod properties_transform;
mod settings;
mod timeline;
mod timeline_clip;
mod timeline_clip_menu;
mod timeline_document;
mod timeline_interactions;
mod timeline_track_menu;
mod timeline_ui;
mod timeline_video;
mod track;
mod track_ui;
mod waveform;
mod workspace;

use crate::playback_view::{DragPhase, PlaybackViewDelegate};
use clip_placement::{
    ClipPlacementRejection, validate_clip_placement, validate_text_clip_placement,
};
use editing::{ClipClipboard, EditAction, edit_and_rebuild_timeline, edit_timeline};
pub(crate) use editor::Editor;
use explorer::{
    ExplorerDropPreview, ExplorerMediaDrag, FileContextMenu, FileTreeEntry, NewTimelineDialogState,
    PendingExplorerDrop, RenameDialogState, load_explorer_expansion, visible_tree,
};
use explorer_filter::ExplorerFilter;
use export_dialog::ExportDialogState;
use export_gstreamer::build_ges_timeline;
use media_probe::probe_asset;
use model::{DEFAULT_IMAGE_CLIP_DURATION, MediaAsset, MediaKind};
use preview::{PreviewTarget, update_playback};
use preview_audio::AudioBackend;
use preview_timeline::TimelinePreviewDrag;
use project_settings::{load_project_local_settings, save_project_local_settings};
use properties::{PropertiesPanelResizeDrag, properties_panel};
use properties_text::TextClipInputs;
use properties_transform::VideoTransformInputs;
use timeline::{
    FRAME_RATE_PRESETS, FrameRate, TimelineRuntimeState, TimelineSerialization, TimelineTime,
    timeline_ranges_overlap,
};
#[cfg(test)]
use timeline_clip::AudioClip;
use timeline_clip::{
    AudioClipProperties, Clip, TextClip, TextClipProperties, VideoClip, VideoClipProperties,
};
use timeline_clip_menu::TimelineClipContextMenu;
use timeline_document::{load_existing_timeline, project_timeline_files};
use timeline_interactions::{
    MarqueeSelection, TimelineContextMenu, TimelineInteractionState, TimelineTool,
};
use timeline_track_menu::TextTrackContextMenu;
use timeline_video::create_timeline_video_v2;
use track::{Track, TrackKind};
use ulid::Ulid;
use workspace::{GlobalEditorSettings, load_global_editor_settings, save_global_editor_settings};

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
const IDLE_UPDATE_INTERVAL: Duration = Duration::from_millis(16);

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
    volume_control_open: bool,
    is_scrubbing: bool,
    is_adjusting_volume: bool,
    last_scrub_seek: Option<Instant>,
    timeline_drag: Option<TimelinePreviewDrag>,
}

struct PropertiesPanelState {
    width: f32,
    resizing: bool,
    transform_inputs: VideoTransformInputs,
    transform_input_clip_id: Option<Ulid>,
    text_inputs: TextClipInputs,
    text_input_clip_id: Option<Ulid>,
}

struct ExportState {
    dialog: Option<ExportDialogState>,
    running: bool,
}

impl Editor {
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
                match editor.update(cx, |editor, cx| {
                    let result = editor.set_project_root(root, cx);
                    cx.notify();
                    result
                }) {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => eprintln!("{error}"),
                    Err(error) => {
                        eprintln!("Could not update the editor project folder: {error}");
                    }
                }
            }
        })
        .detach();
    }

    fn set_project_root(&mut self, root: PathBuf, cx: &mut Context<Self>) -> Result<(), String> {
        let root = std::fs::canonicalize(&root).unwrap_or(root);
        let project_settings = load_project_local_settings(&root);
        let active_timeline =
            load_existing_timeline(&root, project_settings.active_timeline.as_deref())?
                .expect("the selected project has no timeline");
        if let Some(timeline) = self.timeline.as_ref() {
            timeline.save(&self.global_settings.project_root);
        }

        self.global_settings.project_root = root;
        self.waveform_jobs.clear();
        self.waveform_cache.clear();
        self.clipboard = None;
        let explorer_expansion = load_explorer_expansion(&self.global_settings.project_root);
        self.explorer.expanded_directories = explorer_expansion.expanded_directories;
        self.explorer.root_expanded = explorer_expansion.root_expanded;
        self.activate_timeline(active_timeline.0, active_timeline.1, cx)?;
        self.schedule_project_waveforms(cx);
        save_global_editor_settings(&self.global_settings)
    }

    pub(super) fn open_timeline(
        &mut self,
        relative_path: PathBuf,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self
            .timeline
            .as_ref()
            .is_some_and(|timeline| timeline.path == relative_path)
        {
            self.explorer.selected_file = Some(relative_path);
            let timeline = self.timeline.as_ref().expect("timeline was checked above");
            let playhead = timeline.playhead;
            if !timeline.data.clips.is_empty() {
                self.load_timeline_position_with_options(playhead, true);
            }
            cx.notify();
            return Ok(());
        }
        let path = self.global_settings.project_root.join(&relative_path);
        let timeline = TimelineSerialization::load(&path)
            .map_err(|error| format!("Could not open timeline: {error}"))?;
        if let Some(timeline) = self.timeline.as_ref() {
            timeline.save(&self.global_settings.project_root);
        }
        self.activate_timeline(relative_path.clone(), timeline, cx)?;
        self.status = Some(format!("Opened {}", relative_path.display()));
        Ok(())
    }

    /// Saves the current timeline, switches to a freshly created one, and reveals it in
    /// the file tree. `relative_directory` is only used to expand the containing folder.
    fn activate_created_timeline(
        &mut self,
        relative_directory: PathBuf,
        relative_path: PathBuf,
        timeline: TimelineSerialization,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if let Some(active_timeline) = self.timeline.as_ref() {
            active_timeline.save(&self.global_settings.project_root);
        }

        // Expand the target folder so the new timeline is visible in the tree.
        if !relative_directory.as_os_str().is_empty() {
            self.explorer
                .expanded_directories
                .insert(relative_directory);
        }
        self.activate_timeline(relative_path.clone(), timeline, cx)?;
        self.explorer
            .refresh_file_tree(&self.global_settings.project_root)?;
        self.save_explorer_expansion()?;
        self.status = Some(format!("Created {}", relative_path.display()));
        Ok(())
    }

    fn activate_timeline(
        &mut self,
        timeline_path: PathBuf,
        active_timeline: TimelineSerialization,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.explorer.drag_assets.clear();
        self.explorer.drag_probe_jobs.clear();
        self.explorer.drop_preview = None;
        self.explorer.pending_drop = None;
        self.preview.volume_control_open = false;
        self.preview.is_scrubbing = false;
        self.preview.is_adjusting_volume = false;
        self.preview.last_scrub_seek = None;
        self.preview.timeline_drag = None;
        self.properties.transform_input_clip_id = None;
        self.properties.text_input_clip_id = None;
        let ges_timeline = build_ges_timeline(
            &active_timeline,
            &self.global_settings.project_root,
            export::ExportOptions::from_timeline(&active_timeline),
        )?;
        self.timeline = Some(TimelineRuntimeState::new(
            timeline_path,
            active_timeline,
            ges_timeline,
        ));
        save_project_local_settings(
            &self.global_settings.project_root,
            self.timeline
                .as_ref()
                .map(|timeline| timeline.path.as_path()),
        )?;
        self.explorer.search_query = None;
        self.explorer.search_results.clear();
        self.explorer.search_pending = false;
        self.explorer
            .filter
            .update(cx, |filter, cx| filter.clear(cx));
        self.explorer.selected_file = self.timeline.as_ref().map(|timeline| timeline.path.clone());
        self.explorer.context_menu = None;
        self.explorer
            .refresh_file_tree(&self.global_settings.project_root)?;
        if let Some(timeline) = self.timeline.as_ref()
            && !timeline.data.clips.is_empty()
        {
            let playhead = timeline.playhead;
            let video = create_timeline_video_v2(&timeline.ges_timeline)?;
            self.preview.target = PreviewTarget::Timeline(video);
            self.load_timeline_position_with_options(playhead, true);
        } else {
            self.preview.target = PreviewTarget::None;
        }
        self.schedule_active_timeline_waveforms(cx);
        Ok(())
    }

    fn schedule_project_waveforms(&mut self, cx: &mut Context<Self>) {
        let mut paths = HashSet::new();
        let timeline_paths = match project_timeline_files(&self.global_settings.project_root) {
            Ok(paths) => paths,
            Err(error) => {
                eprintln!("Could not scan project timelines for waveforms: {error}");
                return;
            }
        };
        for timeline_path in timeline_paths {
            let timeline = match TimelineSerialization::load(
                &self.global_settings.project_root.join(&timeline_path),
            ) {
                Ok(timeline) => timeline,
                Err(error) => {
                    eprintln!("Could not scan timeline for waveforms: {error}");
                    continue;
                }
            };
            let referenced_assets = timeline
                .clips
                .iter()
                .filter_map(|clip| clip.media().map(|clip| clip.asset_id))
                .collect::<HashSet<_>>();
            paths.extend(
                timeline
                    .assets
                    .into_iter()
                    .filter(|asset| asset.has_audio && referenced_assets.contains(&asset.id))
                    .map(|asset| asset.path),
            );
        }
        self.schedule_waveforms(paths, cx);
    }

    pub(super) fn schedule_active_timeline_waveforms(&mut self, cx: &mut Context<Self>) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let referenced_assets = timeline
            .data
            .clips
            .iter()
            .filter_map(|clip| clip.media().map(|clip| clip.asset_id))
            .collect::<HashSet<_>>();
        let paths = timeline
            .data
            .assets
            .iter()
            .filter(|asset| asset.has_audio && referenced_assets.contains(&asset.id))
            .map(|asset| asset.path.clone())
            .collect::<Vec<_>>();
        self.schedule_waveforms(paths, cx);
    }

    fn schedule_waveforms(
        &mut self,
        paths: impl IntoIterator<Item = PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let mut paths = paths
            .into_iter()
            .filter(|path| {
                !self.waveform_cache.contains_key(path) && !self.waveform_jobs.contains(path)
            })
            .collect::<Vec<_>>();
        if paths.is_empty() {
            return;
        }
        paths.sort();
        self.waveform_jobs.extend(paths.iter().cloned());
        let project_root = self.global_settings.project_root.clone();
        cx.spawn(async move |editor, cx| {
            for relative_path in paths {
                let source = project_root.join(&relative_path);
                let result = cx
                    .background_executor()
                    .spawn(async move { waveform::generate_waveform(&source) })
                    .await;
                let current_project = editor
                    .update(cx, |editor, cx| {
                        if editor.global_settings.project_root != project_root {
                            return false;
                        }
                        editor.waveform_jobs.remove(&relative_path);
                        match result {
                            Ok(waveform) => {
                                editor
                                    .waveform_cache
                                    .insert(relative_path.clone(), Arc::new(waveform));
                            }
                            Err(error) => eprintln!("Waveform: {error}"),
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !current_project {
                    break;
                }
            }
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
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.blade_at_playhead(&mut self.preview, &self.global_settings.project_root);
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
        self.paste_clips(cx);
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
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.activate_timeline_tool(TimelineTool::Selection);
        cx.notify();
    }

    fn action_activate_blade_tool(
        &mut self,
        _: &ActivateBladeTool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.activate_timeline_tool(TimelineTool::Blade);
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
#[path = "mod.test.rs"]
mod tests;

#[cfg(test)]
use tests::ulid;
