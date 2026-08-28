use crate::editor::explorer::ExplorerState;

use super::*;

pub(crate) struct Editor {
    // main UI sections
    pub(super) explorer: ExplorerState,
    pub(super) preview: PreviewState,
    pub(super) timeline: Option<TimelineRuntimeState>,
    // other
    pub(super) global_settings: GlobalEditorSettings,
    pub(super) waveform_jobs: HashSet<PathBuf>,
    pub(super) waveform_cache: HashMap<PathBuf, Arc<waveform::WaveformData>>,
    pub(super) properties: PropertiesPanelState,
    pub(super) settings_open: bool,
    pub(super) export: ExportState,
    pub(super) status: Option<String>,
    pub(super) focus_handle: FocusHandle,
    pub(super) clipboard: Option<ClipClipboard>,
    pub(super) event_bus: Entity<EventBus>,
    pub(super) context_menu: ContextMenu,
}

impl Editor {
    pub(crate) fn new(cx: &mut Context<Self>) -> Self {
        gstreamer_editing_services::init()
            .expect("could not initialize GStreamer Editing Services");

        let global_settings = load_global_editor_settings();

        //
        // Load the active timeline
        //
        let timeline = {
            let project_settings = load_project_local_settings(&global_settings.project_root);
            let active_timeline = load_existing_timeline(
                &global_settings.project_root,
                project_settings.active_timeline.as_deref(),
            )
            .unwrap();
            (|| {
                let Some((timeline_path, timeline_data)) = active_timeline else {
                    return None;
                };
                let ges_timeline = build_ges_timeline(
                    &timeline_data,
                    &global_settings.project_root,
                    export::ExportOptions::from_timeline(&timeline_data),
                )
                .unwrap();
                Some(TimelineRuntimeState::new(
                    timeline_path,
                    timeline_data,
                    ges_timeline,
                ))
            })()
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

            let explorer_expansion = load_explorer_expansion(&global_settings.project_root);
            let expanded_directories = explorer_expansion.expanded_directories;
            let file_tree = visible_tree(&global_settings.project_root, &expanded_directories)
                .unwrap_or_default();
            ExplorerState {
                file_tree,
                expanded_directories,
                root_expanded: explorer_expansion.root_expanded,
                filter: explorer_filter,
                search_query: None,
                search_results: Vec::new(),
                search_pending: false,
                scroll: ScrollHandle::new(),
                selected_file: timeline.as_ref().map(|timeline| timeline.path.clone()),
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
            timeline_drag: None,
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

        let export = ExportState {
            dialog: None,
            running: false,
        };

        start_updates(cx);
        let event_bus = cx.new(|_| EventBus {});
        cx.subscribe(&event_bus, handle_app_event).detach();
        let mut editor = Self {
            global_settings,
            explorer,
            preview,
            waveform_jobs: HashSet::new(),
            waveform_cache: HashMap::new(),
            properties,
            settings_open: false,
            export,
            timeline,
            clipboard: None,
            status: None,
            focus_handle,
            event_bus,
            context_menu: ContextMenu::None,
        };
        if let Some(timeline) = editor.timeline.as_ref()
            && !timeline.data.clips.is_empty()
        {
            match create_timeline_video(&timeline.ges_timeline) {
                Ok(video) => {
                    let playhead = timeline.playhead;
                    editor.preview.target = PreviewTarget::Timeline(video);
                    editor.load_timeline_position_with_options(playhead, true);
                }
                Err(error) => eprintln!("{error}"),
            }
        }
        editor.schedule_project_waveforms(cx);
        editor
    }
}

fn handle_app_event(
    editor: &mut Editor,
    _: Entity<EventBus>,
    event: &AppEvent,
    cx: &mut Context<Editor>,
) {
    match event {
        AppEvent::Edit(edit_action) => {
            let project_root = editor.global_settings.project_root.clone();
            let Some(timeline) = editor.timeline.as_mut() else {
                return;
            };
            timeline.record_editing_history();
            edit_and_rebuild_timeline(
                &mut editor.preview,
                &project_root,
                timeline,
                edit_action.clone(),
            )
            .expect("event bus edit actions cannot be rejected");
            timeline.save(&project_root);
        }
        AppEvent::DragMove(event) => {
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

                timeline.data.tracks.get(track_index).map(|track| track.id)
            })();

            if let (Some(timeline), Some(track_id)) = (timeline, on_track) {
                let local_x = f32::from(event.event.position.x) - f32::from(event.bounds.left());
                let start_time = timeline.data.nearest_time(
                    ((local_x - TIMELINE_PADDING) / timeline.data.view.pixels_per_second).max(0.0)
                        as f64,
                );
                timeline.preview_drop_asset = Some(PreviewDropAsset {
                    track_id,
                    start_time,
                    asset: event.drag.clone(),
                });
            }
        }
        AppEvent::DragDrop => {
            let Some(timeline) = editor.timeline.as_mut() else {
                return;
            };
            let Some(preview) = timeline.preview_drop_asset.take() else {
                return;
            };
            let AssetBeingDragged::V1(asset) = preview.asset else {
                return;
            };
            if !matches!(asset.metadata.kind, MediaKind::Video | MediaKind::Audio) {
                return;
            }
            let relative_path = asset
                .absolute_path
                .strip_prefix(&editor.global_settings.project_root)
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
    }

    cx.notify();
}

fn start_updates(cx: &mut Context<Editor>) {
    cx.spawn(async move |editor, cx| {
        loop {
            cx.background_executor().timer(IDLE_UPDATE_INTERVAL).await;
            let result = editor.update(cx, |editor, cx| {
                let preview_playing = match &editor.preview.target {
                    PreviewTarget::Timeline(video) | PreviewTarget::VideoFile(_, video) => {
                        !video.paused()
                    }
                    PreviewTarget::AudioFile(_, audio) => audio.playing(),
                    _ => false,
                };
                let pinch_zoomed = editor.apply_timeline_pinch();
                let ended_explorer_drag = !cx.has_active_drag();
                if ended_explorer_drag && let Some(timeline) = editor.timeline.as_mut() {
                    timeline.interaction.snap_guide = None;
                }
                let refresh_tree =
                    editor.explorer.last_tree_scan.elapsed() >= Duration::from_secs(1);

                let should_render = preview_playing
                    || editor.export.running
                    || refresh_tree
                    || pinch_zoomed
                    || ended_explorer_drag;

                if refresh_tree {
                    editor
                        .explorer
                        .refresh_file_tree(&editor.global_settings.project_root)?;
                }
                if let Some(timeline) = editor.timeline.as_mut() {
                    update_playback(timeline, &mut editor.preview);
                }
                if should_render {
                    cx.notify();
                }
                Ok::<(), anyhow::Error>(())
            });
            match result {
                Ok(Ok(())) => {}
                Ok(Err(error)) => eprintln!("{error}"),
                Err(error) => {
                    eprintln!("Editor update loop failed: {error}");
                    panic!("Editor update loop failed");
                }
            }
        }
    })
    .detach();
}
