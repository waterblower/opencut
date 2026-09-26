use crate::audio_output::AudioOutput;
#[cfg(target_os = "macos")]
use crate::gpu::GpuResources;
use anyhow::{Context as _, Result, bail};
#[cfg(target_os = "macos")]
use core_video::pixel_buffer::CVPixelBuffer;
use ffmpeg_next::{
    Error as FfmpegError, ffi, format::Pixel, frame::Video, software::scaling, util::color,
};
use futures::{FutureExt, select, try_join};
use gpui::{
    App, AsyncApp, Context, FocusHandle, KeyBinding, RenderImage, WeakEntity, Window, actions,
};
use image::{Frame, RgbaImage};
use opencut_player::video3::{VideoBackend, VideoFrame};
use std::{
    cell::Cell,
    future::poll_fn,
    path::PathBuf,
    sync::Arc,
    task::{Poll, Waker},
    time::{Duration, Instant},
};

#[rustfmt::skip]
pub struct VideoPlayer {
    video_backend: VideoBackend,
    audio_output: AudioOutput,                                   // 已打开的音频设备；有设备不代表正在播放。
    scaler: Option<scaling::Context>,
    #[cfg(target_os = "macos")]
    gpu: Option<GpuResources>,
    play_wakers: [Option<Waker>; 2],                              // 分别唤醒视频、音频循环；只保存等待者，不保存播放进度。
    pub displayed: Option<(DisplayedFrame, Duration, Duration)>, // (图像, 帧 PTS, 该帧时长)；None：尚未呈现首帧。
    pub playback_state: PlaybackState,
    pub title: String,
    pub focus_handle: FocusHandle,
}

impl VideoPlayer {
    pub fn new(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Result<Self> {
        // 同步打开和配置；性能成本直接体现在调用处，不交给后台 worker。
        let mut backend = VideoBackend::open(&path)?;
        let audio_output = AudioOutput::open()?;
        backend.audio.configure_output(&audio_output.format)?;
        #[cfg(target_os = "macos")]
        let gpu = GpuResources::new((
            backend.metadata.video.width as usize,
            backend.metadata.video.height as usize,
        ))?;
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let player = Self {
            video_backend: backend,
            audio_output,
            scaler: None,
            #[cfg(target_os = "macos")]
            gpu,
            play_wakers: [None, None],
            displayed: None,
            playback_state: PlaybackState::Playing,
            title: path.display().to_string(),
            focus_handle,
        };
        cx.spawn(async move |player, cx| {
            if let Err(error) = run_player(player, cx).await {
                eprintln!("Player failed: {error:?}");
                std::process::exit(1);
            }
        })
        .detach();
        Ok(player)
    }

    pub fn duration(&self) -> Duration {
        self.video_backend.metadata.duration
    }

    pub fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        self.playback_state = match self.playback_state {
            PlaybackState::Playing => PlaybackState::Paused,
            PlaybackState::Paused | PlaybackState::Ended => PlaybackState::Playing,
        };
        if matches!(self.playback_state, PlaybackState::Playing) {
            for waker in &mut self.play_wakers {
                if let Some(waker) = waker.take() {
                    waker.wake();
                }
            }
        }
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
    Playing, // 两个循环继续推进，共用同一个媒体时间基准。
    Paused,  // 用户暂停、时钟冻结；seek 直接更新画面。
    Ended,   // 音频尾部和最后一帧均已播完；再次播放会从头开始。
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        for waker in &mut self.play_wakers {
            if let Some(waker) = waker.take() {
                waker.wake(); // 让两个等待中的循环发现 WeakEntity 已失效并退出。
            }
        }
    }
}

#[rustfmt::skip]
#[derive(Clone, Copy)]
struct PlaybackClock {
    start_position_of_video: Duration,     // 共同起点；不逐轮累加播放位置。
    start_time_of_system: Option<Instant>, // None 表示冻结；播放时间由起点加实际经过时间计算。
}

impl PlaybackClock {
    fn position(&self) -> Duration {
        match self.start_time_of_system {
            Some(start) => self.start_position_of_video + start.elapsed(),
            None => self.start_position_of_video,
        }
    }
}

impl VideoPlayer {
    pub fn seek(&mut self, position: Duration) -> Result<()> {
        self.audio_output.clear_at(position)?;
        self.video_backend.video.seek(position)?;
        self.video_backend.audio.seek(position)?;
        if let Some(frame) = self.prepare_next_frame()? {
            self.displayed = Some(frame);
        }
        Ok(())
    }

    fn prepare_next_frame(&mut self) -> Result<Option<(DisplayedFrame, Duration, Duration)>> {
        let started = Instant::now();
        let Some(frame) = self.video_backend.video.next_frame()? else {
            return Ok(None);
        };
        let frame_position = Duration::from_micros(frame.timestamp.0.max(0) as u64);
        #[cfg(target_os = "macos")]
        let image = match &mut self.gpu {
            Some(gpu) => match gpu.convert(&frame)? {
                Some(surface) => DisplayedFrame::Surface(surface),
                None => convert(&mut self.scaler, &frame)?,
            },
            None => convert(&mut self.scaler, &frame)?,
        };
        #[cfg(not(target_os = "macos"))]
        let image = convert(&mut self.scaler, &frame)?;
        let duration = frame
            .duration
            .or(self.video_backend.metadata.video.average_frame_interval)
            .unwrap_or_else(|| self.duration().saturating_sub(frame_position));
        eprintln!(
            "PTS {} µs: synchronous frame={:?}",
            frame.timestamp.0,
            started.elapsed()
        );
        Ok(Some((image, frame_position, duration)))
    }

    fn advance_video(
        &mut self,
        clock: &Cell<PlaybackClock>,
        pending_frame: &mut Option<((DisplayedFrame, Duration, Duration), Instant)>,
        cx: &mut Context<Self>,
    ) -> Result<Duration> {
        let anchor = clock
            .get()
            .start_time_of_system
            .context("video clock is paused")?;
        if pending_frame
            .as_ref()
            .is_some_and(|(_, prepared_at)| *prepared_at != anchor)
        {
            *pending_frame = None; // 暂停恢复或 seek 已改变计时起点，旧的待展示帧失效。
        }
        if pending_frame.is_none() {
            *pending_frame = self.prepare_next_frame()?.map(|frame| (frame, anchor));
        }
        let position = clock.get().position();
        if let Some(((_, pts, _), _)) = pending_frame.as_ref() {
            let wait = pts.saturating_sub(position);
            if !wait.is_zero() {
                return Ok(wait.min(MAX_CONTROL_WAIT)); // 提前准备，按 PTS 等待；长间隔中仍检查暂停/seek。
            }
            let (frame, _) = pending_frame.take().expect("prepared video frame exists");
            self.displayed = Some(frame);
            cx.notify();
            return Ok(Duration::ZERO); // 下一轮准备后续帧，不等到其展示时刻才开始解码。
        }
        let video_remaining = match &self.displayed {
            Some((_, pts, duration)) => (*pts + *duration).saturating_sub(position),
            None => Duration::ZERO,
        };
        let audio_remaining = self.audio_output.remaining_duration()?;
        if self.video_backend.audio.is_drained()
            && audio_remaining.is_zero()
            && video_remaining.is_zero()
        {
            self.audio_output.set_playing(false)?;
            self.playback_state = PlaybackState::Ended;
            clock.set(PlaybackClock {
                start_position_of_video: self.duration(),
                start_time_of_system: None,
            });
            cx.notify();
            return Ok(Duration::ZERO);
        }
        if !self.video_backend.audio.is_drained() {
            return Ok(MAX_CONTROL_WAIT);
        }
        Ok(video_remaining.max(audio_remaining).min(MAX_CONTROL_WAIT))
    }

    fn advance_audio(&mut self) -> Result<Duration> {
        let cycle_start = Instant::now();
        if !self.video_backend.audio.is_drained() {
            let wait = self.audio_output.compute_time_to_wait(Duration::ZERO)?;
            if !wait.is_zero() {
                return Ok(wait.min(MAX_CONTROL_WAIT));
            }
            if let Some(samples) = self.video_backend.audio.next_samples()? {
                self.audio_output.enqueue_samples(samples)?;
            }
        }
        if self.video_backend.audio.is_drained() {
            return Ok(MAX_CONTROL_WAIT); // 输出自行消费尾部；视频循环统一判断音画是否都结束。
        }
        Ok(self
            .audio_output
            .compute_time_to_wait(cycle_start.elapsed())?
            .min(MAX_CONTROL_WAIT))
    }
}

const MAX_CONTROL_WAIT: Duration = Duration::from_millis(100); // 限制控制响应延迟，不对视频 PTS 做取整。
const VIDEO_LOOP: usize = 0;
const AUDIO_LOOP: usize = 1;

trait WaitUntilPlaying {
    async fn wait_until_playing(
        &self,
        clock: &Cell<PlaybackClock>,
        loop_index: usize,
        cx: &mut AsyncApp,
    ) -> Result<()>;
}

impl WaitUntilPlaying for WeakEntity<VideoPlayer> {
    async fn wait_until_playing(
        &self,
        clock: &Cell<PlaybackClock>,
        loop_index: usize,
        cx: &mut AsyncApp,
    ) -> Result<()> {
        poll_fn(|task_cx| {
            self.update(cx, |player, _cx| {
                if matches!(player.playback_state, PlaybackState::Playing) {
                    if !player.audio_output.is_playing()? {
                        if clock.get().start_position_of_video >= player.duration()
                            && player.video_backend.video.is_drained()
                            && player.video_backend.audio.is_drained()
                        {
                            player.seek(Duration::ZERO)?;
                        }
                        while !player.video_backend.audio.is_drained()
                            && player
                                .audio_output
                                .compute_time_to_wait(Duration::ZERO)?
                                .is_zero()
                        {
                            player.advance_audio()?; // 设备仍停止时完成短预缓冲，再启动共同计时。
                        }
                        let position = player.video_backend.audio.seek_position();
                        player.audio_output.set_playing(true)?;
                        clock.set(PlaybackClock {
                            start_position_of_video: position,
                            start_time_of_system: Some(Instant::now()),
                        });
                    }
                    player.play_wakers[loop_index] = None;
                    Poll::Ready(Ok(()))
                } else {
                    if clock.get().start_time_of_system.is_some() {
                        let position = if player.audio_output.is_playing()? {
                            let position = clock.get().position().min(player.duration());
                            player.seek(position)?; // 暂停只重置解码和输出位置，保留 displayed，不额外展示一帧。
                            position
                        } else {
                            player.video_backend.audio.seek_position() // 暂停前发生了 seek，保留解码器的目标。
                        };
                        clock.set(PlaybackClock {
                            start_position_of_video: position,
                            start_time_of_system: None,
                        });
                    }
                    player.play_wakers[loop_index] = Some(task_cx.waker().clone());
                    Poll::Pending
                }
            })?
        })
        .await
    }
}

async fn run_player(player: WeakEntity<VideoPlayer>, cx: &mut AsyncApp) -> Result<()> {
    let mut error_cx = cx.clone();
    select! {
        device_error_result = async {
            let error = player.update(&mut error_cx, |player, _| player.audio_output.detect_error())?;
            error.await
        }.fuse() => device_error_result,
        playback_result = async {
            let bge = cx.background_executor().clone();
            let clock = Cell::new(PlaybackClock {
                start_position_of_video: Duration::ZERO,
                start_time_of_system: None
            });

            let mut video_cx = cx.clone();
            let mut video_loop = async || -> Result<()> {
                let mut pending_frame = None;
                loop {
                    player.wait_until_playing(&clock, VIDEO_LOOP, &mut video_cx).await?;
                    let wait = player.update(&mut video_cx, |player, cx| player.advance_video(&clock, &mut pending_frame, cx))??;
                    bge.timer(wait).await;
                }
            };

            let mut audio_cx = cx.clone();
            let mut audio_loop = async || -> Result<()> {
                loop {
                    player.wait_until_playing(&clock, AUDIO_LOOP, &mut audio_cx).await?;
                    let wait = player.update(&mut audio_cx, |player, _| player.advance_audio())??;
                    bge.timer(wait).await;
                }
            };
            try_join!(video_loop(), audio_loop())?;
            Ok(())
        }.fuse() => playback_result,
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
