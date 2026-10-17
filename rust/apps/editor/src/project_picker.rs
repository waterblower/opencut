use crate::asset::IconName;
use crate::event_bus::EventBus;
use crate::global_settings::GlobalEditorSettings;
use crate::theme;
use crate::{open_project, quit_after_last_window};
use anyhow::{Result, bail};
use gpui::{
    Animation, AnimationExt, App, Bounds, Context, FontWeight, IntoElement, PathPromptOptions,
    Render, Transformation, Window, WindowBounds, WindowOptions, div, img, percentage, px, rgb,
    size, svg,
};
use gpui::{Entity, prelude::*};
use std::path::PathBuf;
use std::time::Duration;

pub fn open(
    cx: &mut App,
    event_bus: Entity<EventBus>,
    missing_project: Option<PathBuf>,
) -> Result<()> {
    for existing_window in cx.windows() {
        if let Some(picker_window) = existing_window.downcast::<ProjectPicker>() {
            picker_window.update(cx, |_, window, _| window.activate_window())?;
            cx.activate(true);
            return Ok(());
        }
    }
    let bounds = Bounds::centered(None, size(px(560.0), px(380.0)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(560.0), px(380.0))),
            focus: true,
            ..WindowOptions::default()
        },
        |_, cx| {
            cx.new(|_| ProjectPicker {
                event_bus,
                choosing: false,
                error: None,
                missing_project,
            })
        },
    )?;
    cx.on_window_closed(quit_after_last_window).detach();
    cx.activate(true);
    Ok(())
}

struct ProjectPicker {
    choosing: bool,
    error: Option<String>,
    missing_project: Option<PathBuf>,
    event_bus: Entity<EventBus>,
}

// Private APIs only
impl ProjectPicker {
    fn ui_callback_choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.choosing {
            return;
        }
        self.choosing = true;
        self.error = None;
        cx.notify();
        let picker_window = window.window_handle();
        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose project folder".into()),
        });
        let event_bus = self.event_bus.clone();
        cx.spawn(async move |picker, cx| {
            let result: Result<Option<PathBuf>> = async {
                let paths = selection.await??;
                let Some(root) = paths.and_then(|paths| paths.into_iter().next()) else {
                    return Ok(None);
                };
                let project_path = std::fs::canonicalize(root)?;
                if !project_path.is_dir() {
                    bail!(
                        "Selected project directory does not exist: {}",
                        project_path.display()
                    );
                }
                let mut settings = GlobalEditorSettings::load()?;
                settings.project_root = project_path.clone();
                settings.save()?;
                Ok(Some(project_path))
            }
            .await;
            if picker.upgrade().is_none() {
                return;
            }
            match result {
                Ok(Some(root)) => {
                    let _ = cx.update(|cx| {
                        if let Err(error) = open_project(cx, root, event_bus) {
                            log::error!("Could not open project folder: {error:?}");
                            let _ = picker.update(cx, |picker, cx| {
                                picker.choosing = false;
                                picker.error = Some(format!("{error:#}"));
                                cx.notify();
                            });
                            return;
                        }
                        let _ = picker_window.update(cx, |_, window, _| window.remove_window());
                    });
                }
                outcome => {
                    let _ = picker.update(cx, |picker, cx| {
                        picker.choosing = false;
                        if let Err(error) = outcome {
                            log::error!("Could not open project folder: {error:?}");
                            picker.error = Some(format!("{error:#}"));
                        }
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }
}

impl Render for ProjectPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let notice = if self.choosing {
            None
        } else {
            match &self.error {
                Some(error) => Some((
                    "Could not open project folder",
                    error.clone(),
                    theme::ERROR,
                    0x5a2a2c,
                    0x1f1213,
                )),
                None => match &self.missing_project {
                    Some(path) => Some((
                        "Last project folder not found",
                        path.display().to_string(),
                        theme::ACCENT,
                        0x4a3a20,
                        0x1c170f,
                    )),
                    None => None,
                },
            }
        };
        let has_notice = notice.is_some();
        let icon_canvas_size = 64.0 * 1024.0 / 896.0; // SVG 画布含透明留白，可见图标保持 64 × 64。
        let icon_margin = (64.0 - icon_canvas_size) / 2.0;
        let button_icon = if self.choosing {
            svg()
                .path(IconName::Spinner.path())
                .size(px(16.0))
                .text_color(rgb(theme::MUTED))
                .with_animation(
                    "choosing-folder-spinner",
                    Animation::new(Duration::from_secs(1)).repeat(),
                    |icon, progress| {
                        icon.with_transformation(Transformation::rotate(percentage(progress)))
                    },
                )
                .into_any_element()
        } else {
            svg()
                .path(IconName::Folder.path())
                .size(px(16.0))
                .text_color(rgb(0x17120a))
                .into_any_element()
        };

        div()
            .size_full()
            .relative()
            .overflow_hidden()
            .flex()
            .flex_col()
            .bg(rgb(theme::BACKGROUND))
            .text_color(rgb(theme::TEXT))
            .child(
                div()
                    .absolute()
                    .top(px(-110.0))
                    .w_full()
                    .flex()
                    .justify_center()
                    .child(img(IconName::PickerGlow.path()).w(px(520.0)).h(px(300.0))),
            )
            .child(
                div()
                    .relative()
                    .flex_1()
                    .w_full()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .child(
                        img(IconName::App.path())
                            .size(px(icon_canvas_size))
                            .m(px(icon_margin))
                            .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .mt(px(20.0))
                            .text_size(px(26.0))
                            .line_height(px(32.0))
                            .font_weight(FontWeight::BOLD)
                            .child("OpenCut"),
                    )
                    .child(
                        div()
                            .mt(px(8.0))
                            .text_size(px(14.0))
                            .line_height(px(20.0))
                            .text_color(rgb(theme::MUTED))
                            .child("Open any folder as a project. Your media stays where it is."),
                    )
                    .when_some(
                        notice,
                        |view, (title, detail, color, border, background)| {
                            view.child(
                                div()
                                    .id("project-picker-notice")
                                    .mt(px(28.0))
                                    .w(px(432.0))
                                    .h(px(56.0))
                                    .flex_shrink_0()
                                    .flex()
                                    .gap(px(8.0))
                                    .px(px(10.0))
                                    .py(px(8.0))
                                    .rounded(px(8.0))
                                    .border_1()
                                    .border_color(rgb(border))
                                    .bg(rgb(background))
                                    .child(
                                        svg()
                                            .path(IconName::Warning.path())
                                            .mt(px(2.0))
                                            .size(px(16.0))
                                            .flex_shrink_0()
                                            .text_color(rgb(color)),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .child(
                                                div()
                                                    .text_size(px(13.0))
                                                    .line_height(px(20.0))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(rgb(color))
                                                    .child(title),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.0))
                                                    .line_height(px(18.0))
                                                    .font_family("monospace")
                                                    .text_color(rgb(0xb4b4bc))
                                                    .truncate()
                                                    .child(detail.clone()),
                                            ),
                                    )
                                    .tooltip(move |_, cx| {
                                        cx.new(|_| PickerDetailTooltip {
                                            text: detail.clone(),
                                        })
                                        .into()
                                    }),
                            )
                        },
                    )
                    .child(
                        div()
                            .id("choose-project")
                            .mt(px(if has_notice { 20.0 } else { 28.0 }))
                            .h(px(40.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .gap(px(12.0))
                            .px(px(12.0))
                            .rounded(px(8.0))
                            .border_1()
                            .text_size(px(15.0))
                            .line_height(px(20.0))
                            .font_weight(FontWeight::SEMIBOLD)
                            .when(self.choosing, |button| {
                                button
                                    .bg(rgb(theme::SURFACE))
                                    .border_color(rgb(theme::BORDER))
                                    .text_color(rgb(theme::MUTED))
                            })
                            .when(!self.choosing, |button| {
                                button
                                    .bg(rgb(theme::ACCENT))
                                    .border_color(rgb(theme::ACCENT))
                                    .text_color(rgb(0x17120a))
                                    .cursor_pointer()
                                    .hover(|style| style.bg(rgb(0xffca7a)))
                                    .on_click(cx.listener(|picker, _, window, cx| {
                                        picker.ui_callback_choose(window, cx)
                                    }))
                            })
                            .child(button_icon)
                            .child(if self.choosing {
                                "Choosing folder…"
                            } else {
                                "Open Folder…"
                            }),
                    ),
            )
    }
}

struct PickerDetailTooltip {
    text: String,
}

impl Render for PickerDetailTooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .max_w(px(432.0))
            .p(px(8.0))
            .rounded(px(6.0))
            .border_1()
            .border_color(rgb(theme::BORDER))
            .bg(rgb(theme::SURFACE))
            .text_color(rgb(theme::TEXT))
            .text_size(px(12.0))
            .child(self.text.clone())
    }
}
