use anyhow::{Context as _, Result};
use gpui::{App, Context, FocusHandle, KeyBinding, RenderImage, Window, actions};
use image::{Frame, RgbaImage};
use opencut_player::video3::{FrameConverter, PixelOrder, VideoBackend, VideoFrame};
use std::{path::PathBuf, sync::Arc, time::Duration};

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
            loop {
                let res = player.update(cx, |player, cx| -> Result<Option<VideoFrame>> {
                    let Some(frame) = player.video_backend.video.next_frame()? else {
                        cx.notify();
                        return Ok(None);
                    };
                    let converted = player.converter.convert(&frame, PixelOrder::Bgra)?;
                    let pixels =
                        RgbaImage::from_raw(converted.width, converted.height, converted.pixels)
                            .context("invalid prepared image dimensions")?;
                    player.displayed = Some(Arc::new(RenderImage::new(vec![Frame::new(pixels)])));
                    player.position = Duration::from_micros(frame.timestamp.0.max(0) as u64);
                    cx.notify();
                    Ok(Some(frame))
                });
                match res {
                    Ok(Ok(Some(_))) => {}
                    Ok(Ok(None)) | Err(_) => break,
                    Ok(Err(error)) => {
                        eprintln!("Player failed: {error:?}");
                        std::process::exit(1);
                    }
                }
                bge.timer(Duration::from_micros(16_600)).await;
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
