use crate::{Seeker, WaitUntilPlaying, audio_output::AudioOutput};
use anyhow::{Context as _, Result, bail};
#[cfg(target_os = "macos")]
use core_video::pixel_buffer::CVPixelBuffer;
#[cfg(target_os = "macos")]
use engine::gpu::GpuResources;
use futures::{FutureExt, select, try_join};
use gpui::{AsyncApp, Context, Entity, Task, Window};
use media_backend::VideoBackend;
#[cfg(not(target_os = "macos"))]
use std::convert::Infallible as CVPixelBuffer; // 不支持的平台无法构造视频 surface。
use std::{
    cell::Cell,
    future::poll_fn,
    path::PathBuf,
    task::{Poll, Waker},
    time::{Duration, Instant},
};

#[rustfmt::skip]
pub struct VideoPlayer {
    video_backend: VideoBackend,
    audio_output: AudioOutput,                                   // 已打开的音频设备；有设备不代表正在播放。
    #[cfg(target_os = "macos")]
    gpu: GpuResources,
    play_wakers: Vec<Waker>,                              // 分别唤醒视频、音频循环；只保存等待者，不保存播放进度。
    pub displayed: Option<(CVPixelBuffer, Duration, Duration)>, // (图像, 帧 PTS, 该帧时长)；None：尚未呈现首帧。
    pub playback_state: PlaybackState,
    pub title: String,
    pending_seek: Option<Duration>,                       // 拖动请求的最新目标位置，下一帧执行；Some 表示已安排执行，新请求只覆盖目标。
}

impl VideoPlayer {
    /// Opens the media and output devices. Call [`Self::start`] once the player is in an entity.
    pub fn new(path: PathBuf) -> Result<Self> {
        #[cfg(not(target_os = "macos"))]
        bail!("Video playback requires macOS GPU surfaces");
        // 同步打开和配置；性能成本直接体现在调用处，不交给后台 worker。
        let mut backend = VideoBackend::open(&path)?;
        let audio_output = AudioOutput::open()?;
        backend.audio.configure_output(&audio_output.format)?;
        #[cfg(target_os = "macos")]
        let gpu = GpuResources::new((
            backend.metadata.video.width as usize,
            backend.metadata.video.height as usize,
        ))?;
        let player = Self {
            video_backend: backend,
            audio_output,
            #[cfg(target_os = "macos")]
            gpu,
            play_wakers: Vec::new(),
            displayed: None,
            playback_state: PlaybackState::Playing,
            title: path.display().to_string(),
            pending_seek: None,
        };
        Ok(player)
    }

    /// Seeks to `position` on the next frame. Requests arriving before then only replace the
    /// target, so a seek slower than the pointer never queues stale positions.
    pub(crate) fn request_seek(
        &mut self,
        position: Duration,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let scheduled = self.pending_seek.is_some();
        self.pending_seek = Some(position);
        if scheduled {
            return;
        }
        cx.on_next_frame(window, |player, _, cx| {
            let Some(position) = player.pending_seek.take() else {
                return;
            };
            match player.seek(position) {
                Ok(()) => cx.notify(),
                Err(error) => {
                    eprintln!("Player seek failed: {error:?}");
                    std::process::exit(1);
                }
            }
        });
    }

    /// Starts playback once. The owner must retain the task and drop it before the player.
    pub fn start(&mut self, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |player, cx| {
            let Some(player) = player.upgrade() else {
                return;
            };
            if let Err(error) = run_player(player, cx).await {
                eprintln!("Player failed: {error:?}");
                std::process::exit(1);
            }
        })
    }

    pub fn duration(&self) -> Duration {
        self.video_backend.metadata.duration
    }

    pub fn is_ended(&self) -> Result<bool> {
        Ok(matches!(self.playback_state, PlaybackState::Paused)
            && self.video_backend.video.is_drained()
            && self.video_backend.audio.is_drained()
            && !self.audio_output.is_playing() // 自然结束时停止设备并保留 EOF；用户暂停会重新定位解码器。
            && self.audio_output.remaining_duration()?.is_zero())
    }

    #[rustfmt::skip]
    pub fn toggle_playback(&mut self, cx: &mut Context<Self>) {
        self.playback_state = match self.playback_state {
            PlaybackState::Playing => PlaybackState::Paused,
            PlaybackState::Paused  => PlaybackState::Playing,
        };
        if matches!(self.playback_state, PlaybackState::Playing) {
            for waker in self.play_wakers.drain(..) {
                waker.wake();
            }
        }
        cx.notify();
    }
}

pub enum PlaybackState {
    Playing, // 两个循环继续推进，共用同一个媒体时间基准。
    Paused,  // 用户暂停、时钟冻结；seek 直接更新画面。
}

impl Drop for VideoPlayer {
    fn drop(&mut self) {
        for waker in self.play_wakers.drain(..) {
            waker.wake();
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

impl Seeker for VideoPlayer {
    fn seek(&mut self, position: Duration) -> Result<()> {
        self.audio_output.clear_at(position)?;
        self.video_backend.video.seek(position)?;
        self.video_backend.audio.seek(position)?;
        if let Some(frame) = self.prepare_next_frame()? {
            self.displayed = Some(frame);
        }
        Ok(())
    }
}

impl VideoPlayer {
    fn prepare_next_frame(&mut self) -> Result<Option<(CVPixelBuffer, Duration, Duration)>> {
        let started = Instant::now();
        let Some(frame) = self.video_backend.video.next_frame()? else {
            return Ok(None);
        };
        let frame_position = Duration::from_micros(frame.timestamp.0.max(0) as u64);
        #[cfg(target_os = "macos")]
        let image = self.gpu.convert(&frame)?;
        #[cfg(not(target_os = "macos"))]
        let image: VideoSurface = bail!("Video playback requires macOS GPU surfaces");
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
        pending_frame: &mut Option<((CVPixelBuffer, Duration, Duration), Instant)>,
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
            self.playback_state = PlaybackState::Paused;
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
impl WaitUntilPlaying for Entity<VideoPlayer> {
    async fn wait_until_playing(&self, cx: &mut AsyncApp) {
        poll_fn(|task_cx| {
            self.update(cx, |player, _| {
                if matches!(player.playback_state, PlaybackState::Playing) {
                    Poll::Ready(())
                } else {
                    if !player
                        .play_wakers
                        .iter()
                        .any(|waker| waker.will_wake(task_cx.waker()))
                    {
                        player.play_wakers.push(task_cx.waker().clone());
                    }
                    Poll::Pending
                }
            })
        })
        .await
    }
}

impl VideoPlayer {
    fn sync_playback_clock(&mut self, clock: &Cell<PlaybackClock>) -> Result<()> {
        if matches!(self.playback_state, PlaybackState::Playing) {
            if !self.audio_output.is_playing() {
                if clock.get().start_position_of_video >= self.duration()
                    && self.video_backend.video.is_drained()
                    && self.video_backend.audio.is_drained()
                {
                    self.seek(Duration::ZERO)?;
                }
                while !self.video_backend.audio.is_drained()
                    && self
                        .audio_output
                        .compute_time_to_wait(Duration::ZERO)?
                        .is_zero()
                {
                    self.advance_audio()?;
                }
                let position = self.video_backend.audio.seek_position();
                self.audio_output.set_playing(true)?;
                clock.set(PlaybackClock {
                    start_position_of_video: position,
                    start_time_of_system: Some(Instant::now()),
                });
            }
        } else if clock.get().start_time_of_system.is_some() {
            let position = if self.audio_output.is_playing() {
                let position = clock.get().position().min(self.duration());
                self.seek(position)?;
                position
            } else {
                self.video_backend.audio.seek_position()
            };
            clock.set(PlaybackClock {
                start_position_of_video: position,
                start_time_of_system: None,
            });
        }
        Ok(())
    }
}

/// The owner cancels the task to release its strong player reference.
async fn run_player(player: Entity<VideoPlayer>, cx: &mut AsyncApp) -> Result<()> {
    let mut error_cx = cx.clone();
    select! {
        device_error_result = async {
            let error = player.update(&mut error_cx, |player, _| player.audio_output.detect_error());
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
                    player.update(&mut video_cx, |player, _| player.sync_playback_clock(&clock))?; // 等待前处理暂停：冻结时钟并停止、重置音频输出。
                    player.wait_until_playing(&mut video_cx).await;
                    player.update(&mut video_cx, |player, _| player.sync_playback_clock(&clock))?; // 恢复后预缓冲、启动输出和时钟；持续播放时不做额外操作。
                    let wait = player.update(&mut video_cx, |player, cx| player.advance_video(&clock, &mut pending_frame, cx))?;
                    bge.timer(wait).await;
                }
            };

            let mut audio_cx = cx.clone();
            let mut audio_loop = async || -> Result<()> {
                loop {
                    player.update(&mut audio_cx, |player, _| player.sync_playback_clock(&clock))?; // 等待前处理暂停：冻结时钟并停止、重置音频输出。
                    player.wait_until_playing(&mut audio_cx).await;
                    player.update(&mut audio_cx, |player, _| player.sync_playback_clock(&clock))?; // 恢复后预缓冲、启动输出和时钟；持续播放时不做额外操作。
                    let wait = player.update(&mut audio_cx, |player, _| player.advance_audio())?;
                    bge.timer(wait).await;
                }
            };
            try_join!(video_loop(), audio_loop())?;
            Ok(())
        }.fuse() => playback_result,
    }
}
