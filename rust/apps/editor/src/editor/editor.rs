use crate::editor::{
    explorer::ExplorerState, preview_events::file_preview_requested,
    project_settings::ProjectLocalSettings,
};
use anyhow::Error;
use gpui::{AsyncApp, WeakEntity};
use std::io::{Error as IoError, ErrorKind};
use std::path::Path;

use super::srt::srt_text_clips;
use super::*;

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
    pub status: Option<String>,
    pub(super) focus_handle: FocusHandle,
    pub(super) clipboard: Option<ClipClipboard>,
    pub(super) context_menu: ContextMenu,
    pub active_asset_drag: AssetBeingDragged,
    // entities
    pub(super) event_bus: Entity<EventBus>,
    pub(super) upper_split_state: Entity<HorizontalSplitState>,
    pub global_settings_input: Option<Entity<gpui_component::input::InputState>>,
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
                let timeline = match TimelineRuntimeState::load(
                    project_root.join(&timeline_path),
                    &project_root,
                ) {
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
            let explorer_filter = cx.new(|cx| ExplorerFilter::new(focus_handle.clone(), cx));
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
            volume_control_open: false,
            is_scrubbing: false,
            is_adjusting_volume: false,
            last_scrub_seek: None,
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
        cx.subscribe(&event_bus, |_, _, event: &AppEvent, cx| {
            let event = event.clone();
            cx.spawn(async move |editor, cx| {
                handle_app_event(editor, event, cx).await;
            })
            .detach();
        })
        .detach();

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
            status: None,
            focus_handle,
            context_menu: ContextMenu::None,
            active_asset_drag: AssetBeingDragged::None,
        };
        if let Some(timeline) = editor.timeline.as_mut() {
            let playhead = timeline.playhead();
            set_timeline_position(&mut editor.preview, &timeline.backend, playhead)?;
        }
        editor.schedule_project_waveforms(cx);
        Ok(editor)
    }

    pub fn emit_event(&mut self, cx: &mut Context<Self>, event: AppEvent) {
        self.event_bus.update(cx, |_, cx| cx.emit(event));
    }
}

async fn handle_app_event(editor: WeakEntity<Editor>, event: AppEvent, cx: &mut AsyncApp) {
    match &event {
        AppEvent::Preview(event) => {
            let Ok(project_root) = editor.update(cx, |editor, _| editor.project_root.clone())
            else {
                return;
            };
            if let Err(error) = Editor::handle_preview_event(editor.clone(), event, cx).await {
                log::error!("Preview action failed: {error:?}");
                let _ = editor.update(cx, |editor, cx| {
                    editor.status = Some(match event {
                        PreviewEvent::SelectFile(path) => {
                            if !file_preview_requested(
                                &editor.project_root,
                                editor.explorer.selected_file.as_deref(),
                                &editor.preview.target,
                                &project_root,
                                path,
                            ) {
                                return;
                            }
                            format!("Could not open {}: {error:#}", path.display())
                        }
                        _ => format!("Preview failed: {error:#}"),
                    });
                    cx.notify();
                });
                return;
            }
        }
        AppEvent::SwitchProject { .. } | AppEvent::Transcribe { .. } => {
            let _ = editor.update(cx, |_, cx| cx.notify());
        }
        AppEvent::HorizontalSplitResized(state) => {
            let _ = editor.update(cx, |editor, cx| {
                if let Err(error) = save_project_local_settings(
                    &editor.project_root,
                    &ProjectLocalSettings {
                        active_timeline: editor.timeline.as_ref().and_then(|timeline| {
                            timeline
                                .path
                                .strip_prefix(&editor.project_root)
                                .ok()
                                .map(Path::to_path_buf)
                        }),
                        upper_space_split_state: state.clone(),
                    },
                ) {
                    log::error!("Could not save project layout: {error:?}");
                }
                cx.notify();
            });
        }
        AppEvent::Edit(edit_action) => {
            let _ = editor.update(cx, |editor, cx| {
                let Some(timeline) = editor.timeline.as_mut() else {
                    return;
                };
                timeline.record_editing_history();
                apply_timeline_edit(&mut editor.preview, timeline, edit_action.clone())
                    .expect("event bus edit actions cannot be rejected");
                if let Err(error) = timeline.save() {
                    log::error!("{error:?}");
                }
                cx.notify();
            });
        }
        AppEvent::DragStarted(asset) => {
            let _ = editor.update(cx, |editor, cx| {
                editor.active_asset_drag = asset.clone();
                cx.notify();
            });
        }
        AppEvent::DragMove(event) => {
            let _ = editor.update(cx, |editor, cx| {
                let timeline = editor.timeline.as_mut();
                let on_track: Option<Ulid> = (|| {
                    let Some(timeline) = timeline.as_deref() else {
                        return None;
                    };
                    let pointer = event.event.position;
                    if !event.bounds.contains(&pointer) {
                        return None;
                    }
                    let local_y = f32::from(pointer.y) - f32::from(event.bounds.top());
                    if local_y < RULER_HEIGHT {
                        return None;
                    }
                    let track_index = ((local_y - RULER_HEIGHT) / TRACK_HEIGHT).floor() as usize;

                    timeline
                        .backend
                        .timeline()
                        .tracks
                        .get(track_index)
                        .map(|track| track.id)
                })();

                if let (Some(timeline), Some(track_id)) = (timeline, on_track) {
                    let local_x =
                        f32::from(event.event.position.x) - f32::from(event.bounds.left());
                    let start_time = timeline.backend.timeline().nearest_time(
                        ((local_x - TIMELINE_PADDING) / timeline.pixels_per_second).max(0.0) as f64,
                    );
                    timeline.preview_drop_asset = Some(PreviewDropAsset {
                        track_id,
                        start_time,
                        asset: editor.active_asset_drag.clone(),
                    });
                }
                cx.notify();
            });
        }
        AppEvent::DragDrop => {
            let _ = editor.update(cx, |editor, cx| {
                editor.active_asset_drag = AssetBeingDragged::None;
                let Some(timeline) = editor.timeline.as_mut() else {
                    return;
                };
                let Some(preview) = timeline.preview_drop_asset.take() else {
                    return;
                };

                match preview.asset {
                    AssetBeingDragged::Srt(srt) => {
                        let result = (|| {
                            let Some(timeline) = editor.timeline.as_mut() else {
                                return Ok(());
                            };
                            let mut text_clips = srt_text_clips(
                                &srt.srt,
                                timeline.backend.timeline().settings.frame_rate,
                            );
                            for clip in &mut text_clips {
                                clip.track_id = preview.track_id;
                                clip.timeline_start += preview.start_time;
                            }
                            let clips = text_clips.into_iter().map(Clip::Text).collect::<Vec<_>>();
                            editing::validate_clips_placements(
                                timeline.backend.timeline(),
                                &clips,
                            )?;

                            let selected_clip_ids =
                                clips.iter().map(Clip::id).collect::<HashSet<_>>();
                            let selected_clip_id = clips.first().map(Clip::id);
                            timeline.record_editing_history();
                            apply_timeline_edit(
                                &mut editor.preview,
                                timeline,
                                EditAction::AddClips {
                                    clips,
                                    assets: Vec::new(),
                                },
                            )?;
                            timeline.interaction.selected_clip_ids = selected_clip_ids;
                            timeline.interaction.selected_clip_id = selected_clip_id;
                            timeline.save()?;
                            editor.status = Some("Added subtitles to the timeline.".to_string());
                            Ok::<(), Error>(())
                        })();
                        if let Err(error) = result {
                            editor.status = Some(format!("Could not add subtitles: {error}"));
                            eprintln!("Could not place dragged subtitles: {error:?}");
                        }
                    }
                    AssetBeingDragged::V1(asset) => {
                        if !matches!(asset.metadata.kind, MediaKind::Video | MediaKind::Audio) {
                            return;
                        }
                        let relative_path = asset
                            .absolute_path
                            .strip_prefix(&editor.project_root)
                            .expect("dragged explorer assets are inside the project root")
                            .to_path_buf();
                        if let Err(error) = editor.place_explorer_asset(
                            relative_path,
                            preview.track_id,
                            preview.start_time,
                            asset.metadata,
                            cx,
                        ) {
                            editor.status = Some(format!("Could not add media: {error}"));
                            eprintln!("Could not place dragged explorer asset: {error:?}");
                        }
                    }
                    AssetBeingDragged::None => return,
                }
                cx.notify();
            });
        }
    }
}

fn start_updates(cx: &mut Context<Editor>) {
    cx.spawn(async move |editor, cx| {
        loop {
            cx.background_executor().timer(IDLE_UPDATE_INTERVAL).await;
            let result = editor.update(cx, |editor, cx| {
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
                Ok::<(), Error>(())
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
