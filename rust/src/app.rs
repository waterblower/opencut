use crate::player::Player;
use gpui::{App, Application, Bounds, WindowBounds, WindowOptions, prelude::*, px, size};
use std::{env, process};
use url::Url;

pub(crate) fn run(program_name: &'static str) {
    env_logger::init();

    let (initial_media, looping) = match parse_args(program_name) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            print_usage(program_name);
            process::exit(2);
        }
    };

    Application::new().run(move |cx: &mut App| {
        crate::player::bind_keys(cx);

        cx.on_window_closed(|cx| {
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
            move |window, cx| cx.new(|cx| Player::new(initial_media, looping, window, cx)),
        )
        .expect("failed to create the GPUI window");
        cx.activate(true);
    });
}

fn parse_args(program_name: &str) -> Result<(Option<(Url, String)>, bool), String> {
    let mut looping = false;
    let mut media = None;

    for argument in env::args().skip(1) {
        match argument.as_str() {
            "--loop" | "-l" => looping = true,
            "--help" | "-h" => {
                print_usage(program_name);
                process::exit(0);
            }
            _ if media.is_none() => media = Some(argument),
            _ => return Err(format!("Unexpected argument: {argument}")),
        }
    }

    let Some(input) = media else {
        return Ok((None, looping));
    };

    let url = match Url::parse(&input) {
        Ok(url) if matches!(url.scheme(), "file" | "http" | "https") => url,
        Ok(url) => return Err(format!("Unsupported URL scheme: {}", url.scheme())),
        Err(_) => {
            let path = std::fs::canonicalize(&input)
                .map_err(|error| format!("Could not open {input}: {error}"))?;
            Url::from_file_path(&path)
                .map_err(|_| format!("Could not convert {} to a file URL", path.display()))?
        }
    };

    let title = if url.scheme() == "file" {
        url.to_file_path()
            .ok()
            .and_then(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| input.clone())
    } else {
        url.host_str().unwrap_or("Network video").to_string()
    };

    Ok((Some((url, title)), looping))
}

fn print_usage(program_name: &str) {
    eprintln!("Usage: {program_name} [--loop] [path-or-url]");
}
