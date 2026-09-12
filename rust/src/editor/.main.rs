#![allow(dead_code)]

#[path = "mod.rs"]
mod editor;
#[path = "../gpui_inspector.rs"]
mod gpui_inspector;
#[path = "../macos_pinch.rs"]
mod macos_pinch;
#[path = "../playback_view.rs"]
mod playback_view;

#[path = "../video/mod.rs"]
mod video;

mod asset;
use asset::EditorAssets;

use editor::workspace::{
    GlobalEditorSettings, load_global_editor_settings, save_global_editor_settings,
};
use editor::{AppEvent, Editor, EventBus};
use gpui::{
    App, Bounds, Entity, WindowBounds, WindowHandle, WindowOptions, prelude::*, px, rgb, size,
};
use gpui_platform::application;
use std::io::Write as _;
use std::path::PathBuf;

fn main() {
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

    macos_pinch::install();
    application().with_assets(EditorAssets).run(run_app);
}

fn run_app(cx: &mut App) {
    gpui_component::init(cx);
    gpui_component::Theme::global_mut(cx).caret = rgb(0xffffff).into();
    gpui_inspector::init(cx);
    editor::bind_keys(cx);
    cx.set_quit_mode(gpui::QuitMode::Explicit);
    let mut close_subscription = Some(cx.on_window_closed(quit_after_last_window));
    let event_bus = cx.new(|_| EventBus {});
    let mut window = open_editor_window(
        load_global_editor_settings().project_root,
        event_bus.clone(),
        cx,
    );
    cx.subscribe(&event_bus, move |event_bus, event, cx| {
        let AppEvent::SwitchProject { project_path } = event else {
            return;
        };
        let root = match std::fs::canonicalize(project_path) {
            Ok(root) => root,
            Err(error) => panic!(
                "could not open {}: {error} at {}:{}",
                project_path.display(),
                file!(),
                line!()
            ),
        };
        let ready = window
            .update(cx, |editor, _, cx| editor.prepare_project_switch(cx))
            .unwrap_or(false);
        if !ready {
            return;
        }
        drop(close_subscription.take());
        if let Err(error) = window.update(cx, |_, window, _| window.remove_window()) {
            panic!("could not close editor: {error} at {}:{}", file!(), line!());
        }
        window = open_editor_window(root.clone(), event_bus, cx);
        close_subscription = Some(cx.on_window_closed(quit_after_last_window));
        if let Err(error) =
            save_global_editor_settings(&GlobalEditorSettings { project_root: root })
        {
            panic!(
                "could not save project settings: {error} at {}:{}",
                file!(),
                line!()
            );
        }
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
