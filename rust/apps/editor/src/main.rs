#![allow(dead_code)]

#[path = "mod.rs"]
mod editor;
mod gpui_inspector;
mod macos_pinch;
mod project_picker;

mod asset;
use asset::EditorAssets;

use editor::global_settings::GlobalEditorSettings;
use editor::{Editor, EventBus};
use gpui::{
    App, Bounds, Entity, WindowBounds, WindowHandle, WindowOptions, prelude::*, px, rgb, size,
};
use gpui_platform::application;
use std::io::Write as _;
use std::path::PathBuf;

use crate::editor::event_bus::handle_event;

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("opencut_editor=debug"),
    )
    .format(|buffer, record| {
        writeln!(
            buffer,
            "{}\n\t\t[{}:{}]",
            record.args(),
            record.file().unwrap_or("<unknown>"),
            record.line().unwrap_or(0)
        )
    })
    .init();

    let settings = GlobalEditorSettings::load()?;

    macos_pinch::install();
    application().with_assets(EditorAssets).run(move |cx| {
        run_app(cx, settings.project_root.clone());
    });
    Ok(())
}

fn run_app(cx: &mut App, project_root: PathBuf) {
    gpui_tokio::init(cx);
    gpui_component::init(cx);

    gpui_component::Theme::global_mut(cx).caret = rgb(0xffffff).into();
    gpui_inspector::init(cx);
    editor::bind_keys(cx);
    cx.set_quit_mode(gpui::QuitMode::Explicit);
    if project_root.is_dir() {
        open_project(project_root, cx);
        return;
    }
    if let Err(error) = project_picker::open(cx) {
        log::error!("Could not open project picker: {error:?}");
        cx.quit();
    }
}

fn open_project(project_root: PathBuf, cx: &mut App) {
    let mut close_subscription = Some(cx.on_window_closed(quit_after_last_window));
    let event_bus = cx.new(|_| EventBus {});
    let mut window = open_editor_window(project_root, event_bus.clone(), cx);
    cx.subscribe(&event_bus, move |event_bus, event, cx| {
        handle_event(
            cx,
            &mut window,
            event.clone(),
            event_bus,
            &mut close_subscription,
            quit_after_last_window,
        )
    })
    .detach();
}

fn quit_after_last_window(cx: &mut App, _: gpui::WindowId) {
    if cx.windows().is_empty() {
        cx.quit();
    }
}

fn open_editor_window(
    root: PathBuf,
    event_bus: Entity<EventBus>,
    cx: &mut App,
) -> WindowHandle<Editor> {
    let bounds = Bounds::centered(None, size(px(1440.0), px(900.0)), cx);
    let editor = cx.new(|cx| match Editor::new(root.clone(), event_bus, cx) {
        Ok(editor) => editor,
        Err(error) => panic!(
            "could not open {}: {error:?} at {}:{}",
            root.display(),
            file!(),
            line!()
        ),
    });
    let window = match cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            focus: true,
            ..WindowOptions::default()
        },
        |_, _| editor,
    ) {
        Ok(window) => window,
        Err(error) => panic!(
            "could not create editor window for {}: {error} at {}:{}",
            root.display(),
            file!(),
            line!()
        ),
    };
    cx.activate(true);
    window
}
