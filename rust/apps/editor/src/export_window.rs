use crate::editor::Editor;
use crate::event_bus::{AppEvent, EventBus};
use crate::generic_containers::TextInput;
use crate::theme::{ACCENT, BACKGROUND, ERROR, MUTED, TEXT};
use anyhow::{Result, ensure};
use engine::export::ExportControl;
use gpui::prelude::*;
use gpui::{Bounds, Context, Entity, Window, WindowBounds, WindowOptions, div, px, rgb, size};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use timeline::TimelineSerialization;

impl Editor {
    pub fn open_export_window(&self, timeline_path: PathBuf, cx: &mut Context<Self>) -> Result<()> {
        let Some(timeline) = self.timeline.as_ref() else {
            return Ok(());
        };
        ensure!(
            timeline.path == timeline_path,
            "The active timeline changed before opening export"
        );
        let document = timeline.to_serialize();
        let event_bus = self.event_bus.clone();
        let bounds = Bounds::centered(None, size(px(560.0), px(520.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                ..WindowOptions::default()
            },
            |window, cx| {
                let view = cx.new(|cx| {
                    let return_focus = cx.focus_handle();
                    let bitrate_input = cx.new(|cx| {
                        TextInput::new_field("export-bitrate", "8".into(), "Mbps", return_focus, cx)
                    });
                    cx.observe(&bitrate_input, |_, _, cx| cx.notify()).detach();
                    ExportWindow {
                        event_bus,
                        document,
                        timeline_path,
                        state: ExportState::Idle,
                        bitrate_input,
                    }
                });
                let weak_view = view.downgrade();
                window.on_window_should_close(cx, move |_, cx| match weak_view.upgrade() {
                    Some(view) => !matches!(view.read(cx).state, ExportState::Running(_)),
                    None => true,
                });
                view
            },
        )?;
        Ok(())
    }
}

pub struct ExportWindow {
    event_bus: Entity<EventBus>,
    document: TimelineSerialization, // 打开导出窗口时的时间线快照。
    timeline_path: PathBuf,
    pub state: ExportState,
    bitrate_input: Entity<TextInput>,
}

pub enum ExportState {
    Idle,
    Choosing,
    Running(Arc<ExportControl>),
    Complete,
    Stopped,
    Failed,
}

impl ExportWindow {
    fn video_bitrate(&self, cx: &Context<Self>) -> Option<u64> {
        let mbps = self
            .bitrate_input
            .read(cx)
            .text()
            .trim()
            .parse::<f64>()
            .ok()?;
        if !mbps.is_finite() || !(0.1..=1000.0).contains(&mbps) {
            return None;
        }
        Some((mbps * 1_000_000.0).round() as u64)
    }

    fn export(&mut self, cx: &mut Context<Self>) {
        if matches!(self.state, ExportState::Choosing | ExportState::Running(_))
            || self.document.frame_count() == 0
        {
            return;
        }
        let Some(video_bitrate) = self.video_bitrate(cx) else {
            return;
        };
        let Some(directory) = self.timeline_path.parent() else {
            return;
        };
        let suggested_name = self.timeline_path.with_extension("mp4");
        let selection = cx.prompt_for_new_path(
            directory,
            suggested_name.file_name().and_then(|name| name.to_str()),
        );
        self.state = ExportState::Choosing;
        cx.notify();
        cx.spawn(async move |view, cx| {
            let selection_result = selection.await;
            let _ = view.update(cx, |view, cx| {
                let result = (|| -> anyhow::Result<Option<PathBuf>> { Ok(selection_result??) })();
                match result {
                    Ok(Some(path)) => {
                        let control = Arc::new(ExportControl::default());
                        view.state = ExportState::Running(control.clone());
                        let event = AppEvent::ExportTimeline {
                            timeline_path: view.timeline_path.clone(),
                            document: view.document.clone(),
                            output_path: path.with_extension("mp4"),
                            video_bitrate,
                            overwrite: false,
                            export_window: cx.entity().downgrade(),
                            control: control.clone(),
                        };
                        view.event_bus.update(cx, |_, cx| cx.emit(event));
                        cx.spawn(async move |view, cx| {
                            loop {
                                cx.background_executor()
                                    .timer(Duration::from_millis(100))
                                    .await;
                                let running = view.update(cx, |view, cx| {
                                    let running = matches!(&view.state, ExportState::Running(active) if Arc::ptr_eq(active, &control));
                                    if running {
                                        cx.notify();
                                    }
                                    running
                                });
                                match running {
                                    Ok(true) => {}
                                    Ok(false) | Err(_) => break,
                                }
                            }
                        })
                        .detach();
                    }
                    Ok(None) => view.state = ExportState::Idle,
                    Err(error) => {
                        view.state = ExportState::Failed;
                        log::error!("Could not choose export location: {error:?}");
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

impl Render for ExportWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let settings = self.document.editing_state.settings;
        let valid_bitrate = self.video_bitrate(cx).is_some();
        let enabled = valid_bitrate
            && !matches!(self.state, ExportState::Choosing | ExportState::Running(_))
            && self.document.frame_count() > 0;
        let timeline_name = self
            .timeline_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        div()
            .id("export-window")
            .size_full()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_4()
            .p_6()
            .bg(rgb(BACKGROUND))
            .text_color(rgb(TEXT))
            .child(div().text_xl().child("Export timeline"))
            .child(timeline_name.into_owned())
            .child(div().text_sm().text_color(rgb(MUTED)).child(format!(
                "{} × {} · {}/{} fps · MP4 / H.264",
                settings.width, settings.height,
                settings.frame_rate.numerator, settings.frame_rate.denominator,
            )))
            .child(div().flex().items_center().gap_4()
                .child("Video bitrate (Mbps)")
                .child(if matches!(self.state, ExportState::Choosing | ExportState::Running(_)) {
                    div().child(self.bitrate_input.read(cx).text().to_string()).into_any_element()
                } else {
                    div().w(px(140.0)).child(self.bitrate_input.clone()).into_any_element()
                }))
            .when(!valid_bitrate, |view| view.child(
                div().text_sm().text_color(rgb(ERROR)).child("Enter a bitrate between 0.1 and 1000 Mbps."),
            ))
            .child(div().text_sm().text_color(rgb(MUTED)).child(
                "Exports the timeline as it was when this window opened. Existing files are not replaced.",
            ))
            .when(self.document.frame_count() == 0, |view| {
                view.child(div().text_color(rgb(ERROR)).child("This timeline is empty."))
            })
            .child(
                div()
                    .id("export-timeline")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(if enabled { ACCENT } else { MUTED }))
                    .text_color(rgb(BACKGROUND))
                    .child(if matches!(self.state, ExportState::Choosing) { "Choosing location…" } else { "Export MP4…" })
                    .when(enabled, |button| {
                        button.cursor_pointer().on_click(cx.listener(|view, _, _, cx| view.export(cx)))
                    }),
            )
            .child(match &self.state {
                ExportState::Running(control) => {
                    let total = self.document.frame_count();
                    let completed = control.completed_frames();
                    let fraction = (completed as f32 / total.max(1) as f32).clamp(0.0, 1.0);
                    let stopping = control.stop_requested();
                    let label = if stopping {
                        "Stopping…".to_string()
                    } else if completed == total {
                        "Finalizing…".to_string()
                    } else {
                        format!("Exporting: {:.0}% · {completed} / {total} frames", fraction * 100.0)
                    };
                    div().flex().flex_col().gap_2()
                        .child(label)
                        .child(div().h_2().w_full().bg(rgb(MUTED)).child(
                            div().h_full().w(gpui::relative(fraction)).bg(rgb(ACCENT)),
                        ))
                        .child(div().id("stop-export").px_4().py_2().rounded_md()
                            .bg(rgb(MUTED)).child(if stopping { "Stopping…" } else { "Stop export" })
                            .when(!stopping, |button| button.cursor_pointer().on_click(cx.listener(|view, _, _, cx| {
                                if let ExportState::Running(control) = &view.state {
                                    control.request_stop();
                                    cx.notify();
                                }
                            }))))
                }
                ExportState::Complete => div().child("Export complete."),
                ExportState::Stopped => div().child("Export stopped. Partial output removed."),
                ExportState::Failed => div().text_color(rgb(ERROR)).child("Export failed. See the log for details."),
                ExportState::Idle | ExportState::Choosing => div(),
            })
    }
}
