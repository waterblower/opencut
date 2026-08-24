use super::*;

impl Editor {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        gstreamer_editing_services::init()
            .expect("could not initialize GStreamer Editing Services");

        let global_settings = load_global_editor_settings();
        let explorer_expansion = load_explorer_expansion(&global_settings.project_root);
        let expanded_directories = explorer_expansion.expanded_directories;
        let file_tree =
            visible_tree(&global_settings.project_root, &expanded_directories).unwrap_or_default();
        let project_settings = load_project_local_settings(&global_settings.project_root);

        //
        // Load the active timeline
        //
        let timeline = {
            let active_timeline = load_existing_timeline(
                &global_settings.project_root,
                project_settings.active_timeline.as_deref(),
            )
            .unwrap();
            (|| {
                let Some((timeline_path, timeline_data)) = active_timeline else {
                    return None;
                };
                let ges_timeline = build_timeline(
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
        let explorer_filter = cx.new(|cx| ExplorerFilter::new(focus_handle.clone(), cx));
        let video_transform_inputs = VideoTransformInputs::new(focus_handle.clone(), cx);
        Self::observe_video_transform_inputs(&video_transform_inputs, cx);
        let text_clip_inputs = TextClipInputs::new(focus_handle.clone(), cx);
        Self::observe_text_clip_inputs(&text_clip_inputs, cx);
        cx.observe(&explorer_filter, |editor, _, cx| {
            editor.schedule_explorer_search(cx);
            cx.notify();
        })
        .detach();
        focus_handle.focus(window, cx);
        start_updates(cx);

        let mut editor = Self {
            global_settings,
            explorer: ExplorerState {
                file_tree,
                expanded_directories,
                root_expanded: explorer_expansion.root_expanded,
                filter: explorer_filter,
                search_query: None,
                search_results: Vec::new(),
                search_pending: false,
                scroll: ScrollHandle::new(),
                selected_file: timeline.as_ref().map(|timeline| timeline.path.clone()),
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
                target: PreviewTarget::None,
                fullscreen: false,
                volume_control_open: false,
                is_scrubbing: false,
                is_adjusting_volume: false,
                last_scrub_seek: None,
                timeline_drag: None,
            },
            waveform_jobs: HashSet::new(),
            waveform_cache: HashMap::new(),
            properties: PropertiesPanelState {
                width: DEFAULT_PROPERTIES_PANEL_WIDTH,
                resizing: false,
                transform_inputs: video_transform_inputs,
                transform_input_clip_id: None,
                text_inputs: text_clip_inputs,
                text_input_clip_id: None,
            },
            settings_open: false,
            export: ExportState {
                dialog: None,
                running: false,
            },
            timeline,
            clipboard: None,
            status: None,
            focus_handle,
        };
        if let Some(timeline) = editor.timeline.as_ref()
            && !timeline.data.clips.is_empty()
        {
            match create_timeline_video(&timeline.data, &editor.global_settings.project_root) {
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
                let ended_explorer_drag =
                    !cx.has_active_drag() && editor.explorer.drop_preview.take().is_some();
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
                Ok::<(), String>(())
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
