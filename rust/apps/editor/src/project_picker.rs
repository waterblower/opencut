use crate::global_settings::GlobalEditorSettings;
use crate::{open_project, quit_after_last_window};
use anyhow::{Result, bail};
use gpui::prelude::*;
use gpui::{
    App, Bounds, Context, IntoElement, PathPromptOptions, Render, Window, WindowBounds,
    WindowOptions, div, px, rgb, size,
};
use std::path::PathBuf;

pub fn open(cx: &mut App) -> Result<()> {
    let bounds = Bounds::centered(None, size(px(480.0), px(260.0)), cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            focus: true,
            ..WindowOptions::default()
        },
        |_, cx| {
            cx.new(|_| ProjectPicker {
                choosing: false,
                error: None,
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
}

impl ProjectPicker {
    fn choose(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
                        open_project(root, cx);
                        let _ = picker_window.update(cx, |_, window, _| window.remove_window());
                    });
                }
                outcome => {
                    let _ = picker.update(cx, |picker, cx| {
                        picker.choosing = false;
                        if let Err(error) = outcome {
                            log::error!("Could not open project folder: {error:?}");
                            picker.error = Some(format!("Could not open project folder: {error}"));
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
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_4()
            .p_6()
            .bg(rgb(0x18181b))
            .text_color(rgb(0xf4f4f5))
            .child(div().text_xl().child("OpenCut"))
            .child("Choose a folder to open as your project.")
            .child(
                div()
                    .id("choose-project")
                    .px_4()
                    .py_2()
                    .rounded_md()
                    .bg(rgb(0x2563eb))
                    .cursor_pointer()
                    .child(if self.choosing {
                        "Choosing folder…"
                    } else {
                        "Choose folder"
                    })
                    .on_click(cx.listener(|picker, _, window, cx| picker.choose(window, cx))),
            )
            .when_some(self.error.clone(), |view, error| {
                view.child(div().text_sm().text_color(rgb(0xfca5a5)).child(error))
            })
    }
}
