use crate::editor::Editor;
use crate::event_bus::{AppEvent, EventBus};
use crate::theme::{ACCENT, BACKGROUND, BORDER, ERROR, MUTED, SURFACE, TEXT};
use crate::track::TrackKind;
use crate::ui_components::TextInput;
use anyhow::{Result, ensure};
use engine::export::ExportControl;
use gpui::prelude::*;
use gpui::{
    Bounds, Context, Entity, FontWeight, Window, WindowBounds, WindowOptions, div, px, rgb, size,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use timeline::TimelineSerialization;
use ulid::Ulid;

impl Editor {
    pub(crate) fn open_export_window(
        &self,
        timeline_path: PathBuf,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let Some(timeline) = self.timeline.as_ref() else {
            return Ok(());
        };
        ensure!(
            timeline.path == timeline_path,
            "The active timeline changed before opening export"
        );
        let document = timeline.to_serialize();
        let subtitle_tracks = document
            .editing_state
            .tracks
            .iter()
            .filter(|track| track.kind == TrackKind::Text)
            .map(|track| SubtitleTrack {
                track_id: track.id,
                name: track.name.clone(),
                checked: false,
            })
            .collect::<Vec<_>>();
        let event_bus = self.event_bus.clone();
        let bounds = Bounds::centered(None, size(px(560.0), px(756.0)), cx);
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
                        TextInput::new_field("export-bitrate", "2".into(), "Mbps", return_focus, cx)
                    });
                    cx.observe(&bitrate_input, |_, _, cx| cx.notify()).detach();
                    ExportWindow {
                        event_bus,
                        document,
                        timeline_path,
                        state: ExportState::Idle,
                        bitrate_input,
                        subtitle_tracks,
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

pub(crate) struct ExportWindow {
    event_bus: Entity<EventBus>,
    document: TimelineSerialization, // 打开导出窗口时的时间线快照。
    timeline_path: PathBuf,
    pub(crate) state: ExportState,
    bitrate_input: Entity<TextInput>,
    subtitle_tracks: Vec<SubtitleTrack>,
}

struct SubtitleTrack {
    track_id: Ulid,
    name: String,
    checked: bool, // 勾选的文字轨道各导出一个 .srt 文件。
}

pub(crate) enum ExportState {
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
        let subtitle_track_ids = self
            .subtitle_tracks
            .iter()
            .filter(|track| track.checked)
            .map(|track| track.track_id)
            .collect::<Vec<_>>();
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
                let result = (|| -> anyhow::Result<Option<PathBuf>> { selection_result? })();
                match result {
                    Ok(Some(path)) => {
                        let control = Arc::new(ExportControl::default());
                        view.state = ExportState::Running(control.clone());
                        let event = AppEvent::ExportTimeline {
                            timeline_path: view.timeline_path.clone(),
                            document: view.document.clone(),
                            output_path: path.with_extension("mp4"),
                            video_bitrate,
                            overwrite: true,
                            subtitle_track_ids,
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

// Private APIs only
impl ExportWindow {
    fn subtitle_tracks_view(&self, locked: bool, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_2()
            .when(locked, |view| view.opacity(0.5))
            .child("Export subtitles")
            .when(self.subtitle_tracks.is_empty(), |view| {
                view.child(
                    div()
                        .text_sm()
                        .text_color(rgb(MUTED))
                        .child("No text tracks in this timeline."),
                )
            })
            .children(
                self.subtitle_tracks
                    .iter()
                    .enumerate()
                    .map(|(index, track)| {
                        div()
                            .id(("export-subtitle-track", index))
                            .flex()
                            .items_center()
                            .gap_3()
                            .when(!locked, |row| {
                                row.cursor_pointer()
                                    .on_click(cx.listener(move |view, _, _, cx| {
                                        view.subtitle_tracks[index].checked =
                                            !view.subtitle_tracks[index].checked;
                                        cx.notify();
                                    }))
                            })
                            .child(
                                div()
                                    .size(px(16.0))
                                    .flex_shrink_0()
                                    .rounded(px(4.0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(BACKGROUND))
                                    .when(track.checked, |checkbox| {
                                        checkbox.bg(rgb(ACCENT)).child("✓")
                                    })
                                    .when(!track.checked, |checkbox| {
                                        checkbox
                                            .bg(rgb(SURFACE))
                                            .border_1()
                                            .border_color(rgb(BORDER))
                                    }),
                            )
                            .child(
                                div()
                                    .size(px(16.0))
                                    .flex_shrink_0()
                                    .rounded(px(4.0))
                                    .bg(rgb(0x7a5fb0))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .text_size(px(10.0))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(rgb(0xe3d4ff))
                                    .child("T"),
                            )
                            .child(track.name.clone())
                    }),
            )
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
            .child(div().h(px(20.0)).text_sm().text_color(rgb(ERROR)).when(!valid_bitrate, |view| {
                view.child("Enter a bitrate between 0.1 and 1000 Mbps.")
            }))
            .child(self.subtitle_tracks_view(
                matches!(self.state, ExportState::Choosing | ExportState::Running(_)),
                cx,
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
