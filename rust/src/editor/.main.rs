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

    application().with_assets(EditorAssets).run(|cx: &mut App| {
        macos_pinch::install();
        gpui_inspector::init(cx);
        editor::bind_keys(cx);
        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1440.0), px(900.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                ..WindowOptions::default()
            },
            |window, cx| cx.new(|cx| Editor::new(window, cx)),
        )
        .expect("failed to create the OpenCut editor window");
        cx.activate(true);
    });
}
