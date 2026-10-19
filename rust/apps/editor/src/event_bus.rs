use crate::edit_action::{EditAction, edit_timeline};
use crate::editing::validate_clips_placements;
use crate::editor::Editor;
use crate::explorer_drag::AssetBeingDragged;
use crate::explorer_file_entry::select_preview_file;
use crate::export_window::{ExportState, ExportWindow};
use crate::global_settings::GlobalEditorSettings;
use crate::layout::{RULER_HEIGHT, TIMELINE_PADDING, TRACK_HEIGHT, track_index_at_y};
use crate::media_probe::probe_asset;
use crate::model::MediaKind;
use crate::preview::{PreviewTarget, timeline_preview_target};
use crate::preview_events::PreviewEvent;
use crate::project_settings::{ProjectLocalSettings, save_project_local_settings};
use crate::properties::{
    PropertiesPanel, VideoFilePanel, current_properties_panel, srt_file_panel, text_clip_panel,
    video_clip_panel,
};
use crate::srt::{srt_text_clips, write_srt};
use crate::timeline::{PreviewDropAsset, TimelineFrameIndex, TimelineRuntimeState};
use crate::timeline_clip::Clip;
use crate::transcription_window;
use crate::transcription_window::{ClipRange, TranscriptionStage, TranscriptionWindow};
use crate::ui_components::{HorizontalSplitState, VerticalSplitState};
use crate::{OpenProject, open_editor_window, quit_after_last_window};
use anyhow::{Context as _, Result, anyhow, bail};
use engine::export::{ExportCompletion, ExportControl, ExportOption, export_timeline};
use gpui::prelude::*;
use gpui::{AsyncApp, Bounds, Entity, EventEmitter, MouseMoveEvent, Pixels, WeakEntity};
use player_ui::Seeker;
use player_ui::timeline_player::TimelinePlayer;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use timeline::TimelineSerialization;
use ulid::Ulid;

pub(crate) struct EventBus;

#[derive(Clone, Debug)]
pub(crate) enum AppEvent {
    OpenExportWindow {
        timeline_path: PathBuf,
    },
    ExportTimeline {
        timeline_path: PathBuf,
        document: TimelineSerialization,
        output_path: PathBuf,
        video_bitrate: u64,
        overwrite: bool,
        export_window: WeakEntity<ExportWindow>,
        control: Arc<ExportControl>,
    },
    Preview(PreviewEvent),
    SwitchProject {
        project_path: PathBuf,
    },
    OpenTranscribeWindow {
        audio_source_path: PathBuf, // 含音轨的音频或视频文件的绝对路径。
        clip: Option<ClipRange>,    // 从时间线片段打开时只转写该片段的源区间。
        project_root: PathBuf,
    },
    /// Transcribe the audio or video file at this absolute path, or only `clip`'s part of it.
    Transcribe {
        source_path: PathBuf,
        clip: Option<ClipRange>,
        project_root: PathBuf,
        window: WeakEntity<TranscriptionWindow>,
    },
    HorizontalSplitResized(HorizontalSplitState),
    VerticalSplitResized(VerticalSplitState),
    Edit(EditAction),
    DragStarted(AssetBeingDragged),
    DragMove(AssetDragMoveEvent),
    DragDrop,
    OpenTimeline {
        path: PathBuf,
    },
    TimelineSeek {
        frame_index: TimelineFrameIndex,
    },
    RefreshPropertiesPanel,
    TextClipSelected {
        clip_id: Ulid,
    },
    VideoClipSelected {
        clip_id: Ulid,
    },
    VideoFileSelected {
        path: PathBuf, // 相对于项目根目录
        size_bytes: Option<u64>,
    },
    SrtFileSelected {
        path: PathBuf, // 相对于项目根目录
    },
}

pub(crate) async fn handle_event(
    cx: &mut AsyncApp,
    project: Entity<OpenProject>,
    event_bus: Entity<EventBus>,
    event: AppEvent,
) {
    eprintln!("handle_event: {:?}", event);
    match event {
        AppEvent::ExportTimeline {
            timeline_path,
            document,
            output_path,
            video_bitrate,
            overwrite,
            export_window,
            control,
        } => {
            // macOS 平台必须在主线程创建；线程安全的文字系统交给导出线程。
            let platform_text_system =
                cx.update(|_| gpui_platform::current_platform(true).text_system());
            let result = cx
                .background_executor()
                .spawn(async move {
                    let directory = timeline_path
                        .parent()
                        .context("Timeline path has no parent directory")?;
                    export_timeline(
                        &document,
                        &output_path,
                        &ExportOption {
                            project_root: directory.to_path_buf(),
                            video_bitrate,
                            overwrite,
                        },
                        platform_text_system,
                        &control,
                    )
                })
                .await;
            let state = match result {
                Ok(ExportCompletion::Completed) => ExportState::Complete,
                Ok(ExportCompletion::Stopped) => ExportState::Stopped,
                Err(error) => {
                    log::error!("Could not export timeline: {error:?}");
                    ExportState::Failed
                }
            };
            let _ = export_window.update(cx, |view, cx| {
                view.state = state;
                cx.notify();
            });
        }
        AppEvent::OpenTranscribeWindow {
            audio_source_path: source_path,
            clip,
            project_root,
        } => {
            let audio_duration = match clip {
                Some(range) => range.end.saturating_sub(range.start).as_secs_f64(),
                None => match probe_asset(&source_path) {
                    Ok(asset) => asset.duration,
                    Err(error) => {
                        log::error!("Could not open transcription window: {error:?}");
                        return;
                    }
                },
            };
            if let Err(error) = cx.update(|cx| {
                transcription_window::open(
                    source_path,
                    clip,
                    audio_duration,
                    project_root,
                    event_bus,
                    cx,
                )
            }) {
                log::error!("Could not open transcription window: {error:?}");
            }
        }
        AppEvent::Transcribe {
            source_path,
            clip,
            project_root,
            window,
        } => {
            let Some(view) = window.upgrade() else {
                return;
            };
            if view.read_with(cx, |view, _| view.stage.is_running()) {
                return;
            }
            view.update(cx, |view, cx| {
                view.stage = TranscriptionStage::Preparing;
                view.started = std::time::Instant::now();
                cx.notify();
                view.task = Some(cx.spawn(async move |view, cx| {
                    let refresh = view.update(cx, |_, cx| {
                        cx.spawn(async move |view, cx| {
                            loop {
                                cx.background_executor()
                                    .timer(std::time::Duration::from_millis(250))
                                    .await;
                                if view.update(cx, |_, cx| cx.notify()).is_err() {
                                    break;
                                }
                            }
                        })
                    });
                    let result: Result<PathBuf> = async {
                        // 先确定输出路径，避免解码和付费请求之后才失败。
                        let stem = source_path
                            .file_stem()
                            .context("Transcription source has no filename")?
                            .to_string_lossy();
                        let file_name = match clip {
                            Some(range) => format!("{stem}_{}.srt", range.clip_id),
                            None => format!("{stem}.srt"),
                        };
                        let srt_path = project_root.join(file_name);
                        let (wav, api_key) = cx
                            .update(|cx| {
                                gpui_tokio::Tokio::spawn(cx, async move {
                                    let settings = GlobalEditorSettings::load()?;
                                    log::info!(
                                        "Transcribing audio with MiniMax: {}",
                                        source_path.display()
                                    );
                                    let wav = match clip {
                                        Some(range) => {
                                            transcribe::audio::extract_audio_range_as_wav(
                                                &source_path,
                                                range.start,
                                                range.end,
                                            )?
                                        }
                                        None => {
                                            transcribe::audio::extract_audio_as_wav(&source_path)?
                                        }
                                    };
                                    Ok::<_, anyhow::Error>((wav, settings.minimax_api_key))
                                })
                            })
                            .await
                            .context("Audio preparation task failed")??;
                        let _ = view.update(cx, |view, cx| {
                            view.stage = TranscriptionStage::Transcribing;
                            cx.notify();
                        })?;
                        let srt = cx
                            .update(|cx| {
                                gpui_tokio::Tokio::spawn(cx, async move {
                                    let srt = transcribe::transcribe_wav(
                                        wav,
                                        &api_key,
                                        &transcribe::Options::default(),
                                    )
                                    .await?;
                                    transcribe::subtitles::merge_srt_sections(&srt)
                                })
                            })
                            .await
                            .context("Transcription task failed")??;
                        let _ = view.update(cx, |view, cx| {
                            view.stage = TranscriptionStage::Saving;
                            cx.notify();
                        })?;
                        cx.background_executor()
                            .spawn(async move {
                                write_srt(&srt_path, &srt)?;
                                Ok(srt_path)
                            })
                            .await
                    }
                    .await;
                    drop(refresh);
                    let stage = match result {
                        Ok(path) => TranscriptionStage::Complete(path),
                        Err(error) => {
                            log::error!("SRT generation failed: {error:?}");
                            TranscriptionStage::Failed(format!("{error:#}"))
                        }
                    };
                    let _ = view.update(cx, |view, cx| {
                        view.stage = stage;
                        cx.notify();
                    });
                }));
            });
        }

        AppEvent::SwitchProject { project_path } => {
            let root = match std::fs::canonicalize(&project_path) {
                Ok(root) => root,
                Err(error) => panic!("could not open {}: {error}", project_path.display()),
            };
            let switched = project.update(cx, |project, cx| {
                let ready = match project.window.update(cx, |editor, _, cx| {
                    match editor.prepare_project_switch() {
                        Ok(()) => true,
                        Err(error) => {
                            log::error!(
                                "Could not save timeline before switching projects: {error:?}"
                            );
                            cx.notify();
                            false
                        }
                    }
                }) {
                    Ok(ready) => ready,
                    Err(error) => panic!("could not reach the editor: {error}"),
                };
                if !ready {
                    return false;
                }
                drop(project.close_subscription.take());
                if let Err(error) = project
                    .window
                    .update(cx, |_, window, _| window.remove_window())
                {
                    panic!("could not close editor: {error}");
                }
                project.window = match open_editor_window(root.clone(), event_bus, cx) {
                    Ok(window) => window,
                    Err(error) => panic!("could not open editor window: {error:?}"),
                };
                project.close_subscription = Some(cx.on_window_closed(quit_after_last_window));
                true
            });
            if !switched {
                return;
            }
            let mut settings = match GlobalEditorSettings::load() {
                Ok(settings) => settings,
                Err(error) => {
                    log::error!("Could not load settings: {error:?}");
                    return;
                }
            };
            settings.project_root = root;
            if let Err(error) = settings.save() {
                panic!("could not save project settings: {error}");
            }
        }
        event => {
            let window = project.read_with(cx, |project, _| project.window);
            let editor = match window.entity(cx) {
                Ok(editor) => editor,
                Err(error) => {
                    log::error!("Could not find the editor window: {error:?}");
                    return;
                }
            };
            if let Err(error) = handle_app_event(editor, event, cx).await {
                log::error!("Could not handle the event: {error:?}");
            }
        }
    }
}

async fn handle_app_event(
    editor: Entity<Editor>,
    event: AppEvent,
    cx: &mut AsyncApp,
) -> Result<()> {
    match event {
        AppEvent::OpenExportWindow { timeline_path } => {
            editor.update(cx, |editor, cx| {
                editor.open_export_window(timeline_path, cx)
            })?;
        }
        AppEvent::Preview(preview_event) => match preview_event {
            PreviewEvent::SelectFile(path) => {
                select_preview_file(editor, path, cx).await?;
            }
            PreviewEvent::TogglePlayback => {
                editor.update(cx, |editor, cx| -> Result<()> {
                    editor.toggle_preview_playback(cx)?;
                    cx.notify();
                    Ok(())
                })?;
            }
        },
        AppEvent::SwitchProject { .. }
        | AppEvent::OpenTranscribeWindow { .. }
        | AppEvent::Transcribe { .. }
        | AppEvent::ExportTimeline { .. } => {
            editor.update(cx, |_, cx| cx.notify());
        }
        AppEvent::HorizontalSplitResized(state) => {
            editor.update(cx, |editor, cx| -> Result<()> {
                save_project_local_settings(
                    &editor.project_root,
                    &ProjectLocalSettings {
                        active_timeline: editor.timeline.as_ref().and_then(|timeline| {
                            timeline
                                .path
                                .strip_prefix(&editor.project_root)
                                .ok()
                                .map(Path::to_path_buf)
                        }),
                        upper_space_split_state: state,
                        workspace_split_state: editor.workspace_split_state.read(cx).clone(),
                    },
                )?;
                cx.notify();
                Ok(())
            })?;
        }
        AppEvent::VerticalSplitResized(state) => {
            editor.update(cx, |editor, cx| -> Result<()> {
                save_project_local_settings(
                    &editor.project_root,
                    &ProjectLocalSettings {
                        active_timeline: editor.timeline.as_ref().and_then(|timeline| {
                            timeline
                                .path
                                .strip_prefix(&editor.project_root)
                                .ok()
                                .map(Path::to_path_buf)
                        }),
                        upper_space_split_state: editor.upper_split_state.read(cx).clone(),
                        workspace_split_state: state,
                    },
                )?;
                cx.notify();
                Ok(())
            })?;
        }
        AppEvent::Edit(edit_action) => {
            editor.update(cx, |editor, cx| -> Result<()> {
                let Some(timeline) = editor.timeline.as_mut() else {
                    return Ok(());
                };
                let should_save = edit_timeline(timeline, edit_action)?;
                if should_save {
                    timeline.save()?;
                    // 预览播放器持有编辑前的快照；编辑提交后按新状态重建。
                    match editor.preview.target {
                        PreviewTarget::Timeline { .. } => {
                            editor.preview.target =
                                timeline_preview_target(timeline, &editor.project_root, cx)?;
                        }
                        _ => {
                            // do nothing
                        }
                    }
                }
                cx.notify();
                Ok(())
            })?;
        }
        AppEvent::DragStarted(asset) => {
            editor.update(cx, |editor, cx| {
                editor.active_asset_drag = asset;
                cx.notify();
            });
        }
        AppEvent::RefreshPropertiesPanel => {
            editor.update(cx, |editor, cx| {
                editor.properties_panel = current_properties_panel(editor);
                cx.notify();
            });
        }
        AppEvent::TextClipSelected { clip_id } => {
            editor.update(cx, |editor, cx| -> Result<()> {
                let timeline = editor.timeline.as_ref().context("No timeline is open")?;
                let Some(Clip::Text(clip)) = timeline.editing_state.clip(clip_id) else {
                    bail!("Text clip {clip_id} no longer exists");
                };
                editor.properties_panel =
                    PropertiesPanel::TextClip(text_clip_panel(&timeline.editing_state, clip));
                cx.notify();
                Ok(())
            })?;
        }
        AppEvent::VideoClipSelected { clip_id } => {
            editor.update(cx, |editor, cx| -> Result<()> {
                let timeline = editor.timeline.as_ref().context("No timeline is open")?;
                let Some(Clip::Video(clip)) = timeline.editing_state.clip(clip_id) else {
                    bail!("Video clip {clip_id} no longer exists");
                };
                editor.properties_panel =
                    PropertiesPanel::VideoClip(video_clip_panel(&timeline.editing_state, clip));
                cx.notify();
                Ok(())
            })?;
        }
        AppEvent::VideoFileSelected { path, size_bytes } => {
            editor.update(cx, |editor, cx| {
                editor.properties_panel = PropertiesPanel::VideoFile(VideoFilePanel {
                    path: editor.project_root.join(path),
                    size_bytes,
                });
                cx.notify();
            });
        }
        AppEvent::SrtFileSelected { path } => {
            editor.update(cx, |editor, cx| {
                editor.properties_panel =
                    PropertiesPanel::SrtFile(srt_file_panel(editor.project_root.join(path)));
                cx.notify();
            });
        }
        AppEvent::DragMove(event) => {
            editor.update(cx, |editor, cx| {
                let Some(timeline) = editor.timeline.as_mut() else {
                    return;
                };
                let (new_preview_drop_asset, new_snap_guide) =
                    (|timeline: &TimelineRuntimeState| {
                        let pointer = event.event.position;
                        if !event.bounds.contains(&pointer) {
                            return (None, None);
                        }
                        let track_index = track_index_at_y(
                            f32::from(pointer.y),
                            f32::from(event.bounds.top()) + RULER_HEIGHT,
                            TRACK_HEIGHT,
                        );
                        let Ok(target_track_index) = usize::try_from(track_index) else {
                            return (None, None);
                        };
                        let Some(track) = timeline.editing_state.tracks.get(target_track_index)
                        else {
                            return (None, None);
                        };
                        let local_x = f32::from(pointer.x) - f32::from(event.bounds.left());
                        let raw_start = timeline.editing_state.nearest_time(
                            ((local_x - TIMELINE_PADDING) / timeline.pixels_per_second).max(0.0)
                                as f64,
                        );
                        let (start_time, snap_guide) = match &editor.active_asset_drag {
                            AssetBeingDragged::V1(asset)
                                if matches!(
                                    asset.metadata.kind,
                                    MediaKind::Video | MediaKind::Audio
                                ) =>
                            {
                                let duration = timeline
                                    .editing_state
                                    .nearest_time(asset.metadata.duration)
                                    .max(TimelineFrameIndex::ONE_FRAME);
                                timeline.snap_clip_start_ignoring(
                                    raw_start,
                                    duration,
                                    &HashSet::new(),
                                )
                            }
                            AssetBeingDragged::V1(_) | AssetBeingDragged::Srt(_) => {
                                (raw_start, None)
                            }
                            AssetBeingDragged::None => return (None, None),
                        };
                        let preview = PreviewDropAsset {
                            track_id: track.id,
                            start_time,
                            asset: editor.active_asset_drag.clone(),
                        };
                        let valid_snap_guide =
                            if preview.validate_placement(&timeline.editing_state).is_ok() {
                                snap_guide
                            } else {
                                None
                            };
                        (Some(preview), valid_snap_guide)
                    })(timeline);

                timeline.preview_drop_asset = new_preview_drop_asset;
                timeline.interaction.snap_guide = new_snap_guide;
                cx.notify();
            });
        }
        AppEvent::DragDrop => {
            editor.update(cx, |editor, cx| -> Result<()> {
                editor.active_asset_drag = AssetBeingDragged::None;
                cx.notify();
                let Some(timeline) = editor.timeline.as_mut() else {
                    return Ok(());
                };
                timeline.interaction.snap_guide = None;
                let Some(preview) = timeline.preview_drop_asset.take() else {
                    return Ok(());
                };

                if preview.validate_placement(&timeline.editing_state).is_err() {
                    return Ok(());
                }

                match preview.asset {
                    AssetBeingDragged::Srt(srt) => {
                        let mut text_clips =
                            srt_text_clips(&srt.srt, timeline.editing_state.settings.frame_rate);
                        for clip in &mut text_clips {
                            clip.track_id = preview.track_id;
                            clip.timeline_start += preview.start_time;
                        }
                        let clips = text_clips.into_iter().map(Clip::Text).collect::<Vec<_>>();
                        validate_clips_placements(&timeline.editing_state, &clips)?;

                        let selected_clip_ids = clips.iter().map(Clip::id).collect::<HashSet<_>>();
                        let selected_clip_id = clips.first().map(Clip::id);
                        edit_timeline(
                            timeline,
                            EditAction::AddClips {
                                clips,
                                assets: Vec::new(),
                            },
                        )?;
                        timeline.interaction.selected_clip_ids = selected_clip_ids;
                        timeline.interaction.selected_clip_id = selected_clip_id;
                        timeline.save()?;
                    }
                    AssetBeingDragged::V1(asset) => {
                        if !matches!(asset.metadata.kind, MediaKind::Video | MediaKind::Audio) {
                            return Ok(());
                        }
                        editor.place_explorer_asset(
                            asset.absolute_path.clone(),
                            preview.track_id,
                            preview.start_time,
                            asset.metadata,
                            cx,
                        )?;
                    }
                    AssetBeingDragged::None => return Ok(()),
                }
                cx.notify();
                Ok(())
            })?;
        }
        AppEvent::TimelineSeek { frame_index } => {
            editor.update(cx, |editor, cx| -> Result<()> {
                match &editor.preview.target {
                    PreviewTarget::Timeline { player, .. } => {
                        player.update(cx, |player, cx| {
                            let position =
                                player.backend().timeline().position_at_frame(frame_index);
                            let result = player.seek(position);
                            cx.notify();
                            result
                        })?;
                    }
                    _ => {
                        let Some(timeline) = editor.timeline.as_ref() else {
                            return Err(anyhow!("TimelineSeek arrived while no timeline is open"));
                        };
                        let relative_path = match timeline.path.strip_prefix(&editor.project_root) {
                            Ok(relative_path) => relative_path.to_path_buf(),
                            Err(_) => timeline.path.clone(),
                        };
                        let timeline_directory = timeline
                            .path
                            .parent()
                            .context("Timeline path has no parent directory")?; // 素材路径相对于时间线文件所在目录。
                        let mut timeline_player = TimelinePlayer::from_editing_state(
                            timeline.editing_state.clone(),
                            timeline_directory,
                        )?;
                        timeline_player.title = timeline
                            .path
                            .file_name()
                            .context("Timeline path has no file name")?
                            .to_string_lossy()
                            .into_owned();
                        let position = timeline_player
                            .backend()
                            .timeline()
                            .position_at_frame(frame_index);
                        timeline_player.backend_mut().seek(position)?;
                        let player = cx.new(move |_| timeline_player);
                        editor.preview.target = PreviewTarget::Timeline {
                            _task: player.update(cx, |player, cx| player.start(cx)),
                            _subscription: cx.observe(&player, |_, _, cx| cx.notify()),
                            path: relative_path,
                            player,
                        };
                    }
                }
                Ok(())
            })?;
        }
        AppEvent::OpenTimeline { path } => {
            editor.update(cx, |editor, cx| -> Result<()> {
                if let Err(error) = editor.open_timeline(path.clone(), cx) {
                    log::error!("Could not open timeline: {error:?}");
                    editor.preview.target = PreviewTarget::Error {
                        path,
                        error: format!("{error:?}"),
                    };
                }
                cx.notify();
                Ok(())
            })?;
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub(crate) struct AssetDragMoveEvent {
    pub(crate) event: MouseMoveEvent,
    pub(crate) bounds: Bounds<Pixels>,
}

impl EventEmitter<AppEvent> for EventBus {}
