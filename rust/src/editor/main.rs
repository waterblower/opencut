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

use editor::Editor;
use gpui::{
    App, AssetSource, Bounds, SharedString, WindowBounds, WindowOptions, prelude::*, px, size,
};
use gpui_platform::application;
use std::borrow::Cow;

#[derive(Clone, Copy)]
enum IconName {
    Eye,
    Lock,
    Mute,
    Trash,
    Unmute,
}

impl IconName {
    const ALL: [Self; 5] = [Self::Eye, Self::Lock, Self::Mute, Self::Trash, Self::Unmute];

    const fn path(self) -> &'static str {
        match self {
            Self::Eye => "icons/eye.svg",
            Self::Lock => "icons/lock.svg",
            Self::Mute => "icons/mute.svg",
            Self::Trash => "icons/trash.svg",
            Self::Unmute => "icons/unmute.svg",
        }
    }

    const fn bytes(self) -> &'static [u8] {
        match self {
            Self::Eye => include_bytes!("../icons/eye.svg"),
            Self::Lock => include_bytes!("../icons/lock.svg"),
            Self::Mute => include_bytes!("../icons/mute.svg"),
            Self::Trash => include_bytes!("../icons/trash.svg"),
            Self::Unmute => include_bytes!("../icons/unmute.svg"),
        }
    }

    fn from_path(path: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|icon| icon.path() == path)
    }
}

struct EditorAssets;

impl AssetSource for EditorAssets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        let Some(icon) = IconName::from_path(path) else {
            return Ok(None);
        };
        Ok(Some(Cow::Borrowed(icon.bytes())))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(match path {
            "icons" => IconName::ALL
                .into_iter()
                .map(|icon| icon.path().trim_start_matches("icons/").into())
                .collect(),
            _ => Vec::new(),
        })
    }
}

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
