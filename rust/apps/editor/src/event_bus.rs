use crate::editor::edit_action::{EditAction, apply_timeline_edit};
use crate::editor::editing::validate_clips_placements;
use crate::editor::explorer_drag::AssetBeingDragged;
use crate::editor::generic_containers::HorizontalSplitState;
use crate::editor::model::MediaKind;
use crate::editor::preview::PreviewTarget;
use crate::editor::preview_events::PreviewEvent;
use crate::editor::project_settings::{ProjectLocalSettings, save_project_local_settings};
use crate::editor::srt::srt_text_clips;
use crate::editor::timeline::PreviewDropAsset;
use crate::editor::timeline_clip::Clip;
use crate::editor::transcription::start_transcription;
use crate::editor::write_srt;
use crate::editor::{
    Editor, RULER_HEIGHT, TIMELINE_PADDING, TRACK_HEIGHT, global_settings::GlobalEditorSettings,
};
use crate::open_editor_window;
use anyhow::{Error, anyhow, bail};
use gpui::{
    App, AsyncApp, Bounds, Entity, EventEmitter, MouseMoveEvent, Pixels, Subscription, WeakEntity,
    WindowHandle, WindowId,
};
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
}

pub fn handle_event(
    cx: &mut App,
    window: &mut WindowHandle<Editor>,
    event: AppEvent,
    event_bus: Entity<EventBus>,
    close_subscription: &mut Option<Subscription>,
    quit_after_last_window: fn(&mut App, WindowId),
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
            let project_root = project_root.clone();
            let source_path = source_path.clone();
            let task = gpui_tokio::Tokio::spawn(cx, async move {
                let srt = start_transcription(source_path.clone(), api_key).await?;
                log::info!("Writing SRT for {}", source_path.display());
                let Some(stem) = source_path.file_stem() else {
                    bail!(
                        "transcription source has no filename at {}:{}",
                        file!(),
                        line!()
                    );
                };
                let stem = stem.to_string_lossy();
                let path = project_root.join(format!("{stem}.srt"));
                write_srt(&path, &srt)?;
                Ok(path)
            });
            cx.spawn(async move |_| {
                let result = match task.await {
                    Ok(result) => result,
                    Err(error) => Err(anyhow!(
                        "transcription task failed: {error} at {}:{}",
                        file!(),
                        line!()
                    )),
                };
                match result {
                    Ok(path) => log::info!("SRT saved: {}", path.display()),
                    Err(error) => log::error!("SRT generation failed: {error:?}"),
                }
            })
            .detach();
        }
        AppEvent::SwitchProject { project_path } => {
            let root = match std::fs::canonicalize(&project_path) {
                Ok(root) => root,
                Err(error) => panic!(
                    "could not open {}: {error} at {}:{}",
                    project_path.display(),
                    file!(),
                    line!()
                ),
            };
            let ready = window
                .update(cx, |editor, _, cx| match editor.prepare_project_switch() {
                    Ok(()) => true,
                    Err(error) => {
                        log::error!("Could not save timeline before switching projects: {error:?}");
                        cx.notify();
                        false
                    }
                })
                .unwrap();
            if !ready {
                return;
            }
            drop(close_subscription.take());
            if let Err(error) = window.update(cx, |_, window, _| window.remove_window()) {
                panic!("could not close editor: {error} at {}:{}", file!(), line!());
            }
            *window = open_editor_window(root.clone(), event_bus, cx);
            *close_subscription = Some(cx.on_window_closed(quit_after_last_window));
            let mut settings = match GlobalEditorSettings::load() {
                Ok(settings) => settings,
                Err(error) => {
                    log::error!("Could not load settings: {error:?}");
                    return;
                }
            };
            settings.project_root = root;
            if let Err(error) = settings.save() {
                panic!(
                    "could not save project settings: {error} at {}:{}",
                    file!(),
                    line!()
                );
            }
        }
        AppEvent::Preview(_)
        | AppEvent::HorizontalSplitResized(_)
        | AppEvent::Edit(_)
        | AppEvent::DragStarted(_)
        | AppEvent::DragMove(_)
        | AppEvent::DragDrop
        | AppEvent::OpenTimeline { .. } => {
            // These events are handled by the editor subscriber in handle_app_event.
        }
    }
}

pub async fn handle_app_event(editor: WeakEntity<Editor>, event: AppEvent, cx: &mut AsyncApp) {
    eprintln!("handle_app_event: {:?}", event);
    match &event {
        AppEvent::Preview(event) => {
            if let Err(error) = Editor::handle_preview_event(editor.clone(), event, cx).await {
                log::error!("Preview action failed: {error:?}");
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
                            validate_clips_placements(timeline.backend.timeline(), &clips)?;

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
                            Ok::<(), Error>(())
                        })();
                        if let Err(error) = result {
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
                            eprintln!("Could not place dragged explorer asset: {error:?}");
                        }
                    }
                    AssetBeingDragged::None => return,
                }
                cx.notify();
            });
        }
        AppEvent::OpenTimeline { path } => {
            let err = editor.update(cx, |editor, cx| {
                if let Err(error) = editor.open_timeline(path.clone(), cx) {
                    log::error!("Could not open timeline: {error:?}");
                }
                editor.preview.target = PreviewTarget::Timeline;
                cx.notify();
            });
            match err {
                Ok(()) => {}
                Err(error) => {
                    log::error!("{error:?}");
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct AssetDragMoveEvent {
    pub event: MouseMoveEvent,
    pub bounds: Bounds<Pixels>,
}

impl EventEmitter<AppEvent> for EventBus {}
