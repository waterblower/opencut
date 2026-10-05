//! Standalone timeline player entity: a [`TimelineBackend`] plus its own loop, controls, and
//! audio output. Mixed audio is queued by timeline position against the backend's clock.

use crate::{Seeker, audio_output::AudioOutput};
use anyhow::Result;
use engine::{
    export::{ClipAudio, mix_timeline_audio},
    timeline_backend::{MAX_CONTROL_WAIT, TimelineBackend},
};
use gpui::{Context, Task, Window};
use media_backend::{AudioSamples, MediaTime};
use std::{collections::HashMap, path::Path, time::Duration};
use timeline::TimelineEditingState;
use ulid::Ulid;

const AUDIO_LEAD: Duration = Duration::from_millis(500); // 混音领先播放时钟的时长；低于输出队列的 1 秒上限。

#[rustfmt::skip]
pub struct TimelinePlayer {
    pub backend: TimelineBackend,                 // 直接修改后需自行 notify 并释放旧图像。
    pub title: String,
    audio_output: AudioOutput,
    audio_readers: HashMap<Ulid, ClipAudio>,      // 按片段顺序读取的解码器；重新开始输出时清空。
    audio_cursor: i64,                            // 下一块待混音的起始采样位置（设备采样率），不是播放位置。
    pending_seek: Option<Duration>,               // 拖动请求的最新目标位置，下一帧执行；Some 表示已安排执行，新请求只覆盖目标。
}

impl TimelinePlayer {
    /// Validates the timeline, opens the audio device, and prepares frame zero, paused.
    /// Media paths resolve against `timeline_directory`, the directory containing the timeline file. Call [`Self::start`] once the player is in an entity.
    pub fn new(timeline: TimelineEditingState, timeline_directory: &Path) -> Result<Self> {
        Ok(Self {
            backend: TimelineBackend::new(timeline, timeline_directory)?,
            title: String::new(),
            audio_output: AudioOutput::open()?,
            audio_readers: HashMap::new(),
            audio_cursor: 0,
            pending_seek: None,
        })
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
                Err(error) => player.fail(error, cx),
            }
        });
    }

    /// Starts the playback loop once. The owner must retain the task and drop it before the player.
    pub fn start(&mut self, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |player, cx| {
            loop {
                let Ok(wait) = player.update(cx, |player, cx| {
                    let result = match player.backend.advance() {
                        Ok(advance) => match player.sync_audio() {
                            Ok(()) => Ok(advance),
                            Err(error) => Err(error),
                        },
                        Err(error) => Err(error),
                    };
                    match result {
                        Ok(advance) => {
                            if advance.changed {
                                cx.notify();
                            }
                            advance.wait
                        }
                        Err(error) => {
                            player.fail(error, cx);
                            MAX_CONTROL_WAIT
                        }
                    }
                }) else {
                    return; // 播放器已释放。
                };
                cx.background_executor().timer(wait).await;
            }
        })
    }

    pub fn play(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let result = match self.backend.play() {
            Ok(()) => self.sync_audio(),
            Err(error) => Err(error),
        };
        cx.notify();
        result
    }

    pub fn toggle_playback(&mut self, cx: &mut Context<Self>) -> Result<()> {
        if self.backend.is_playing() {
            self.backend.pause();
            let result = self.sync_audio();
            cx.notify();
            result
        } else {
            self.play(cx)
        }
    }

    /// Logs the error and pauses on the last good frame.
    /// The playback loop stops audio output on its next step.
    pub(crate) fn fail(&mut self, error: anyhow::Error, cx: &mut Context<Self>) {
        eprintln!("Timeline player failed: {error:?}");
        self.backend.pause();
        cx.notify();
    }

    /// Starts, refills, or stops the audio output to match the backend's playback state.
    fn sync_audio(&mut self) -> Result<()> {
        if !self.backend.is_playing() {
            if self.audio_output.is_playing() {
                self.audio_output.clear_at(self.backend.clock_position())?; // 丢弃已排队的 PCM 并保持停止。
            }
            return Ok(());
        }
        let rate = self.audio_output.format.sample_rate;
        if !self.audio_output.is_playing() {
            // 开始播放或 seek 后：输出队列从时钟位置重新开始，解码器按新位置重新打开。
            let position = self.backend.clock_position();
            self.audio_output.clear_at(position)?;
            self.audio_readers.clear();
            self.audio_cursor = sample_index(position, rate);
        }
        let duration_end = sample_index(self.backend.duration(), rate);
        let target =
            sample_index(self.backend.clock_position() + AUDIO_LEAD, rate).min(duration_end);
        if self.audio_cursor < target {
            let count = (target - self.audio_cursor) as usize;
            let mixed = mix_timeline_audio(
                self.backend.timeline(),
                self.backend.timeline_directory(),
                &mut self.audio_readers,
                self.audio_cursor,
                count,
                rate,
            )?;
            let channels = self.audio_output.format.channel_layout.len();
            let mut samples = Vec::with_capacity(count * channels);
            for [left, right] in mixed {
                if channels == 1 {
                    samples.push((left + right) * 0.5);
                } else {
                    samples.push(left);
                    samples.push(right);
                    samples.extend(std::iter::repeat_n(0.0, channels - 2)); // 多声道设备只输出前两个声道。
                }
            }
            let timestamp_microseconds = self.audio_cursor * 1_000_000 / i64::from(rate);
            self.audio_output.enqueue_samples(AudioSamples {
                samples,
                timestamp: MediaTime(timestamp_microseconds),
                format: self.audio_output.format.clone(),
                frame_count: count,
            })?;
            self.audio_cursor = target;
        }
        self.audio_output.set_playing(true)
    }
}

impl Seeker for TimelinePlayer {
    fn seek(&mut self, position: Duration) -> Result<()> {
        self.backend.seek(position)?;
        // 停止输出；播放中时 sync_audio 从新位置清空队列并重新混音。
        self.audio_output.set_playing(false)?;
        self.sync_audio()
    }
}

/// The sample containing `position` at `rate`.
fn sample_index(position: Duration, rate: u32) -> i64 {
    (position.as_nanos() * u128::from(rate) / 1_000_000_000) as i64
}
