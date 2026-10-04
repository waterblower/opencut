use crate::{WaitUntilPlaying, audio_output::AudioOutput};
use anyhow::Result;
use futures::{FutureExt, select};
use gpui::{AsyncApp, Context, Entity, Task};
use media_backend::{AudioBackend, AudioSamples};
use std::{
    future::poll_fn,
    path::PathBuf,
    task::{Poll, Waker},
    time::{Duration, Instant},
};

pub struct AudioPlayer {
    pub audio_backend: AudioBackend,
    audio_output: AudioOutput,
    pub playback_state: PlaybackState,
    play_waker: Option<Waker>,
    pub position: Duration,
    pub title: String,
}

impl AudioPlayer {
    /// Opens the media and output devices. Call [`Self::start`] once the player is in an entity.
    pub fn new(path: PathBuf) -> Result<Self> {
        let mut audio_backend = AudioBackend::open(&path)?;
        let audio_output = AudioOutput::open()?;
        audio_backend.audio.configure_output(&audio_output.format)?;
        let player = Self {
            audio_backend,
            audio_output,
            playback_state: PlaybackState::Playing,
            play_waker: None,
            position: Duration::ZERO,
            title: path.display().to_string(),
        };
        Ok(player)
    }

    /// Starts playback once. The owner must retain the task and drop it before the player.
    pub fn start(&mut self, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |player, cx| {
            let Some(player) = player.upgrade() else {
                return;
            };
            if let Err(error) = run_player(player, cx).await {
                eprintln!("Audio player failed: {error:?}");
                std::process::exit(1);
            }
        })
    }
}

pub enum PlaybackState {
    Playing,
    Paused,
    Ended,
}

impl AudioPlayer {
    /// The caller notifies after a successful seek.
    pub fn seek(&mut self, position: Duration) -> Result<()> {
        let position = position.min(self.audio_backend.metadata.duration);
        self.audio_output.clear()?;
        self.audio_backend.audio.seek(position)?;
        self.position = position;
        if matches!(self.playback_state, PlaybackState::Ended) {
            self.playback_state = PlaybackState::Paused;
        }
        Ok(())
    }

    pub fn toggle_playback(&mut self, cx: &mut Context<Self>) -> Result<()> {
        if matches!(self.playback_state, PlaybackState::Ended) {
            self.seek(Duration::ZERO)?;
        }
        let playing = !matches!(self.playback_state, PlaybackState::Playing);
        self.audio_output.set_playing(playing)?;
        self.playback_state = if playing {
            PlaybackState::Playing
        } else {
            PlaybackState::Paused
        };
        if playing && let Some(waker) = self.play_waker.take() {
            waker.wake();
        }
        cx.notify();
        Ok(())
    }
}

impl AudioPlayer {
    fn get_next_samples(&mut self, cx: &mut Context<Self>) -> Result<Option<AudioSamples>> {
        let samples = match self.audio_backend.audio.next_samples() {
            // A decoded PCM block is available; update its position below.
            Ok(Some(samples)) => samples,
            // Decoder EOF: the playback loop must still wait for output to drain.
            Ok(None) => return Ok(None),
            // Reading or decoding failed; propagate the error to the playback task.
            Err(error) => return Err(error),
        };
        self.position = Duration::from_micros(samples.timestamp.0.max(0) as u64)
            .min(self.audio_backend.metadata.duration);
        cx.notify();
        Ok(Some(samples))
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        if let Some(waker) = self.play_waker.take() {
            waker.wake();
        }
    }
}

impl WaitUntilPlaying for Entity<AudioPlayer> {
    async fn wait_until_playing(&self, cx: &mut AsyncApp) {
        poll_fn(|task_cx| {
            self.update(cx, |player, _| {
                if matches!(player.playback_state, PlaybackState::Playing) {
                    player.play_waker = None;
                    Poll::Ready(())
                } else {
                    player.play_waker = Some(task_cx.waker().clone());
                    Poll::Pending
                }
            })
        })
        .await
    }
}

/// The owner cancels the task to release its strong player reference.
async fn run_player(player: Entity<AudioPlayer>, cx: &mut AsyncApp) -> Result<()> {
    /// 解码结束后等待尾部音频播完；返回下次检查前的等待时长，耗尽后进入 Ended。
    fn finish_playback(
        player: &mut AudioPlayer,
        cx: &mut Context<AudioPlayer>,
    ) -> Result<Duration> {
        let remaining = player.audio_output.remaining_duration()?;
        if !remaining.is_zero() {
            // 保持 Playing，由外层 timer 等待后再次检查。
            return Ok(remaining);
        }
        // 软件队列和设备尾部均已耗尽，停止输出并显示完成进度。
        player.audio_output.set_playing(false)?;
        player.playback_state = PlaybackState::Ended;
        player.position = player.audio_backend.metadata.duration;
        cx.notify();
        // 播放任务继续存在，下一轮会停在 play gate，等待用户重播。
        Ok(Duration::ZERO)
    }

    let mut error_cx = cx.clone();
    // 在整个播放循环外等待错误，暂停和 timer 等待期间也能被设备错误唤醒。
    // 任一分支完成后，退出本函数并丢弃另一个 future；不会新建后台任务。
    select! {
        device_error_result = async {
            let error = player.update(&mut error_cx, |player, _| player.audio_output.detect_error());
            error.await
        }.fuse() => device_error_result,
        playback_result = async {
            let bge = cx.background_executor().clone();
            loop {
                player.wait_until_playing(cx).await;
                let time_to_wait = player.update(cx, |player, cx| -> Result<Duration> {
                    let cycle_start = Instant::now();
                    match player.get_next_samples(cx)? {
                        // 提交下一块音频，并计算补充数据前的等待时长。
                        Some(samples) => {
                            player.audio_output.push_samples(samples)?;
                            player.audio_output.compute_time_to_wait(cycle_start.elapsed())
                        }
                        // 解码结束后，等待已提交的尾部音频播完。
                        None => finish_playback(player, cx),
                    }
                })?;
                // Recheck the decoder and output after waiting so pause and seek still
                // apply during the tail. Once output drains, the play gate waits for replay.
                bge.timer(time_to_wait).await;
            }
        }.fuse() => playback_result,
    }
}
