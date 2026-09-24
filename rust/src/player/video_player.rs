#[cfg(target_os = "macos")]
use crate::gpu::GpuResources;
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

pub struct VideoPlayer {
    pub video_backend: VideoBackend,
    scaler: Option<scaling::Context>,
    #[cfg(target_os = "macos")]
    gpu: Option<GpuResources>,
    pub displayed: Option<(DisplayedFrame, Duration)>,
    pub playback_state: PlaybackState,
    play_waker: Option<Waker>,
    pub title: String,
    pub focus_handle: FocusHandle,
}

impl VideoPlayer {
    pub fn new(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Result<Self> {
        let video_backend = VideoBackend::open_video(&path)?;

        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        #[cfg(target_os = "macos")]
        let gpu = GpuResources::new((
            video_backend.metadata.video.width as usize,
            video_backend.metadata.video.height as usize,
        ))?;
        let player = Self {
            video_backend,
            scaler: None,
            #[cfg(target_os = "macos")]
            gpu,
            displayed: None,
            playback_state: PlaybackState::Playing,
            play_waker: None,
            title: path.display().to_string(),
            focus_handle,
        };

        cx.spawn(async move |player, cx| {
            let res = run_playback(player, cx).await;
            if let Err(error) = res {
                eprintln!("Player failed: {error:?}");
                std::process::exit(1);
            }
        })
        .detach();
        Ok(player)
    }

    pub fn set_frame(&mut self, image: DisplayedFrame, position: Duration, cx: &mut Context<Self>) {
        self.displayed = Some((image, position));
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

pub enum PlaybackState {
    Playing,
    Paused,
    Ended,
}

impl VideoPlayer {
    fn set_next_frame(&mut self, cx: &mut Context<Self>) -> Result<Duration> {
        let decode_start = Instant::now();
        let frame = match self.video_backend.video.next_frame()? {
            // A decoded frame is available;
            // prepare and display it below.
            Some(frame) => frame,
            // The decoder is drained. EOF
            None => {
                self.playback_state = PlaybackState::Ended;
                cx.notify();
                return Ok(Duration::ZERO);
            }
        };
        let decode_end = Instant::now();

        #[cfg(target_os = "macos")]
        let surface = match self.gpu.as_mut() {
            Some(gpu) => gpu.convert(&frame)?,
            None => None,
        };

        #[cfg(target_os = "macos")]
        let image = match surface {
            Some(buffer) => DisplayedFrame::Surface(buffer),
            None => convert(&mut self.scaler, &frame)?,
        };

        #[cfg(not(target_os = "macos"))]
        let image = convert(&mut self.scaler, &frame)?;

        let convert_end = Instant::now();
        let path = match &image {
            #[cfg(target_os = "macos")]
            DisplayedFrame::Surface(_) => "GPU-prepared NV12 surface",
            DisplayedFrame::Image(_) => "converted BGRA",
        };

        {
            let decode_time = decode_end.duration_since(decode_start);
            let convert_time = convert_end.duration_since(decode_end);
            eprintln!(
                "PTS {} µs: next_frame={decode_time:?}, prepare={convert_time:?}, path={path}",
                frame.timestamp.0,
            );
        }

        let position = Duration::from_micros(frame.timestamp.0.max(0) as u64);
        self.set_frame(image, position, cx);

        let duration = frame
            .duration
            .or(self.video_backend.metadata.video.average_frame_interval)
            .unwrap_or_default();
        Ok(duration)
    }

    pub fn seek(
        &mut self,
        fraction: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let duration = self.video_backend.metadata.duration;
        let position = duration.mul_f64(f64::from(fraction.clamp(0.0, 1.0)));
        self.video_backend.video.seek(position)?;
        self.set_next_frame(cx)?;
        if matches!(self.playback_state, PlaybackState::Ended) {
            self.playback_state = PlaybackState::Paused;
        }
        self.focus_handle.focus(window, cx);
        cx.notify();
        Ok(())
    }

    pub fn toggle_playback(&mut self, cx: &mut Context<Self>) -> Result<()> {
        self.playback_state = match self.playback_state {
            PlaybackState::Playing => PlaybackState::Paused,
            PlaybackState::Paused => PlaybackState::Playing,
            PlaybackState::Ended => PlaybackState::Playing,
        };
        if matches!(self.playback_state, PlaybackState::Playing)
            && let Some(waker) = self.play_waker.take()
        {
            waker.wake();
        }
        cx.notify();
        Ok(())
    }
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        // Let a paused task observe that its weak entity is no longer available.
        if let Some(waker) = self.play_waker.take() {
            waker.wake();
        }
    }
}

trait WaitUntilPlaying {
    async fn wait_until_playing(&self, cx: &mut AsyncApp) -> Result<()>;
}

impl WaitUntilPlaying for WeakEntity<VideoPlayer> {
    async fn wait_until_playing(&self, cx: &mut AsyncApp) -> Result<()> {
        // One playback task waits here. Check the state and register its waker in
        // one foreground update, releasing entity access before suspending.
        poll_fn(|task_cx| {
            self.update(cx, |player, _| {
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
}

async fn run_playback(player: WeakEntity<VideoPlayer>, cx: &mut AsyncApp) -> Result<()> {
    let bge = cx.background_executor().clone();
    eprintln!("Player task started");
    loop {
        player.wait_until_playing(cx).await?;
        // Return the time to wait, or propagate a playback error.
        let res = player.update(cx, |player, cx| -> Result<Duration> {
            let cycle_start = Instant::now();
            let duration = player.set_next_frame(cx)?;
            if duration.is_zero() {
                return Ok(duration);
            }
            let time_to_wait = frame_wait(duration, cycle_start.elapsed())?;
            Ok(time_to_wait)
        })??;
        // At EOF, the next iteration waits at the play gate until restarted.
        bge.timer(res).await;
    }
}

fn frame_wait(frame_budget: Duration, elapsed: Duration) -> Result<Duration> {
    if frame_budget >= elapsed {
        Ok(frame_budget - elapsed)
    } else {
        bail!("frame deadline missed: deadline={frame_budget:?}, elapsed={elapsed:?}");
    }
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
