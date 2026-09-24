use crate::audio_output::AudioOutput;
use anyhow::Result;
use futures::{FutureExt, select};
use gpui::{AsyncApp, Context, FocusHandle, WeakEntity, Window, actions};
use opencut_player::video3::{AudioBackend, AudioSamples};
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
    pub focus: FocusHandle,
}

impl AudioPlayer {
    pub fn new(path: PathBuf, window: &mut Window, cx: &mut Context<Self>) -> Result<Self> {
        let mut audio_backend = AudioBackend::open(&path)?;
        let audio_output = AudioOutput::open()?;
        audio_backend.audio.configure_output(&audio_output.format)?;
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        let player = Self {
            audio_backend,
            audio_output,
            playback_state: PlaybackState::Playing,
            play_waker: None,
            position: Duration::ZERO,
            title: path.display().to_string(),
            focus,
        };
        cx.spawn(async move |player, cx| {
            if let Err(error) = run_player(player.clone(), cx).await {
                // 取消播放 future 不会销毁 player 持有的设备流，需要显式停止输出。
                if let Ok(Err(stop_error)) =
                    player.update(cx, |player, _| player.audio_output.set_playing(false))
                {
                    eprintln!("Stopping audio output failed: {stop_error:?}");
                }
                eprintln!("Audio player failed: {error:?}");
                std::process::exit(1);
            }
        })
        .detach();
        Ok(player)
    }
}

actions!(opencut, [ToggleAudio]);

pub enum PlaybackState {
    Playing,
    Paused,
    Ended,
}

impl AudioPlayer {
    pub fn seek(&mut self, position: Duration, cx: &mut Context<Self>) -> Result<()> {
        let position = position.min(self.audio_backend.metadata.duration);
        self.audio_output.clear()?;
        self.audio_backend.audio.seek(position)?;
        self.position = position;
        if matches!(self.playback_state, PlaybackState::Ended) {
            self.playback_state = PlaybackState::Paused;
        }
        cx.notify();
        Ok(())
    }

    pub fn toggle_playback(&mut self, cx: &mut Context<Self>) -> Result<()> {
        if matches!(self.playback_state, PlaybackState::Ended) {
            self.seek(Duration::ZERO, cx)?;
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

trait WaitUntilPlaying {
    async fn wait_until_playing(&self, cx: &mut AsyncApp) -> Result<()>;
}

impl WaitUntilPlaying for WeakEntity<AudioPlayer> {
    async fn wait_until_playing(&self, cx: &mut AsyncApp) -> Result<()> {
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

async fn run_player(player: WeakEntity<AudioPlayer>, cx: &mut AsyncApp) -> Result<()> {
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
            let error = player.update(&mut error_cx, |player, _| player.audio_output.detect_error())?;
            error.await
        }.fuse() => device_error_result,
        playback_result = async {
            let bge = cx.background_executor().clone();
            loop {
                player.wait_until_playing(cx).await?;
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
                })??;
                // Recheck the decoder and output after waiting so pause and seek still
                // apply during the tail. Once output drains, the play gate waits for replay.
                bge.timer(time_to_wait).await;
            }
        }.fuse() => playback_result,
    }
}
