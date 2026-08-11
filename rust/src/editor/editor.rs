use super::*;

impl Editor {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let global_settings = load_global_editor_settings();
        let expanded_directories = HashSet::new();
        let file_tree =
            visible_tree(&global_settings.project_root, &expanded_directories).unwrap_or_default();
        let active_timeline = match load_existing(&global_settings.project_root, None) {
            Ok(active_timeline) => active_timeline,
            Err(error) => {
                eprintln!("Could not open timeline: {error}");
                None
            }
        };
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
        start_updates(cx);

        let mut editor = Self {
            global_settings,
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
            waveform_jobs: HashSet::new(),
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
            clipboard: None,
            status: None,
            focus_handle,
        };
        if let Some(timeline) = editor.timeline.as_ref()
            && !timeline.data.clips.is_empty()
        {
            editor.load_timeline_position_with_options(timeline.playhead, false, true);
        }
        editor.schedule_project_waveforms(cx);
        editor
    }
}

fn start_updates(cx: &mut Context<Editor>) {
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
