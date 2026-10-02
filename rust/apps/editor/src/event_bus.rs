use crate::edit_action::{EditAction, apply_timeline_edit};
use crate::editing::validate_clips_placements;
use crate::editor::Editor;
use crate::explorer_drag::AssetBeingDragged;
use crate::explorer_file_entry::select_preview_file;
use crate::generic_containers::HorizontalSplitState;
use crate::global_settings::GlobalEditorSettings;
use crate::layout::{RULER_HEIGHT, TIMELINE_PADDING, TRACK_HEIGHT};
use crate::model::MediaKind;
use crate::preview_events::PreviewEvent;
use crate::project_settings::{ProjectLocalSettings, save_project_local_settings};
use crate::srt::{srt_text_clips, write_srt};
use crate::timeline::{PreviewDropAsset, TimelineFrameIndex};
use crate::timeline_clip::Clip;
use crate::transcription::start_transcription;
use crate::{OpenProject, open_editor_window, quit_after_last_window};
use anyhow::{Context as _, Result, anyhow, bail};
use gpui::{AsyncApp, Bounds, Entity, EventEmitter, MouseMoveEvent, Pixels};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use ulid::Ulid;

pub struct EventBus;

#[derive(Clone, Debug)]
pub enum AppEvent {
    Preview(PreviewEvent),
    SwitchProject {
        project_path: PathBuf,
    },
    /// Transcribe the audio or video file at this absolute path.
    Transcribe {
        source_path: PathBuf,
        project_root: PathBuf,
    },
    HorizontalSplitResized(HorizontalSplitState),
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
}

pub async fn handle_event(
    cx: &mut AsyncApp,
    project: Entity<OpenProject>,
    event_bus: Entity<EventBus>,
    event: AppEvent,
) {
    eprintln!("handle_event: {:?}", event);
    match event {
        AppEvent::Transcribe {
            source_path,
            project_root,
        } => {
            let settings = match GlobalEditorSettings::load() {
                Ok(settings) => settings,
                Err(error) => {
                    log::error!("Could not load settings: {error:?}");
                    return;
                }
            };
            let api_key = settings.minimax_api_key;
            let task = cx.update(|cx| {
                gpui_tokio::Tokio::spawn(cx, async move {
                    let srt = start_transcription(source_path.clone(), api_key).await?;
                    log::info!("Writing SRT for {}", source_path.display());
                    let Some(stem) = source_path.file_stem() else {
                        bail!(
                            "transcription source has no filename: {}",
                            source_path.display()
                        );
                    };
                    let stem = stem.to_string_lossy();
                    let path = project_root.join(format!("{stem}.srt"));
                    write_srt(&path, &srt)?;
                    Ok(path)
                })
            });
            let result = match task.await {
                Ok(result) => result,
                Err(error) => Err(anyhow!("transcription task failed: {error}")),
            };
            match result {
                Ok(path) => log::info!("SRT saved: {}", path.display()),
                Err(error) => log::error!("SRT generation failed: {error:?}"),
            }
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
                project.window = open_editor_window(root.clone(), event_bus, cx);
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
    match &event {
        AppEvent::Preview(preview_event) => match preview_event {
            PreviewEvent::SelectFile(path) => {
                select_preview_file(editor.downgrade(), path.clone(), cx).await?;
            }
            PreviewEvent::TogglePlayback => {
                editor.update(cx, |editor, cx| -> Result<()> {
                    editor.toggle_preview_playback(cx)?;
                    cx.notify();
                    Ok(())
                })?;
            }
        },
        AppEvent::SwitchProject { .. } | AppEvent::Transcribe { .. } => {
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
                        upper_space_split_state: state.clone(),
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
                timeline.record_editing_history();
                apply_timeline_edit(timeline, edit_action.clone())
                    .expect("event bus edit actions cannot be rejected");
                timeline.save()?;
                cx.notify();
                Ok(())
            })?;
        }
        AppEvent::DragStarted(asset) => {
            editor.update(cx, |editor, cx| {
                editor.active_asset_drag = asset.clone();
                cx.notify();
            });
        }
        AppEvent::DragMove(event) => {
            editor.update(cx, |editor, cx| {
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
                        .editing_state
                        .tracks
                        .get(track_index)
                        .map(|track| track.id)
                })();

                if let (Some(timeline), Some(track_id)) = (timeline, on_track) {
                    let local_x =
                        f32::from(event.event.position.x) - f32::from(event.bounds.left());
                    let start_time = timeline.editing_state.nearest_time(
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
            editor.update(cx, |editor, cx| -> Result<()> {
                editor.active_asset_drag = AssetBeingDragged::None;
                let Some(timeline) = editor.timeline.as_mut() else {
                    return Ok(());
                };
                let Some(preview) = timeline.preview_drop_asset.take() else {
                    return Ok(());
                };

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
                        timeline.record_editing_history();
                        apply_timeline_edit(
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
                        let relative_path = asset
                            .absolute_path
                            .strip_prefix(&editor.project_root)
                            .expect("dragged explorer assets are inside the project root")
                            .to_path_buf();
                        editor.place_explorer_asset(
                            relative_path,
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
            eprintln!("TimelineSeek: {frame_index:?}");
        }
        AppEvent::OpenTimeline { path } => {
            editor.update(cx, |editor, cx| -> Result<()> {
                editor.open_timeline(path.clone(), cx)?;
                cx.notify();
                Ok(())
            })?;
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct AssetDragMoveEvent {
    pub event: MouseMoveEvent,
    pub bounds: Bounds<Pixels>,
}

impl EventEmitter<AppEvent> for EventBus {}
