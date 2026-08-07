mod editor;
mod gpui_inspector;
mod macos_pinch;
mod playback_view;
#[allow(dead_code)]
#[path = "video_backend/gstreamer.rs"]
mod video_backend;

use editor::Editor;
use gpui::{
    App, Application, AssetSource, Bounds, SharedString, WindowBounds, WindowOptions, prelude::*,
    px, size,
};
use std::borrow::Cow;

struct EditorAssets;

impl AssetSource for EditorAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        let bytes = match path {
            "icons/lock.svg" => include_bytes!("icons/lock.svg").as_slice(),
            "icons/eye.svg" => include_bytes!("icons/eye.svg").as_slice(),
            _ => return Ok(None),
        };
        Ok(Some(Cow::Borrowed(bytes)))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(match path {
            "icons" => vec!["lock.svg".into(), "eye.svg".into()],
            _ => Vec::new(),
        })
    }
}

fn main() {
    env_logger::init();

    Application::new()
        .with_assets(EditorAssets)
        .run(|cx: &mut App| {
            macos_pinch::install();
            gpui_inspector::init(cx);
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
