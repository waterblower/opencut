use anyhow::{Context as _, Result};
use gpui::{App, Context, FocusHandle, KeyBinding, RenderImage, Window, actions};
use image::{Frame, RgbaImage};
use opencut_player::video3::{FrameConverter, PixelOrder, VideoBackend};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

mod view;

pub struct Player {
    video_backend: VideoBackend,
    converter: FrameConverter,
    displayed: Option<Arc<RenderImage>>,
    position: Duration,
    title: String,
    focus_handle: FocusHandle,
}

impl Player {
    pub fn new(
        video_backend: VideoBackend,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let player = Self {
            video_backend,
            converter: FrameConverter::new(),
            displayed: None,
            position: Duration::ZERO,
            title: path.display().to_string(),
            focus_handle,
        };

        cx.spawn(async move |player, cx| {
            let bge = cx.background_executor().clone();
            eprintln!("Player task started");
            let started = Instant::now();
            loop {
                let res = player.update(cx, |player, cx| -> Result<Option<Duration>> {
                    let stage_started = Instant::now();
                    let Some(frame) = player.video_backend.video.next_frame()? else {
                        return Ok(None);
                    };
                    let decode_time = stage_started.elapsed();

                    let stage_started = Instant::now();
                    let converted = player.converter.convert(&frame, PixelOrder::Bgra)?;
                    let convert_time = stage_started.elapsed();

                    let stage_started = Instant::now();
                    let pixels =
                        RgbaImage::from_raw(converted.width, converted.height, converted.pixels)
                            .context("invalid prepared image dimensions")?;
                    let pixels_time = stage_started.elapsed();

                    let stage_started = Instant::now();
                    let image = Arc::new(RenderImage::new(vec![Frame::new(pixels)]));
                    let image_time = stage_started.elapsed();
                    eprintln!(
                        "PTS {} µs: next_frame={decode_time:?}, convert={convert_time:?}, RgbaImage={pixels_time:?}, RenderImage={image_time:?}",
                        frame.timestamp.0,
                    );
                    let position = Duration::from_micros(frame.timestamp.0.max(0) as u64);
                    player.displayed = Some(image);
                    player.position = position;
                    cx.notify();

                    let duration = frame
                        .duration
                        .or(player.video_backend.metadata.video.average_frame_interval)
                        .unwrap_or_default();
                    let wait = position
                        .saturating_add(duration)
                        .saturating_sub(started.elapsed());
                    Ok(Some(wait))
                });
                let wait = match res {
                    Ok(Ok(Some(wait))) => wait,
                    Ok(Ok(None)) | Err(_) => break,
                    Ok(Err(error)) => {
                        eprintln!("Player failed: {error:?}");
                        std::process::exit(1);
                    }
                };
                bge.timer(wait).await;
            }
        })
        .detach();
        player
    }
}

actions!(opencut, [TogglePlayback, StepBackward, StepForward]);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("space", TogglePlayback, None),
        KeyBinding::new("left", StepBackward, None),
        KeyBinding::new("right", StepForward, None),
    ]);
}
