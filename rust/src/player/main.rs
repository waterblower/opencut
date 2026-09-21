#[path = "mod.rs"]
mod player;

use crate::player::Player;
use gpui::{App, Bounds, WindowBounds, WindowOptions, prelude::*, px, size};
use gpui_platform::application;
use opencut_player::video3::VideoBackend;
use std::{path::PathBuf, process::ExitCode};

fn main() -> ExitCode {
    let path = {
        let arguments: Vec<_> = std::env::args_os().skip(1).collect();
        let [path] = arguments.as_slice() else {
            eprintln!("Usage: cargo player-mac <path_to_video>");
            return ExitCode::from(2);
        };
        let path = PathBuf::from(path);
        if !path.is_file() {
            eprintln!(
                "Video file does not exist or is not a file: {}",
                path.display()
            );
            return ExitCode::from(2);
        }
        path
    };
    env_logger::init();
    let video_backend = match VideoBackend::open_video(&path) {
        Ok(backend) => backend,
        Err(error) => {
            eprintln!("Could not open video: {error:?}");
            return ExitCode::FAILURE;
        }
    };

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
            move |window, cx| cx.new(|cx| Player::new(video_backend, path, window, cx)),
        )
        .expect("failed to create the GPUI window");
        cx.activate(true);
    });
    ExitCode::SUCCESS
}
