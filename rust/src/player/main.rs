#[path = "../playback_view.rs"]
mod playback_view;
#[path = "mod.rs"]
mod player;
#[path = "../video/mod.rs"]
mod video;

use crate::player::Player;
use gpui::{App, Bounds, WindowBounds, WindowOptions, prelude::*, px, size};
use gpui_platform::application;

fn main() {
    env_logger::init();

    application().run(move |cx: &mut App| {
        crate::player::bind_keys(cx);

        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1100.0), px(760.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                ..WindowOptions::default()
            },
            move |window, cx| cx.new(|cx| Player::new(window, cx)),
        )
        .expect("failed to create the GPUI window");
        cx.activate(true);
    });
}
