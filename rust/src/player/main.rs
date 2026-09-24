mod audio_output;
mod audio_player;
mod gpu;
mod video_player;
mod video_player_view;

use crate::audio_player::{AudioPlayer, ToggleAudio};
use crate::video_player::VideoPlayer;
use anyhow::{Context as _, Result};
use ffmpeg_next::{format, format::stream::Disposition, media::Type};
use gpui::{App, Bounds, KeyBinding, WindowBounds, WindowOptions, prelude::*, px, size};
use gpui_platform::application;

use std::path::{Path, PathBuf};

fn main() -> Result<()> {
    let path = {
        let arguments: Vec<_> = std::env::args_os().skip(1).collect();
        let [path] = arguments.as_slice() else {
            eprintln!("Usage: cargo player-mac <path_to_media>");
            return Ok(());
        };
        let path = PathBuf::from(path);
        if !path.is_file() {
            eprintln!(
                "Media file does not exist or is not a file: {}",
                path.display()
            );
            return Ok(());
        }
        path
    };
    env_logger::init();
    ffmpeg_next::init().context("initializing FFmpeg").unwrap();
    let audio_only = is_audio_only(&path)?;

    application().run(move |cx: &mut App| {
        if audio_only {
            cx.bind_keys([KeyBinding::new("space", ToggleAudio, None)]);
        } else {
            crate::video_player::bind_keys(cx);
        }

        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1100.0), px(760.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            focus: true,
            ..WindowOptions::default()
        };
        if audio_only {
            cx.open_window(options, move |window, cx| {
                cx.new(|cx| match AudioPlayer::new(path, window, cx) {
                    Ok(player) => player,
                    Err(error) => {
                        eprintln!("Audio player failed: {error:?}");
                        std::process::exit(1);
                    }
                })
            })
            .expect("failed to create the audio player window");
        } else {
            cx.open_window(options, move |window, cx| {
                cx.new(|cx| match VideoPlayer::new(path, window, cx) {
                    Ok(player) => player,
                    Err(error) => {
                        eprintln!("Player failed: {error:?}");
                        std::process::exit(1);
                    }
                })
            })
            .expect("failed to create the video player window");
        }
        cx.activate(true);
    });
    return Ok(());
}

fn is_audio_only(path: &Path) -> Result<bool> {
    let input = format::input(path)?;
    for stream in input.streams() {
        if stream.parameters().medium() == Type::Video
            && !stream.disposition().contains(Disposition::ATTACHED_PIC)
        {
            return Ok(false);
        }
    }
    Ok(input.streams().best(Type::Audio).is_some())
}
