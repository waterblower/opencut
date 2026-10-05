use crate::event_bus::{AppEvent, EventBus};
use crate::theme::{ACCENT, BACKGROUND, ERROR, MUTED, TEXT};
use anyhow::Result;
use gpui::prelude::*;
use gpui::{
    App, Bounds, Context, Entity, Task, Window, WindowBounds, WindowOptions, div, px, rgb, size,
};
use std::path::PathBuf;
use std::time::Instant;

/// An existing running job is focused instead of submitting the file again.
pub fn open(
    source_path: PathBuf,
    project_root: PathBuf,
    event_bus: Entity<EventBus>,
    cx: &mut App,
) -> Result<Entity<TranscriptionWindow>> {
    for handle in cx.windows() {
        let Some(window) = handle.downcast::<TranscriptionWindow>() else {
            continue;
        };
        let view = window.entity(cx)?;
        if view.read(cx).source_path != source_path {
            continue;
        }
        window.update(cx, |_, window, _| window.activate_window())?;
        view.update(cx, |view, _| {
            if !view.stage.is_running() {
                view.project_root = project_root;
                view.event_bus = event_bus;
            }
        });
        return Ok(view);
    }
    let bounds = Bounds::centered(None, size(px(600.0), px(400.0)), cx);
    let handle = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            focus: true,
            ..WindowOptions::default()
        },
        |window, cx| {
            let view = cx.new(|_| TranscriptionWindow {
                source_path,
                project_root,
                event_bus,
                stage: TranscriptionStage::Idle,
                started: Instant::now(),
                task: None,
            });
            let weak_view = view.downgrade();
            window.on_window_should_close(cx, move |_, cx| match weak_view.upgrade() {
                Some(view) => !view.read(cx).stage.is_running(),
                None => true,
            });
            view
        },
    )?;
    handle.entity(cx)
}

pub struct TranscriptionWindow {
    source_path: PathBuf,
    project_root: PathBuf,
    event_bus: Entity<EventBus>,
    pub stage: TranscriptionStage,
    pub started: Instant,
    pub task: Option<Task<()>>,
}

#[derive(Clone)]
pub enum TranscriptionStage {
    Idle,
    Cancelled,
    Preparing,
    Transcribing,
    Saving,
    Complete(PathBuf),
    Failed(String),
}

impl TranscriptionStage {
    pub fn is_running(&self) -> bool {
        matches!(self, Self::Preparing | Self::Transcribing | Self::Saving)
    }
}

impl Render for TranscriptionWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let stage = self.stage.clone();
        let running = stage.is_running();
        let elapsed = self.started.elapsed();
        let status = match &stage {
            TranscriptionStage::Idle => "Ready",
            TranscriptionStage::Cancelled => "Cancelled",
            TranscriptionStage::Preparing => "Preparing audio",
            TranscriptionStage::Transcribing => "Uploading and transcribing",
            TranscriptionStage::Saving => "Saving subtitles",
            TranscriptionStage::Complete(_) => "Subtitles saved",
            TranscriptionStage::Failed(_) => "Transcription failed",
        };
        div().id("transcription-window").size_full().overflow_y_scroll()
            .flex().flex_col().gap_4().p_6().bg(rgb(BACKGROUND)).text_color(rgb(TEXT))
            .child(div().text_xl().child("Generate SRT"))
            .child(self.source_path.file_name().unwrap_or_default().to_string_lossy().into_owned())
            .child(div().text_color(rgb(ACCENT)).child(if running {
                format!("{status}{}", ".".repeat((elapsed.as_millis() / 500 % 4) as usize))
            } else { status.to_string() }))
            .when(running, |view| view
                .child(format!("Elapsed: {}:{:02}", elapsed.as_secs() / 60, elapsed.as_secs() % 60))
                .child(div().text_sm().text_color(rgb(MUTED)).child(
                    "Cancel stops this local request. A request already sent may continue processing on the server.",
                )))
            .child(match stage {
                TranscriptionStage::Complete(path) => div().flex().flex_col().gap_3()
                    .child(path.display().to_string())
                    .child(div().id("reveal-srt").px_4().py_2().rounded_md().bg(rgb(ACCENT))
                        .text_color(rgb(BACKGROUND)).cursor_pointer().child("Reveal in Finder")
                        .on_click(move |_, _, cx| cx.reveal_path(&path))),
                TranscriptionStage::Failed(error) => div().text_sm().text_color(rgb(ERROR)).child(error),
                TranscriptionStage::Idle | TranscriptionStage::Cancelled | TranscriptionStage::Preparing | TranscriptionStage::Transcribing | TranscriptionStage::Saving => div(),
            })
            .child(div().flex().gap_3()
                .child(div().id("start-transcription").px_4().py_2().rounded_md()
                    .bg(rgb(if running { MUTED } else { ACCENT })).text_color(rgb(BACKGROUND))
                    .child("Start")
                    .when(!running, |button| button.cursor_pointer().on_click(cx.listener(|view, _, _, cx| {
                        let event = AppEvent::Transcribe {
                            source_path: view.source_path.clone(),
                            project_root: view.project_root.clone(),
                            window: cx.entity().downgrade(),
                        };
                        view.event_bus.update(cx, |_, cx| cx.emit(event));
                    }))))
                .child(div().id("cancel-transcription").px_4().py_2().rounded_md()
                    .bg(rgb(MUTED)).child("Cancel")
                    .when(!matches!(self.stage, TranscriptionStage::Saving), |button| {
                        button.cursor_pointer().on_click(cx.listener(|view, _, window, cx| {
                            if view.stage.is_running() {
                                view.task.take();
                                view.stage = TranscriptionStage::Cancelled;
                                cx.notify();
                            } else {
                                window.remove_window();
                            }
                        }))
                    })))
    }
}
