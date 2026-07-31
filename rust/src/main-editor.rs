mod editor;
#[allow(dead_code)]
#[path = "video_backend/gstreamer.rs"]
mod video_backend;

use editor::Editor;
use gpui::{App, Application, Bounds, WindowBounds, WindowOptions, prelude::*, px, size};

fn main() {
    env_logger::init();

    Application::new().run(|cx: &mut App| {
        editor::bind_keys(cx);
        cx.on_window_closed(|cx| {
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
