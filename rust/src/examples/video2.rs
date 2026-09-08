//! cargo run --no-default-features --features ffmpeg-video,ui --bin video2 -- <file>
//! Starts paused. Space toggles playback, arrows seek five seconds, M toggles mute.

use std::{path::PathBuf, process::ExitCode, time::Duration};

use anyhow::{Context as _, Result};
use gpui::{
    App, Bounds, Context, FocusHandle, Render, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};
use opencut_player::video2::{VideoBackend, video};

fn main() -> ExitCode {
    env_logger::init();
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let path = PathBuf::from(std::env::args_os().nth(1).context(format!(
        "Usage: video2 <file> at {}:{}",
        file!(),
        line!()
    ))?);
    // Blocking initialization happens before GPUI starts processing events.
    let backend = VideoBackend::open_sync(&path)?;
    gpui_platform::application().run(move |cx: &mut App| {
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(960.0), px(640.0)), cx);
        if let Err(error) = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                focus: true,
                ..Default::default()
            },
            move |window, cx| {
                cx.new(|cx| {
                    let focus = cx.focus_handle();
                    focus.focus(window, cx);
                    Demo { backend, focus }
                })
            },
        ) {
            log::error!(
                "Opening video demo window at {}:{}: {error:#}",
                file!(),
                line!()
            );
            cx.quit();
        }
        cx.activate(true);
    });
    Ok(())
}

struct Demo {
    backend: VideoBackend,
    focus: FocusHandle,
}

impl Render for Demo {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let picture = match video(&self.backend) {
            Ok(element) => element
                .id("demo-video")
                .size(viewport.width, (viewport.height - px(40.0)).max(px(0.0)))
                .into_any_element(),
            Err(error) => div().child(format!("{error:#}")).into_any_element(),
        };
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x000000))
            .text_color(rgb(0xffffff))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|demo, event: &gpui::KeyDownEvent, _, cx| {
                let result = match event.keystroke.key.as_str() {
                    "space" => demo.backend.set_paused(!demo.backend.paused()),
                    "m" => demo.backend.set_muted(!demo.backend.muted()),
                    // The demo deliberately exercises the blocking seek API.
                    // A migrated production player can use the awaited variant.
                    "left" => demo.backend.seek_sync(
                        demo.backend
                            .position()
                            .saturating_sub(Duration::from_secs(5)),
                    ),
                    "right" => demo.backend.seek_sync(
                        demo.backend
                            .position()
                            .saturating_add(Duration::from_secs(5)),
                    ),
                    _ => return,
                };
                if let Err(error) = result {
                    log::error!(
                        "Video demo control failed at {}:{}: {error:#}",
                        file!(),
                        line!()
                    );
                }
                cx.notify();
            }))
            .child(picture)
            .child(div().h(px(40.0)).px_3().child(format!(
                "{}  {:.2} / {:.2}s  ·  Space: play/pause  ←/→: seek  M: mute",
                if self.backend.paused() {
                    "Paused"
                } else {
                    "Playing"
                },
                self.backend.position().as_secs_f64(),
                self.backend.duration().as_secs_f64()
            )))
    }
}
