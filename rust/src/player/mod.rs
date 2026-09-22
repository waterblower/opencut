#[cfg(target_os = "macos")]
use crate::player::gpu::GpuResources;
use anyhow::{Context as _, Result, bail};
#[cfg(target_os = "macos")]
use core_video::pixel_buffer::CVPixelBuffer;
use ffmpeg_next::{
    Error as FfmpegError, ffi, format::Pixel, frame::Video, software::scaling, util::color,
};
use gpui::{
    App, AsyncApp, Context, FocusHandle, KeyBinding, RenderImage, WeakEntity, Window, actions,
};
use image::{Frame, RgbaImage};
use opencut_player::video3::{VideoBackend, VideoFrame};
use std::{
    future::poll_fn,
    path::PathBuf,
    sync::Arc,
    task::{Poll, Waker},
    time::{Duration, Instant},
};

#[cfg(target_os = "macos")]
mod gpu;
mod view;

pub struct Player {
    video_backend: VideoBackend,
    scaler: Option<scaling::Context>,
    displayed: Option<DisplayedFrame>,
    position: Duration,
    playback_state: PlaybackState,
    paused_duration: Duration,
    play_waker: Option<Waker>,
    title: String,
    focus_handle: FocusHandle,
}

impl Player {
    pub fn new(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Result<Self> {
        let video_backend = VideoBackend::open_video(&path)?;

        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        #[cfg(target_os = "macos")]
        let dimensions = (
            video_backend.metadata.video.width as usize,
            video_backend.metadata.video.height as usize,
        );
        let player = Self {
            video_backend,
            scaler: None,
            displayed: None,
            position: Duration::ZERO,
            playback_state: PlaybackState::Playing,
            paused_duration: Duration::ZERO,
            play_waker: None,
            title: path.display().to_string(),
            focus_handle,
        };

        cx.spawn(async move |player, cx| {
            let res = run_playback(
                player,
                cx,
                #[cfg(target_os = "macos")]
                dimensions,
            )
            .await;
            if let Err(error) = res {
                eprintln!("Player failed: {error:?}");
                std::process::exit(1);
            }
        })
        .detach();
        Ok(player)
    }

    pub fn set_frame(&mut self, image: DisplayedFrame, position: Duration, cx: &mut Context<Self>) {
        self.displayed = Some(image);
        self.position = position;
        cx.notify();
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

pub enum DisplayedFrame {
    #[cfg(target_os = "macos")]
    Surface(CVPixelBuffer),
    Image(Arc<RenderImage>),
}

enum PlaybackState {
    Playing,
    Paused(Instant),
    Ended,
}

impl Player {
    fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        self.playback_state = match self.playback_state {
            PlaybackState::Playing => PlaybackState::Paused(Instant::now()),
            PlaybackState::Paused(started) => {
                self.paused_duration += started.elapsed();
                PlaybackState::Playing
            }
            PlaybackState::Ended => return,
        };
        if matches!(self.playback_state, PlaybackState::Playing)
            && let Some(waker) = self.play_waker.take()
        {
            waker.wake();
        }
        cx.notify();
    }
}

impl Drop for Player {
    fn drop(&mut self) {
        // Let a paused task observe that its weak entity is no longer available.
        if let Some(waker) = self.play_waker.take() {
            waker.wake();
        }
    }
}

async fn wait_until_playing(player: &WeakEntity<Player>, cx: &mut AsyncApp) -> Result<()> {
    // One playback task waits here. Check the state and register its waker in
    // one foreground update, releasing entity access before suspending.
    poll_fn(|task_cx| {
        player.update(cx, |player, _| {
            if matches!(player.playback_state, PlaybackState::Playing) {
                player.play_waker = None;
                Poll::Ready(Ok(()))
            } else {
                player.play_waker = Some(task_cx.waker().clone());
                Poll::Pending
            }
        })?
    })
    .await
}

async fn run_playback(
    player: WeakEntity<Player>,
    cx: &mut AsyncApp,
    #[cfg(target_os = "macos")] dimensions: (usize, usize),
) -> Result<()> {
    let bge = cx.background_executor().clone();
    eprintln!("Player task started");
    #[cfg(target_os = "macos")]
    let mut gpu = GpuResources::new(dimensions)?;
    let started = Instant::now();
    let mut next_frame_at = Duration::ZERO;
    loop {
        wait_until_playing(&player, cx).await?;
        // Callback returns Some(wait) to continue, None at EOF, or Err on failure.
        // A zero wait means continue immediately, so EOF needs a separate value.
        let res = player.update(cx, |player, cx| -> Result<Option<Duration>> {
            // A pause may overlap the previous timer. Preserve the remaining
            // frame interval on resume instead of decoding the next frame early.
            let elapsed = started.elapsed().saturating_sub(player.paused_duration);
            let wait = next_frame_at.saturating_sub(elapsed);
            if !wait.is_zero() {
                return Ok(Some(wait));
            }
            let stage_started = Instant::now();
            let Some(frame) = player.video_backend.video.next_frame()? else {
                player.playback_state = PlaybackState::Ended;
                cx.notify();
                return Ok(None);
            };
            let decode_time = stage_started.elapsed();

            let stage_started = Instant::now();

            #[cfg(target_os = "macos")]
            let surface = match gpu.as_mut() {
                Some(gpu) => gpu.convert(&frame)?,
                None => None,
            };

            #[cfg(target_os = "macos")]
            let image = match surface {
                Some(buffer) => DisplayedFrame::Surface(buffer),
                None => convert(&mut player.scaler, &frame)?,
            };

            #[cfg(not(target_os = "macos"))]
            let image = convert(&mut player.scaler, &frame)?;

            let convert_time = stage_started.elapsed();
            let path = match &image {
                #[cfg(target_os = "macos")]
                DisplayedFrame::Surface(_) => "GPU-prepared NV12 surface",
                DisplayedFrame::Image(_) => "converted BGRA",
            };

            eprintln!(
                "PTS {} µs: next_frame={decode_time:?}, prepare={convert_time:?}, path={path}",
                frame.timestamp.0,
            );

            let position = Duration::from_micros(frame.timestamp.0.max(0) as u64);

            player.set_frame(image, position, cx);

            let duration = frame
                .duration
                .or(player.video_backend.metadata.video.average_frame_interval)
                .unwrap_or_default();
            next_frame_at = position.saturating_add(duration);
            let elapsed = started.elapsed().saturating_sub(player.paused_duration);
            let wait = next_frame_at.saturating_sub(elapsed);
            Ok(Some(wait))
        });
        let wait = match res {
            // Wait for the next frame deadline (also rechecked after resuming).
            Ok(Ok(Some(wait))) => wait,
            // Decoder reached EOF
            Ok(Ok(None)) => break,
            // Could not access the player entity (e.g. it was dropped).
            Err(error) => {
                return Err(error);
            }
            // Decode/conversion failed.
            Ok(Err(error)) => {
                return Err(error);
            }
        };
        bge.timer(wait).await;
    }
    return Ok(());
}

/// CPU fallback for frames unsupported by the native surface path.
fn convert(scaler: &mut Option<scaling::Context>, frame: &VideoFrame) -> Result<DisplayedFrame> {
    let matrix = match frame.color_space {
        color::Space::BT709 => ffi::SWS_CS_ITU709,
        color::Space::BT2020NCL | color::Space::BT2020CL => ffi::SWS_CS_BT2020,
        color::Space::FCC => ffi::SWS_CS_FCC,
        color::Space::SMPTE240M => ffi::SWS_CS_SMPTE240M,
        color::Space::BT470BG | color::Space::SMPTE170M => ffi::SWS_CS_ITU601,
        _ if frame.native.height() >= 720 => ffi::SWS_CS_ITU709,
        _ => ffi::SWS_CS_ITU601,
    };
    let mut transferred = Video::empty();
    // SAFETY: frame owns its AVFrame. A non-null hw_frames_ctx requires a
    // hardware transfer; the newly allocated destination is exclusively owned.
    let hardware = unsafe { !(*frame.native.as_ptr()).hw_frames_ctx.is_null() };
    let source = if hardware {
        let result = unsafe {
            ffi::av_hwframe_transfer_data(transferred.as_mut_ptr(), frame.native.as_ptr(), 0)
        };
        if result < 0 {
            return Err(FfmpegError::from(result)).context("transferring hardware video frame");
        }
        &transferred
    } else {
        &frame.native
    };
    let definition = scaling::context::Definition {
        format: source.format(),
        width: source.width(),
        height: source.height(),
    };
    let destination = Pixel::BGRA;
    let reconfigure = match scaler.as_ref() {
        Some(scaler) => *scaler.input() != definition || scaler.output().format != destination,
        None => true,
    };
    if reconfigure {
        *scaler = Some(
            scaling::Context::get(
                source.format(),
                source.width(),
                source.height(),
                destination,
                source.width(),
                source.height(),
                scaling::Flags::BILINEAR,
            )
            .context("creating BGRA scaler")?,
        );
    }
    let scaler = scaler.as_mut().context("missing scaler")?;
    // SAFETY: coefficients have static lifetime; scaler is exclusively owned.
    let result = unsafe {
        let coefficients = ffi::sws_getCoefficients(matrix as i32);
        ffi::sws_setColorspaceDetails(
            scaler.as_mut_ptr(),
            coefficients,
            i32::from(frame.color_range == color::Range::JPEG),
            coefficients,
            1,
            0,
            1 << 16,
            1 << 16,
        )
    };
    if result < 0 {
        return Err(FfmpegError::from(result)).context("configuring video color conversion");
    }
    let mut bgra = Video::empty();
    scaler
        .run(source, &mut bgra)
        .context("converting selected video frame")?;
    let width = bgra.width();
    let height = bgra.height();
    let row_bytes = width as usize * 4;
    let mut pixels = vec![
        0;
        row_bytes
            .checked_mul(height as usize)
            .context("video image is too large")?
    ];
    for (row, output) in pixels.chunks_exact_mut(row_bytes).enumerate() {
        let offset = row * bgra.stride(0);
        output.copy_from_slice(&bgra.data(0)[offset..offset + row_bytes]);
    }
    let angle = frame.rotation_degrees.rem_euclid(360.0);
    let quarter = (angle / 90.0).round() as u32 % 4;
    if (angle - (angle / 90.0).round() * 90.0).abs() > 0.1 {
        bail!(
            "unsupported display rotation: {} degrees",
            frame.rotation_degrees
        );
    }
    let (pixels, width, height) = rotate(pixels, width, height, quarter);
    let pixels =
        RgbaImage::from_raw(width, height, pixels).context("invalid prepared image dimensions")?;
    Ok(DisplayedFrame::Image(Arc::new(RenderImage::new(vec![
        Frame::new(pixels),
    ]))))
}

fn rotate(pixels: Vec<u8>, width: u32, height: u32, quarter: u32) -> (Vec<u8>, u32, u32) {
    if quarter == 0 {
        return (pixels, width, height);
    }
    let (out_width, out_height) = if quarter % 2 == 1 {
        (height, width)
    } else {
        (width, height)
    };
    let mut output = vec![0; pixels.len()];
    for y in 0..height {
        for x in 0..width {
            let (out_x, out_y) = match quarter {
                1 => (y, width - 1 - x),
                2 => (width - 1 - x, height - 1 - y),
                _ => (height - 1 - y, x),
            };
            let from = ((y * width + x) * 4) as usize;
            let to = ((out_y * out_width + out_x) * 4) as usize;
            output[to..to + 4].copy_from_slice(&pixels[from..from + 4]);
        }
    }
    (output, out_width, out_height)
}
