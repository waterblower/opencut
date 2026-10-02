use crate::context_menu::ContextMenu;
use crate::editing::ClipClipboard;
use crate::event_bus::{AppEvent, EventBus};
use crate::explorer::{ExplorerState, load_explorer_expansion};
use crate::explorer_drag::AssetBeingDragged;
use crate::explorer_file_entry::visible_tree;
use crate::generic_containers::{HorizontalSplitState, TextInput};
use crate::preview::{PreviewState, PreviewTarget};
use crate::project_settings::load_project_local_settings;
use crate::properties::PropertiesPanelState;
use crate::properties_transform::VideoTransformInputs;
use crate::timeline::TimelineRuntimeState;
use crate::waveform;
use anyhow::Result;
use gpui::prelude::*;
use gpui::{Entity, FocusHandle, ScrollHandle};
use std::collections::{HashMap, HashSet};
use std::io::{Error as IoError, ErrorKind};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

const IDLE_UPDATE_INTERVAL: Duration = Duration::from_millis(16);

pub(crate) struct Editor {
    // main UI sections
    pub(super) explorer: ExplorerState,
    pub(super) preview: PreviewState,
    pub timeline: Option<TimelineRuntimeState>,

    // other
    pub project_root: PathBuf,
    pub(super) waveform_jobs: HashSet<PathBuf>,
    pub(super) waveform_cache: HashMap<PathBuf, Arc<waveform::WaveformData>>,
    pub(super) properties: PropertiesPanelState,
    pub(super) settings_open: bool,

    pub(super) focus_handle: FocusHandle,
    pub(super) clipboard: Option<ClipClipboard>,
    pub(super) context_menu: ContextMenu,
    pub active_asset_drag: AssetBeingDragged,
    // entities
    pub(super) event_bus: Entity<EventBus>,
    pub(super) upper_split_state: Entity<HorizontalSplitState>,
    pub global_settings_input: Option<Entity<TextInput>>,
}

impl Editor {
    pub(crate) fn new(
        project_root: PathBuf,
        event_bus: Entity<EventBus>,
        cx: &mut Context<Self>,
    ) -> Result<Self> {
        let project_root = std::path::absolute(project_root)?;
        //
        // Load the active timeline
        //
        let timeline = {
            let project_settings = load_project_local_settings(&project_root);

            (|| -> Result<Option<TimelineRuntimeState>> {
                let Some(timeline_path) = project_settings.active_timeline else {
                    return Ok(None);
                };
                let timeline = match TimelineRuntimeState::load(project_root.join(&timeline_path)) {
                    Ok(timeline) => timeline,
                    Err(error) => {
                        // The saved timeline may have been moved or deleted; open
                        // the editor without an active timeline in that case.
                        if error
                            .downcast_ref::<IoError>()
                            .is_some_and(|error| error.kind() == ErrorKind::NotFound)
                        {
                            return Ok(None);
                        }
                        // We still error out for other kinds of errors.
                        return Err(error);
                    }
                };
                Ok(Some(timeline))
            })()?
        };
        // load timeline end

        let focus_handle = cx.focus_handle();
        let explorer = {
            let explorer_filter = cx.new(|cx| {
                TextInput::new_search("explorer-filter", "Filter files…", focus_handle.clone(), cx)
            });
            cx.observe(&explorer_filter, |editor, _, cx| {
                editor.schedule_explorer_search(cx);
                cx.notify();
            })
            .detach();

            let explorer_expansion = load_explorer_expansion(&project_root);
            let expanded_directories = explorer_expansion.expanded_directories;
            let file_tree = visible_tree(&project_root, &expanded_directories).unwrap_or_default();
            ExplorerState {
                file_tree,
                expanded_directories,
                root_expanded: explorer_expansion.root_expanded,
                filter: explorer_filter,
                search_query: None,
                search_results: Vec::new(),
                search_pending: false,
                scroll: ScrollHandle::new(),
                selected_file: timeline.as_ref().and_then(|timeline| {
                    timeline
                        .path
                        .strip_prefix(&project_root)
                        .ok()
                        .map(Path::to_path_buf)
                }),
                rename_dialog: None,
                new_timeline_dialog: None,

                last_tree_scan: Instant::now(),
            }
        };

        let preview = PreviewState {
            target: PreviewTarget::None,
            fullscreen: false,
        };

        let properties = {
            let video_transform_inputs = VideoTransformInputs::new(focus_handle.clone(), cx);
            Self::observe_video_transform_inputs(&video_transform_inputs, cx);

            PropertiesPanelState {
                transform_inputs: video_transform_inputs,
                transform_input_clip_id: None,
                text_input_clip_id: None,
            }
        };

        start_updates(cx);
        let project_local_settings = load_project_local_settings(&project_root);
        let mut editor = Self {
            // Entities
            event_bus,
            upper_split_state: cx.new(|_| project_local_settings.upper_space_split_state),
            //
            project_root,
            explorer,
            preview,
            waveform_jobs: HashSet::new(),
            waveform_cache: HashMap::new(),
            properties,
            settings_open: false,
            global_settings_input: None,
            timeline,
            clipboard: None,

            focus_handle,
            context_menu: ContextMenu::None,
            active_asset_drag: AssetBeingDragged::None,
        };
        if let Some(timeline) = editor.timeline.as_mut() {
            timeline.seek_frame(timeline.playhead());
        }
        editor.preview.target = editor.create_timeline_preview(cx)?;

        // todo:
        // instead of have an async starting here
        // with a sync signature, so that we increase indirect
        // we should submit an event to compute waveforms
        // and update the UI using real async functions
        editor.schedule_project_waveforms(cx);
        Ok(editor)
    }

    pub fn emit_event(&mut self, cx: &mut Context<Self>, event: AppEvent) {
        self.event_bus.update(cx, |_, cx| cx.emit(event));
    }
}

fn start_updates(cx: &mut Context<Editor>) {
    cx.spawn(async move |editor, cx| {
        loop {
            cx.background_executor().timer(IDLE_UPDATE_INTERVAL).await;
            let result = editor.update(cx, |editor, cx| -> Result<()> {
                let pinch_zoomed = editor.apply_timeline_pinch()?;
                let ended_explorer_drag = !cx.has_active_drag();
                if ended_explorer_drag && let Some(timeline) = editor.timeline.as_mut() {
                    timeline.interaction.snap_guide = None;
                }
                let refresh_tree =
                    editor.explorer.last_tree_scan.elapsed() >= Duration::from_secs(1);

                let should_render = refresh_tree || pinch_zoomed || ended_explorer_drag;

                if refresh_tree {
                    editor.explorer.refresh_file_tree(&editor.project_root)?;
                }
                if should_render {
                    cx.notify();
                }
                Ok(())
            });
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => eprintln!("{error:?}"),
                Err(_) => {
                    break;
                }
            }
        }
    })
    .detach();
}
