use anyhow::{Context as _, Result};
use ffmpeg_next::{format, format::stream::Disposition, media::Type};
use gpui::{
    AnyElement, App, Bounds, Context, Entity, FocusHandle, KeyBinding, MouseButton, Render, Task,
    Window, WindowBounds, WindowOptions, actions, div, prelude::*, px, size,
};
use gpui_platform::application;
use player_ui::audio_player::AudioPlayer;
use player_ui::video_player::VideoPlayer;

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
        cx.bind_keys([KeyBinding::new("space", TogglePlayback, None)]);

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
        cx.open_window(options, move |window, cx| {
            let (player, task) = if audio_only {
                match AudioPlayer::new(path) {
                    Ok(player) => {
                        let player = cx.new(move |_| player);
                        let task = player.update(cx, |player, cx| player.start(cx));
                        (Player::Audio(player), task)
                    }
                    Err(error) => {
                        eprintln!("Audio player failed: {error:?}");
                        std::process::exit(1);
                    }
                }
            } else {
                match VideoPlayer::new(path) {
                    Ok(player) => {
                        let player = cx.new(move |_| player);
                        let task = player.update(cx, |player, cx| player.start(cx));
                        (Player::Video(player), task)
                    }
                    Err(error) => {
                        eprintln!("Player failed: {error:?}");
                        std::process::exit(1);
                    }
                }
            };
            let focus_handle = cx.focus_handle();
            focus_handle.focus(window, cx);
            cx.new(|_| PlayerWindow {
                _playback_task: task,
                focus_handle,
                player,
            })
        })
        .expect("failed to create the player window");
        cx.activate(true);
    });
    return Ok(());
}

actions!(opencut, [TogglePlayback]);

enum Player {
    Audio(Entity<AudioPlayer>),
    Video(Entity<VideoPlayer>),
}

/// Window root: owns keyboard focus and key actions; the player views only render and handle clicks.
struct PlayerWindow {
    _playback_task: Task<()>, // 窗口销毁时先取消播放任务，再释放播放器。
    focus_handle: FocusHandle,
    player: Player,
}

impl Render for PlayerWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let player: AnyElement = match &self.player {
            Player::Audio(player) => player.clone().into_any_element(),
            Player::Video(player) => player.clone().into_any_element(),
        };
        div()
            .size_full()
            .track_focus(&self.focus_handle)
            // Clicking the player's controls must not leave the window without focus.
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| this.focus_handle.focus(window, cx)),
            )
            .on_action(
                cx.listener(|this, _: &TogglePlayback, _, cx| match &this.player {
                    Player::Audio(player) => {
                        if let Err(error) =
                            player.update(cx, |player, cx| player.toggle_playback(cx))
                        {
                            eprintln!("Toggling audio playback failed: {error:?}");
                        }
                    }
                    Player::Video(player) => {
                        player.update(cx, |player, cx| player.toggle_playback(cx))
                    }
                }),
            )
            .child(player)
    }
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
