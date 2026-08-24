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

use editor::Editor;
use gpui::{App, Bounds, WindowBounds, WindowOptions, prelude::*, px, size};
use gpui_platform::application;

fn main() {
    env_logger::init();
    macos_pinch::install();
    application().with_assets(EditorAssets).run(run_app);
}

fn run_app(cx: &mut App) {
    gpui_inspector::init(cx);
    editor::bind_keys(cx);
    cx.on_window_closed(|cx, _window_id| {
        if cx.windows().is_empty() {
            cx.quit();
        }
    })
    .detach();

    let bounds = Bounds::centered(None, size(px(1440.0), px(900.0)), cx);
    let editor = cx.new(Editor::new);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            focus: true,
            ..WindowOptions::default()
        },
        |_window, _cx| editor,
    )
    .expect("failed to create the OpenCut editor window");
    cx.activate(true);
}
