use super::export::{DEFAULT_VIDEO_BIT_RATE, ExportEncoder, ExportOptions, export_project};
use super::*;
use std::{
    env,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::{Duration, Instant},
};

const EXPORT_AUDIO_BIT_RATE: usize = 192_000;
const EXPORT_PROGRESS_SCALE: u32 = 1_000;
const RESOLUTION_PRESETS: [(u32, u32, &str); 6] = [
    (3840, 2160, "3840 × 2160 · 4K UHD"),
    (2560, 1440, "2560 × 1440 · QHD"),
    (1920, 1080, "1920 × 1080 · Full HD"),
    (1280, 720, "1280 × 720 · HD"),
    (1080, 1920, "1080 × 1920 · Vertical"),
    (1080, 1080, "1080 × 1080 · Square"),
];
const EXPORT_FRAME_RATE_PRESETS: [(FrameRate, &str); 8] = [
    (FrameRate::new(24_000, 1_001), "23.976 fps"),
    (FrameRate::new(24, 1), "24 fps"),
    (FrameRate::new(25, 1), "25 fps"),
    (FrameRate::new(30_000, 1_001), "29.97 fps"),
    (FrameRate::new(30, 1), "30 fps"),
    (FrameRate::new(50, 1), "50 fps"),
    (FrameRate::new(60_000, 1_001), "59.94 fps"),
    (FrameRate::new(60, 1), "60 fps"),
];
const EXPORT_ENCODER_PRESETS: [ExportEncoder; 2] =
    [ExportEncoder::Hardware, ExportEncoder::Software];

pub(super) struct ExportDialogState {
    resolution: (u32, u32),
    frame_rate: FrameRate,
    encoder: ExportEncoder,
    resolution_menu_open: bool,
    frame_rate_menu_open: bool,
    encoder_menu_open: bool,
    bitrate: Entity<ExplorerFilter>,
    destination: Entity<ExplorerFilter>,
    status: ExportDialogStatus,
}

enum ExportDialogStatus {
    Idle,
    Exporting(Arc<AtomicU32>),
    Complete { path: PathBuf, elapsed: Duration },
    Failed { message: String, progress: u32 },
}

struct ValidatedExport {
    options: ExportOptions,
    destination: PathBuf,
}

impl Editor {
    pub(super) fn open_export_dialog(&mut self, cx: &mut Context<Self>) {
        if self.project.clips.is_empty() || self.exporting {
            return;
        }

        let return_focus = self.focus_handle.clone();
        let bitrate = cx.new(|cx| {
            ExplorerFilter::new_integer_field(
                "export-bitrate-input",
                (DEFAULT_VIDEO_BIT_RATE / 1_000_000).to_string(),
                "8",
                return_focus.clone(),
                cx,
            )
        });
        let destination = cx.new(|cx| {
            ExplorerFilter::new_field(
                "export-destination-input",
                default_export_destination(&self.project_root, &self.timeline_path)
                    .display()
                    .to_string(),
                "/path/to/export.mp4",
                return_focus,
                cx,
            )
        });

        for input in [&bitrate, &destination] {
            cx.observe(input, |_, _, cx| cx.notify()).detach();
        }

        self.settings_open = false;
        self.file_context_menu = None;
        self.export_dialog_state = Some(ExportDialogState {
            resolution: (self.project.settings.width, self.project.settings.height),
            frame_rate: self.project.settings.frame_rate,
            encoder: ExportEncoder::default_for_platform(),
            resolution_menu_open: false,
            frame_rate_menu_open: false,
            encoder_menu_open: false,
            bitrate,
            destination,
            status: ExportDialogStatus::Idle,
        });
    }

    pub(super) fn export_dialog(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let state = self
            .export_dialog_state
            .as_ref()
            .expect("export dialog rendered without state");
        let project_name = self
            .timeline_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .map(|name| {
                name.strip_suffix(".timeline.json")
                    .unwrap_or(&name)
                    .to_string()
            })
            .unwrap_or_else(|| "OpenCut timeline".to_string());
        let duration = self.project.content_duration();
        let duration_seconds = self.project.seconds(duration);
        let validated = self.validated_export(cx);
        let validation_error = validated.as_ref().err().cloned();
        let video_bit_rate = validated
            .as_ref()
            .map(|validated| validated.options.video_bit_rate)
            .unwrap_or(DEFAULT_VIDEO_BIT_RATE);
        let start_enabled = validated.is_ok() && !self.exporting;
        let idle_summary = validation_error.clone().unwrap_or_else(|| {
            format!(
                "Est. {} · H.264 video · AAC audio",
                format_estimated_size(duration_seconds, video_bit_rate)
            )
        });
        let (progress_fraction, progress_label, footer_message, footer_is_error) =
            match &state.status {
                ExportDialogStatus::Idle => (None, None, idle_summary, validation_error.is_some()),
                ExportDialogStatus::Exporting(progress) => {
                    let progress = load_export_progress(progress);
                    let percentage = (progress * 100.0).round() as u32;
                    (
                        Some(progress),
                        Some("Exporting…".to_string()),
                        format!("Encoding timeline · {percentage}%"),
                        false,
                    )
                }
                ExportDialogStatus::Complete { path, elapsed } => (
                    Some(1.0),
                    Some(format!("Finished in {}", format_export_duration(*elapsed))),
                    format!("Exported {}", path.display()),
                    false,
                ),
                ExportDialogStatus::Failed { message, progress } => (
                    Some(*progress as f32 / EXPORT_PROGRESS_SCALE as f32),
                    Some("Export failed".to_string()),
                    message.clone(),
                    true,
                ),
            };

        div()
            .id("export-dialog-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .occlude()
            .bg(gpui::rgba(0x000000b8))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _, _, cx| {
                    if !editor.exporting {
                        editor.export_dialog_state = None;
                        cx.notify();
                    }
                }),
            )
            .child(
                div()
                    .id("export-dialog")
                    .w(gpui::relative(0.88))
                    .max_w(px(1080.0))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded_xl()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(PANEL))
                    .shadow_lg()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .min_h(px(112.0))
                            .flex()
                            .items_center()
                            .justify_between()
                            .px_8()
                            .py_5()
                            .border_b_1()
                            .border_color(rgb(BORDER))
                            .child(
                                div()
                                    .min_w_0()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div()
                                            .text_2xl()
                                            .font_weight(gpui::FontWeight::BOLD)
                                            .child("Export sequence"),
                                    )
                                    .child(
                                        div()
                                            .font_family("monospace")
                                            .text_sm()
                                            .text_color(rgb(MUTED))
                                            .text_ellipsis()
                                            .child(format!(
                                                "{project_name} · {}",
                                                format_export_timecode(
                                                    duration,
                                                    self.project.settings.frame_rate
                                                )
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .id("close-export-dialog")
                                    .size_9()
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .cursor(if self.exporting {
                                        CursorStyle::Arrow
                                    } else {
                                        CursorStyle::PointingHand
                                    })
                                    .text_2xl()
                                    .text_color(rgb(MUTED))
                                    .when(!self.exporting, |button| {
                                        button.hover(|style| {
                                            style.bg(rgb(SURFACE_HOVER)).text_color(rgb(TEXT))
                                        })
                                    })
                                    .child("×")
                                    .when(!self.exporting, |button| {
                                        button.on_click(cx.listener(|editor, _, _, cx| {
                                            editor.export_dialog_state = None;
                                            cx.notify();
                                        }))
                                    }),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_6()
                            .px_8()
                            .py_7()
                            .child(
                                div()
                                    .grid()
                                    .grid_cols(4)
                                    .gap_4()
                                    .child(self.export_resolution_dropdown(cx))
                                    .child(self.export_frame_rate_dropdown(cx))
                                    .child(self.export_encoder_dropdown(cx))
                                    .child(export_editable_field(
                                        "Bitrate (Mb/s)",
                                        state.bitrate.clone(),
                                    )),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_2()
                                    .child(
                                        div().text_sm().text_color(rgb(MUTED)).child("Destination"),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_3()
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .flex_1()
                                                    .child(state.destination.clone()),
                                            )
                                            .child(
                                                export_dialog_button("Choose…", false, true)
                                                    .on_click(cx.listener(|editor, _, _, cx| {
                                                        editor.choose_export_destination(cx);
                                                    })),
                                            ),
                                    ),
                            ),
                    )
                    .when_some(progress_fraction, |dialog, progress| {
                        dialog.child(export_progress_view(
                            progress,
                            progress_label
                                .clone()
                                .unwrap_or_else(|| "Exporting…".to_string()),
                            footer_is_error,
                        ))
                    })
                    .child(
                        div()
                            .min_h(px(86.0))
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .px_8()
                            .py_4()
                            .border_t_1()
                            .border_color(rgb(BORDER))
                            .child(
                                div()
                                    .min_w_0()
                                    .font_family("monospace")
                                    .text_sm()
                                    .text_color(if footer_is_error {
                                        rgb(ERROR)
                                    } else {
                                        rgb(MUTED)
                                    })
                                    .text_ellipsis()
                                    .child(footer_message),
                            )
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .flex()
                                    .items_center()
                                    .gap_3()
                                    .child(
                                        export_dialog_button(
                                            if matches!(&state.status, ExportDialogStatus::Idle) {
                                                "Cancel"
                                            } else {
                                                "Close"
                                            },
                                            false,
                                            !self.exporting,
                                        )
                                        .when(
                                            !self.exporting,
                                            |button| {
                                                button.on_click(cx.listener(|editor, _, _, cx| {
                                                    editor.export_dialog_state = None;
                                                    cx.notify();
                                                }))
                                            },
                                        ),
                                    )
                                    .child(
                                        export_dialog_button(
                                            if self.exporting {
                                                "Exporting…"
                                            } else {
                                                "Start export"
                                            },
                                            true,
                                            start_enabled,
                                        )
                                        .when(
                                            start_enabled,
                                            |button| {
                                                button.on_click(cx.listener(|editor, _, _, cx| {
                                                    editor.start_export(cx);
                                                }))
                                            },
                                        ),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn validated_export(&self, cx: &App) -> Result<ValidatedExport, String> {
        let state = self
            .export_dialog_state
            .as_ref()
            .ok_or_else(|| "Export dialog is closed.".to_string())?;
        let video_bit_rate = parse_bitrate(state.bitrate.read(cx).query())?;
        let destination =
            parse_destination(state.destination.read(cx).query(), &self.project_root)?;

        Ok(ValidatedExport {
            options: ExportOptions {
                width: state.resolution.0,
                height: state.resolution.1,
                frame_rate: state.frame_rate,
                video_bit_rate,
                encoder: state.encoder,
            },
            destination,
        })
    }

    fn export_resolution_dropdown(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let state = self
            .export_dialog_state
            .as_ref()
            .expect("resolution dropdown rendered without export state");
        let selected = state.resolution;
        let mut presets = RESOLUTION_PRESETS.to_vec();
        if !presets
            .iter()
            .any(|(width, height, _)| (*width, *height) == selected)
        {
            presets.insert(0, (selected.0, selected.1, "Project resolution"));
        }
        let options = presets
            .into_iter()
            .enumerate()
            .map(|(index, (width, height, description))| {
                let active = selected == (width, height);
                export_dropdown_option(
                    ("export-resolution-option", index),
                    format!("{width} × {height}"),
                    description,
                    active,
                )
                .on_click(cx.listener(move |editor, _, _, cx| {
                    if let Some(state) = editor.export_dialog_state.as_mut() {
                        state.resolution = (width, height);
                        state.resolution_menu_open = false;
                    }
                    cx.stop_propagation();
                    cx.notify();
                }))
            })
            .collect::<Vec<_>>();

        export_dropdown_field(
            "Resolution",
            format!("{} × {}", selected.0, selected.1),
            state.resolution_menu_open,
            options,
        )
        .on_click(cx.listener(|editor, _, _, cx| {
            if let Some(state) = editor.export_dialog_state.as_mut() {
                state.resolution_menu_open = !state.resolution_menu_open;
                state.frame_rate_menu_open = false;
                state.encoder_menu_open = false;
            }
            cx.notify();
        }))
        .into_any_element()
    }

    fn export_frame_rate_dropdown(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let state = self
            .export_dialog_state
            .as_ref()
            .expect("frame-rate dropdown rendered without export state");
        let selected = state.frame_rate;
        let options = EXPORT_FRAME_RATE_PRESETS
            .into_iter()
            .enumerate()
            .map(|(index, (frame_rate, label))| {
                let active = selected == frame_rate;
                export_dropdown_option(
                    ("export-frame-rate-option", index),
                    label.to_string(),
                    "",
                    active,
                )
                .on_click(cx.listener(move |editor, _, _, cx| {
                    if let Some(state) = editor.export_dialog_state.as_mut() {
                        state.frame_rate = frame_rate;
                        state.frame_rate_menu_open = false;
                    }
                    cx.stop_propagation();
                    cx.notify();
                }))
            })
            .collect::<Vec<_>>();

        export_dropdown_field(
            "Frame rate",
            format_frame_rate(selected),
            state.frame_rate_menu_open,
            options,
        )
        .on_click(cx.listener(|editor, _, _, cx| {
            if let Some(state) = editor.export_dialog_state.as_mut() {
                state.frame_rate_menu_open = !state.frame_rate_menu_open;
                state.resolution_menu_open = false;
                state.encoder_menu_open = false;
            }
            cx.notify();
        }))
        .into_any_element()
    }

    fn export_encoder_dropdown(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let state = self
            .export_dialog_state
            .as_ref()
            .expect("encoder dropdown rendered without export state");
        let selected = state.encoder;
        let options = EXPORT_ENCODER_PRESETS
            .into_iter()
            .enumerate()
            .map(|(index, encoder)| {
                export_dropdown_option(
                    ("export-encoder-option", index),
                    encoder.label().to_string(),
                    encoder.implementation(),
                    selected == encoder,
                )
                .on_click(cx.listener(move |editor, _, _, cx| {
                    if let Some(state) = editor.export_dialog_state.as_mut() {
                        state.encoder = encoder;
                        state.encoder_menu_open = false;
                    }
                    cx.stop_propagation();
                    cx.notify();
                }))
            })
            .collect::<Vec<_>>();

        export_dropdown_field(
            "Encoder",
            format!("{} · {}", selected.label(), selected.implementation()),
            state.encoder_menu_open,
            options,
        )
        .on_click(cx.listener(|editor, _, _, cx| {
            if let Some(state) = editor.export_dialog_state.as_mut() {
                state.encoder_menu_open = !state.encoder_menu_open;
                state.resolution_menu_open = false;
                state.frame_rate_menu_open = false;
            }
            cx.notify();
        }))
        .into_any_element()
    }

    fn choose_export_destination(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.export_dialog_state.as_ref() else {
            return;
        };
        let current = expand_home(state.destination.read(cx).query());
        let directory = current
            .parent()
            .filter(|path| path.is_dir())
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.project_root.clone());
        let suggested_name = current
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "opencut-export.mp4".to_string());
        let selection = cx.prompt_for_new_path(&directory, Some(&suggested_name));

        cx.spawn(async move |editor, cx| {
            let result = selection.await;
            editor
                .update(cx, |editor, cx| {
                    match result {
                        Ok(Ok(Some(path))) => {
                            if let Some(state) = editor.export_dialog_state.as_ref() {
                                state.destination.update(cx, |input, cx| {
                                    input.set_text(
                                        with_mp4_extension(path).display().to_string(),
                                        cx,
                                    );
                                });
                            }
                        }
                        Ok(Ok(None)) => {}
                        Ok(Err(error)) => {
                            editor.error =
                                Some(format!("Could not choose export destination: {error}"));
                        }
                        Err(error) => {
                            editor.error = Some(format!("Export dialog failed: {error}"));
                        }
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    fn start_export(&mut self, cx: &mut Context<Self>) {
        if self.exporting || self.project.clips.is_empty() {
            return;
        }
        let validated = match self.validated_export(cx) {
            Ok(validated) => validated,
            Err(_) => {
                cx.notify();
                return;
            }
        };

        let path = validated.destination;
        let options = validated.options;
        let project = self.project.clone();
        let project_root = self.project_root.clone();
        let export_path = path.clone();
        let progress = Arc::new(AtomicU32::new(0));
        if let Some(state) = self.export_dialog_state.as_mut() {
            state.resolution_menu_open = false;
            state.frame_rate_menu_open = false;
            state.encoder_menu_open = false;
            state.status = ExportDialogStatus::Exporting(progress.clone());
        }
        self.exporting = true;
        self.status = Some("Exporting…".to_string());
        self.error = None;
        cx.notify();

        let started_at = Instant::now();
        cx.spawn(async move |editor, cx| {
            let encoder_progress = progress.clone();
            let result = cx
                .background_executor()
                .spawn(async move {
                    export_project(&project, &project_root, &export_path, options, |fraction| {
                        store_export_progress(&encoder_progress, fraction)
                    })
                })
                .await;
            let elapsed = started_at.elapsed();
            editor
                .update(cx, |editor, cx| {
                    editor.exporting = false;
                    match result {
                        Ok(()) => {
                            if let Some(state) = editor.export_dialog_state.as_mut() {
                                state.status = ExportDialogStatus::Complete {
                                    path: path.clone(),
                                    elapsed,
                                };
                            }
                            editor.status = Some(format!(
                                "Exported {} in {}",
                                path.display(),
                                format_export_duration(elapsed)
                            ));
                            editor.error = None;
                        }
                        Err(error) => {
                            if let Some(state) = editor.export_dialog_state.as_mut() {
                                state.status = ExportDialogStatus::Failed {
                                    message: error.clone(),
                                    progress: progress.load(Ordering::Relaxed),
                                };
                            }
                            editor.status = None;
                            editor.error = Some(error);
                        }
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }
}

fn export_editable_field(label: &'static str, input: Entity<ExplorerFilter>) -> gpui::Div {
    div()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_sm().text_color(rgb(MUTED)).child(label))
        .child(input)
}

fn export_progress_view(progress: f32, label: String, failed: bool) -> gpui::Div {
    let progress = progress.clamp(0.0, 1.0);
    div()
        .px_8()
        .pb_7()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .text_sm()
                .child(label)
                .child(
                    div()
                        .font_family("monospace")
                        .text_color(if failed { rgb(ERROR) } else { rgb(MUTED) })
                        .child(format!("{:.0}%", progress * 100.0)),
                ),
        )
        .child(
            div()
                .h(px(6.0))
                .w_full()
                .overflow_hidden()
                .rounded_full()
                .bg(rgb(0x29292f))
                .child(
                    div()
                        .h_full()
                        .w(gpui::relative(progress))
                        .rounded_full()
                        .bg(if failed { rgb(ERROR) } else { rgb(ACCENT) }),
                ),
        )
}

fn load_export_progress(progress: &AtomicU32) -> f32 {
    progress.load(Ordering::Relaxed).min(EXPORT_PROGRESS_SCALE) as f32
        / EXPORT_PROGRESS_SCALE as f32
}

fn store_export_progress(progress: &AtomicU32, fraction: f32) {
    let scaled = (fraction.clamp(0.0, 1.0) * EXPORT_PROGRESS_SCALE as f32).round() as u32;
    progress.store(scaled, Ordering::Relaxed);
}

fn export_dropdown_field(
    label: &'static str,
    value: String,
    open: bool,
    options: Vec<gpui::Stateful<gpui::Div>>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(label)
        .relative()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_2()
        .child(div().text_sm().text_color(rgb(MUTED)).child(label))
        .child(
            div()
                .h(px(54.0))
                .flex()
                .items_center()
                .justify_between()
                .px_4()
                .rounded_lg()
                .border_1()
                .border_color(if open { rgb(ACCENT) } else { rgb(BORDER) })
                .bg(rgb(SURFACE))
                .cursor(CursorStyle::PointingHand)
                .font_family("monospace")
                .text_base()
                .hover(|style| style.border_color(rgb(0x4a4a52)))
                .child(value)
                .child(
                    div()
                        .text_color(rgb(MUTED))
                        .child(if open { "▴" } else { "▾" }),
                ),
        )
        .when(open, |this| {
            this.child(
                gpui::deferred(
                    div()
                        .id(gpui::SharedString::from(format!("{label}-menu")))
                        .absolute()
                        .top(px(82.0))
                        .left_0()
                        .w_full()
                        .max_h(px(280.0))
                        .overflow_y_scroll()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .p_1()
                        .rounded_lg()
                        .border_1()
                        .border_color(rgb(BORDER))
                        .bg(rgb(0x111113))
                        .shadow_lg()
                        .occlude()
                        .children(options),
                )
                .with_priority(10),
            )
        })
}

fn export_dropdown_option(
    id: (&'static str, usize),
    value: String,
    description: &'static str,
    active: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .min_h(px(40.0))
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .px_3()
        .rounded_md()
        .cursor(CursorStyle::PointingHand)
        .bg(if active { rgb(0x2a241b) } else { rgb(0x111113) })
        .hover(|style| style.bg(rgb(SURFACE_HOVER)))
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap_2()
                .child(div().font_family("monospace").text_sm().child(value))
                .when(!description.is_empty(), |this| {
                    this.child(div().text_xs().text_color(rgb(MUTED)).child(description))
                }),
        )
        .child(div().size_2().flex_shrink_0().rounded_full().bg(if active {
            rgb(ACCENT)
        } else {
            rgb(0x45454d)
        }))
}

fn export_dialog_button(
    label: &'static str,
    primary: bool,
    enabled: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(label)
        .h(px(52.0))
        .px_6()
        .flex()
        .items_center()
        .justify_center()
        .rounded_lg()
        .border_1()
        .border_color(if primary { rgb(ACCENT) } else { rgb(BORDER) })
        .bg(if primary { rgb(ACCENT) } else { rgb(SURFACE) })
        .cursor(if enabled {
            CursorStyle::PointingHand
        } else {
            CursorStyle::Arrow
        })
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(if enabled {
            if primary { rgb(0x17120a) } else { rgb(TEXT) }
        } else {
            rgb(0x55555d)
        })
        .when(enabled, |button| {
            button.hover(|style| {
                if primary {
                    style.bg(rgb(0xf6c676))
                } else {
                    style.bg(rgb(SURFACE_HOVER))
                }
            })
        })
        .child(label)
}

fn parse_bitrate(value: &str) -> Result<usize, String> {
    let megabits = value
        .trim()
        .parse::<usize>()
        .map_err(|_| "Bitrate must be a whole number in Mb/s.".to_string())?;
    if !(1..=200).contains(&megabits) {
        return Err("Bitrate must be between 1 and 200 Mb/s.".to_string());
    }
    megabits
        .checked_mul(1_000_000)
        .ok_or_else(|| "Bitrate is too large.".to_string())
}

fn parse_destination(value: &str, project_root: &Path) -> Result<PathBuf, String> {
    if value.trim().is_empty() {
        return Err("Choose an export destination.".to_string());
    }
    let mut path = expand_home(value.trim());
    if path.is_relative() {
        path = project_root.join(path);
    }
    path = with_mp4_extension(path);
    let Some(parent) = path.parent() else {
        return Err("Export destination has no parent folder.".to_string());
    };
    if !parent.is_dir() {
        return Err(format!(
            "Destination folder does not exist: {}",
            parent.display()
        ));
    }
    Ok(path)
}

fn default_export_destination(project_root: &Path, timeline_path: &Path) -> PathBuf {
    let stem = timeline_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .and_then(|name| name.strip_suffix(".timeline.json").map(str::to_string))
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "opencut".to_string());
    let candidate = project_root.join(format!("{stem}-export.mp4"));
    if !candidate.exists() {
        return candidate;
    }

    for number in 2.. {
        let candidate = project_root.join(format!("{stem}-export-{number}.mp4"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn expand_home(value: &str) -> PathBuf {
    if value == "~" {
        return env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(value));
    }
    if let Some(relative) = value.strip_prefix("~/")
        && let Some(home) = env::var_os("HOME")
    {
        return PathBuf::from(home).join(relative);
    }
    PathBuf::from(value)
}

fn with_mp4_extension(mut path: PathBuf) -> PathBuf {
    if !path
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"))
    {
        path.set_extension("mp4");
    }
    path
}

fn format_frame_rate(frame_rate: FrameRate) -> String {
    EXPORT_FRAME_RATE_PRESETS
        .into_iter()
        .find_map(|(candidate, label)| (candidate == frame_rate).then_some(label.to_string()))
        .unwrap_or_else(|| format!("{:.3} fps", frame_rate.frames_per_second()))
}

fn format_export_timecode(duration: TimelineTime, frame_rate: FrameRate) -> String {
    let nominal_fps = frame_rate.frames_per_second().round().max(1.0) as i64;
    let total_frames = duration.frames().max(0);
    let frames = total_frames % nominal_fps;
    let total_seconds = total_frames / nominal_fps;
    let seconds = total_seconds % 60;
    let minutes = (total_seconds / 60) % 60;
    let hours = total_seconds / 3600;
    format!("{hours:02}:{minutes:02}:{seconds:02}:{frames:02}")
}

fn format_estimated_size(duration_seconds: f64, video_bit_rate: usize) -> String {
    let bytes = duration_seconds.max(0.0) * (video_bit_rate + EXPORT_AUDIO_BIT_RATE) as f64 / 8.0;
    if bytes >= 1_000_000_000.0 {
        format!("{:.1} GB", bytes / 1_000_000_000.0)
    } else {
        format!("{:.0} MB", bytes / 1_000_000.0)
    }
}

fn format_export_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs_f64().round().max(1.0) as u64;
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    if minutes == 0 {
        format!(
            "{seconds} {}",
            if seconds == 1 { "second" } else { "seconds" }
        )
    } else {
        format!(
            "{minutes} min {seconds} {}",
            if seconds == 1 { "second" } else { "seconds" }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_integer_bitrate() {
        assert_eq!(parse_bitrate("16").unwrap(), 16_000_000);
    }

    #[test]
    fn rejects_invalid_bitrate() {
        assert!(parse_bitrate("8 Mb/s").is_err());
        assert!(parse_bitrate("fast").is_err());
    }

    #[test]
    fn offers_hardware_and_software_encoders_with_distinct_labels() {
        assert_eq!(EXPORT_ENCODER_PRESETS.len(), 2);
        assert_ne!(
            ExportEncoder::Hardware.label(),
            ExportEncoder::Software.label()
        );
        assert_ne!(
            ExportEncoder::Hardware.implementation(),
            ExportEncoder::Software.implementation()
        );
    }

    #[test]
    fn formats_export_duration() {
        assert_eq!(format_export_duration(Duration::from_secs(1)), "1 second");
        assert_eq!(
            format_export_duration(Duration::from_secs(42)),
            "42 seconds"
        );
        assert_eq!(
            format_export_duration(Duration::from_secs(128)),
            "2 min 8 seconds"
        );
    }
}
