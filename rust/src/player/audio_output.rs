use anyhow::{Context, Result, anyhow, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use futures::{
    FutureExt,
    channel::oneshot::{Receiver, Sender, channel},
    future::Shared,
};
use opencut_player::video3::{AudioSamples, PcmFormat};
use std::{
    collections::VecDeque,
    future::Future,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub struct AudioOutput {
    pub format: PcmFormat,
    buffer: Arc<Mutex<OutputBuffer>>,
    error_sender: Arc<Mutex<Option<Sender<cpal::StreamError>>>>,
    device_error: Shared<Receiver<cpal::StreamError>>,
    stream: cpal::Stream,
}

impl AudioOutput {
    pub fn open() -> Result<Self> {
        let position = Duration::ZERO;
        let (error_sender, device_error) = channel();
        let error_sender = Arc::new(Mutex::new(Some(error_sender)));
        let device_error = device_error.shared();
        let device = cpal::default_host()
            .default_output_device()
            .context("no audio output device")?;
        let default = device
            .default_output_config()
            .context("reading audio output configuration")?;
        let config = if default.sample_format() == cpal::SampleFormat::F32 {
            default.config()
        } else {
            let mut selected = None;
            for range in device.supported_output_configs()? {
                if range.sample_format() == cpal::SampleFormat::F32 {
                    selected = Some(range.with_max_sample_rate().config());
                    break;
                }
            }
            selected.context("audio device has no f32 output configuration")?
        };
        let format = PcmFormat::default_layout(config.sample_rate, config.channels)?;
        // Bound decode-ahead to one second of interleaved PCM. Submission never
        // grows the buffer, and the device callback never allocates or waits.
        let capacity = (config.sample_rate as usize)
            .checked_mul(usize::from(config.channels))
            .context("audio output buffer is too large")?;
        let buffer = Arc::new(Mutex::new(OutputBuffer::new(
            capacity,
            position,
            config.sample_rate,
        )));
        let callback_buffer = buffer.clone();
        let callback_error_sender = error_sender.clone();
        let channels = usize::from(config.channels);
        let sample_rate = config.sample_rate;
        let mut skipped_frames = 0;
        let stream = device
            .build_output_stream(
                &config,
                move |buffer: &mut [f32], info: &cpal::OutputCallbackInfo| {
                    buffer.fill(0.0);
                    let Ok(mut queued) = callback_buffer.try_lock() else {
                        // 锁竞争时也已经输出了静音；下次回调补计这些设备帧。
                        skipped_frames += buffer.len() / channels;
                        return;
                    };
                    queued.render(buffer, info, channels, sample_rate, skipped_frames);
                    skipped_frames = 0;
                },
                move |error| {
                    let sender = {
                        let Ok(mut sender) = callback_error_sender.lock() else {
                            return;
                        };
                        sender.take()
                    };
                    // 只发送第一次错误；oneshot 保存结果并唤醒外层 select。
                    // 接收端已被丢弃时，播放任务已经退出，无需再次报告错误。
                    if let Some(sender) = sender {
                        let _ = sender.send(error);
                    }
                },
                None,
            )
            .context("creating audio output stream")?;
        Ok(Self {
            format,
            buffer,
            error_sender,
            device_error,
            stream,
        })
    }

    /// 等待设备错误，对调用方只暴露 async 语义，不暴露通知机制。
    pub fn detect_error(&self) -> impl Future<Output = Result<()>> + use<> {
        // 等待期间不借用 AudioOutput，允许播放循环继续提交数据或重建流。
        let device_error = self.device_error.clone();
        async move { Err(device_error.await?).context("audio output failed") }
    }

    pub fn is_playing(&self) -> Result<bool> {
        let buffer = self
            .buffer
            .lock()
            .map_err(|_| anyhow!("audio output buffer lock is poisoned"))?;
        Ok(buffer.playing)
    }

    pub fn set_playing(&self, playing: bool) -> Result<()> {
        {
            let mut buffer = self
                .buffer
                .lock()
                .map_err(|_| anyhow!("audio output buffer lock is poisoned"))?;
            if buffer.playing == playing {
                return Ok(());
            }
            buffer.playing = playing;
        }
        // 不持有队列锁调用设备 API；play/pause 可能等待设备回调。
        if playing {
            self.stream.play().context("starting audio output")?;
        } else {
            self.stream.pause().context("pausing audio output")?;
        }
        Ok(())
    }

    /// 按提交顺序加入 PCM 并启动输出，保留独立音频播放器的原有语义。
    /// 返回成功只表示入队和启动成功；播放完成仍需等待队列及设备尾部耗尽。
    pub fn push_samples(&mut self, samples: AudioSamples) -> Result<()> {
        validate_samples(&samples, &self.format)?;
        {
            let mut buffer = self
                .buffer
                .lock()
                .map_err(|_| anyhow!("audio output buffer lock is poisoned"))?;
            if samples.samples.len() > buffer.samples.capacity() - buffer.samples.len()
                || buffer.spans.len() == buffer.spans.capacity()
            {
                bail!("audio output buffer is full");
            }
            if samples.frame_count > 0 {
                // None 表示紧接上一块播放；seek/underrun 后也不按 PTS 丢弃 PCM。
                buffer.spans.push_back(AudioSpan {
                    start: None,
                    frames: samples.frame_count,
                });
                buffer.samples.extend(samples.samples);
            }
        }
        self.set_playing(true)
    }

    /// 仅把 PCM 放入软件队列，不启动设备，更不表示音频已经播放完。
    /// 回调随后按 PTS 取走 samples；设备仍需等待输出延迟并逐个播放它们。
    pub fn enqueue_samples(&mut self, samples: AudioSamples) -> Result<()> {
        validate_samples(&samples, &self.format)?;
        if samples.frame_count == 0 {
            return Ok(());
        }
        let mut buffer = self
            .buffer
            .lock()
            .map_err(|_| anyhow!("audio output buffer lock is poisoned"))?;
        // PTS 使用微秒，换算回 sample index 时四舍五入，避免每块少一个采样。
        let start = (samples.timestamp.0 as f64 * f64::from(self.format.sample_rate) / 1_000_000.0)
            .round() as i64;
        let queued_end = buffer.queued_end().unwrap_or(buffer.next_frame);
        let skip = (queued_end.max(buffer.next_frame) - start)
            .max(0)
            .min(samples.frame_count as i64) as usize;
        let frames = samples.frame_count - skip;
        if frames == 0 {
            return Ok(());
        }
        let count = frames * self.format.channel_layout.len();
        if count > buffer.samples.capacity() - buffer.samples.len()
            || buffer.spans.len() == buffer.spans.capacity()
        {
            bail!("audio output buffer is full");
        }
        buffer.spans.push_back(AudioSpan {
            start: Some(start + skip as i64),
            frames,
        });
        buffer.samples.extend(
            samples
                .samples
                .into_iter()
                .skip(skip * self.format.channel_layout.len()),
        );
        Ok(())
    }

    /// 为回调和调度抖动保留 50 ms；这个值与视频帧时长无关。
    pub fn compute_time_to_wait(&self, cycle_elapsed: Duration) -> Result<Duration> {
        const MIN_REFILL_RESERVE: Duration = Duration::from_millis(50);
        let buffer = self
            .buffer
            .lock()
            .map_err(|_| anyhow!("audio output buffer lock is poisoned"))?;
        let end = buffer.queued_end().unwrap_or(buffer.next_frame);
        let queued = Duration::from_secs_f64(
            (end - buffer.next_frame).max(0) as f64 / f64::from(self.format.sample_rate),
        );
        Ok(queued.saturating_sub(MIN_REFILL_RESERVE.max(cycle_elapsed)))
    }

    /// 软件队列和已经送往设备的真实音频都播完，才返回零。
    pub fn remaining_duration(&self) -> Result<Duration> {
        let buffer = self
            .buffer
            .lock()
            .map_err(|_| anyhow!("audio output buffer lock is poisoned"))?;
        let queued_end = buffer.queued_end().unwrap_or(buffer.next_frame);
        let queued = Duration::from_secs_f64(
            (queued_end - buffer.next_frame).max(0) as f64 / f64::from(self.format.sample_rate),
        );
        let device_tail = buffer
            .device_tail
            .map_or(Duration::ZERO, |end| end.saturating_duration_since(Instant::now()));
        Ok(queued + device_tail)
    }

    /// 清空输出并保持停止，保留独立音频播放器的无参数接口。
    pub fn clear(&mut self) -> Result<()> {
        self.clear_at(Duration::ZERO)
    }

    /// Replace the stream to discard queued PCM and leave output stopped for a seek.
    pub fn clear_at(&mut self, position: Duration) -> Result<()> {
        // Design decision pending playback testing: keep stream replacement for
        // now. If the short tail of old audio after a seek is acceptable, clear
        // only the software PCM queue and let device-submitted samples finish.
        // The 50 ms refill reserve is not a limit on the device's buffered audio.
        // Clearing the software queue alone leaves samples already submitted to
        // the device. A fresh stream also discards that stream's pending output.
        // 重建流后复用同一组错误通知，已经创建的 future 继续等待新流的错误。
        let output = {
            let error_sender = self.error_sender.clone();
            let device_error = self.device_error.clone();
            let device = cpal::default_host()
                .default_output_device()
                .context("no audio output device")?;
            let default = device
                .default_output_config()
                .context("reading audio output configuration")?;
            let config = if default.sample_format() == cpal::SampleFormat::F32 {
                default.config()
            } else {
                let mut selected = None;
                for range in device.supported_output_configs()? {
                    if range.sample_format() == cpal::SampleFormat::F32 {
                        selected = Some(range.with_max_sample_rate().config());
                        break;
                    }
                }
                selected.context("audio device has no f32 output configuration")?
            };
            let format = PcmFormat::default_layout(config.sample_rate, config.channels)?;
            // Bound decode-ahead to one second of interleaved PCM. Submission never
            // grows the buffer, and the device callback never allocates or waits.
            let capacity = (config.sample_rate as usize)
                .checked_mul(usize::from(config.channels))
                .context("audio output buffer is too large")?;
            let buffer = Arc::new(Mutex::new(OutputBuffer::new(
                capacity,
                position,
                config.sample_rate,
            )));
            let callback_buffer = buffer.clone();
            let callback_error_sender = error_sender.clone();
            let channels = usize::from(config.channels);
            let sample_rate = config.sample_rate;
            let mut skipped_frames = 0;
            let stream = device
                .build_output_stream(
                    &config,
                    move |buffer: &mut [f32], info: &cpal::OutputCallbackInfo| {
                        buffer.fill(0.0);
                        let Ok(mut queued) = callback_buffer.try_lock() else {
                            // 锁竞争时也已经输出了静音；下次回调补计这些设备帧。
                            skipped_frames += buffer.len() / channels;
                            return;
                        };
                        queued.render(buffer, info, channels, sample_rate, skipped_frames);
                        skipped_frames = 0;
                    },
                    move |error| {
                        let sender = {
                            let Ok(mut sender) = callback_error_sender.lock() else {
                                return;
                            };
                            sender.take()
                        };
                        // 只发送第一次错误；oneshot 保存结果并唤醒外层 select。
                        // 接收端已被丢弃时，播放任务已经退出，无需再次报告错误。
                        if let Some(sender) = sender {
                            let _ = sender.send(error);
                        }
                    },
                    None,
                )
                .context("creating audio output stream")?;
            Self {
                format,
                buffer,
                error_sender,
                device_error,
                stream,
            }
        };
        if output.format != self.format {
            bail!("audio output format changed while clearing playback");
        }
        output.set_playing(false)?;
        // Prepare the replacement before stopping the current stream so an open
        // or format error leaves the current playback untouched.
        self.set_playing(false)?;
        *self = output;
        Ok(())
    }
}

struct AudioSpan {
    // Some 按媒体时间播放；None 按提交顺序播放。
    start: Option<i64>,
    frames: usize,
}

struct OutputBuffer {
    samples: VecDeque<f32>,
    spans: VecDeque<AudioSpan>,
    next_frame: i64,
    playing: bool,
    device_tail: Option<Instant>, // 最后提交的真实 PCM 预计播完的本地时刻；仅用于判断输出耗尽。
}

impl OutputBuffer {
    fn new(capacity: usize, position: Duration, sample_rate: u32) -> Self {
        Self {
            samples: VecDeque::with_capacity(capacity),
            spans: VecDeque::with_capacity(sample_rate as usize),
            next_frame: (position.as_secs_f64() * f64::from(sample_rate)).round() as i64,
            playing: false,
            device_tail: None,
        }
    }

    fn queued_end(&self) -> Option<i64> {
        if self.spans.is_empty() {
            return None;
        }
        let mut end = self.next_frame;
        for span in &self.spans {
            end = match span.start {
                Some(start) => end.max(start + span.frames as i64),
                None => end + span.frames as i64,
            };
        }
        Some(end)
    }

    fn render(
        &mut self,
        output: &mut [f32],
        info: &cpal::OutputCallbackInfo,
        channels: usize,
        sample_rate: u32,
        skipped_frames: usize,
    ) {
        if !self.playing {
            return;
        }
        let now = Instant::now();
        self.next_frame += skipped_frames as i64;
        let start = self.next_frame;
        let frames = output.len() / channels;
        let end = start + frames as i64;
        let mut cursor = start;
        while let Some(span) = self.spans.front_mut() {
            let mut span_start = span.start.unwrap_or(cursor);
            // 仅有时间戳的块需要丢弃过期 PCM；顺序提交的块始终接着播放。
            let skip = (cursor - span_start).max(0).min(span.frames as i64) as usize;
            self.samples.drain(..skip * channels);
            span_start += skip as i64;
            span.frames -= skip;
            if span.frames == 0 {
                self.spans.pop_front();
                continue;
            }
            if span_start >= end {
                break;
            }
            let offset = (span_start - start) as usize * channels;
            let count = span.frames.min((end - span_start) as usize);
            for sample in &mut output[offset..offset + count * channels] {
                *sample = self.samples.pop_front().unwrap_or_default();
            }
            cursor = span_start + count as i64;
            if span.start.is_some() {
                span.start = Some(cursor);
            }
            span.frames -= count;
            if span.frames == 0 {
                self.spans.pop_front();
            }
            if cursor == end {
                break;
            }
        }
        self.next_frame = end;
        if cursor > start {
            let timestamp = info.timestamp();
            let latency = timestamp
                .playback
                .duration_since(&timestamp.callback)
                .unwrap_or_default();
            let duration = Duration::from_secs_f64((cursor - start) as f64 / f64::from(sample_rate));
            self.device_tail = Some(now + latency + duration); // 输出静音不延长真实音频尾部。
        }
    }
}

fn validate_samples(samples: &AudioSamples, format: &PcmFormat) -> Result<()> {
    if samples.format != *format
        || samples.frame_count.checked_mul(format.channel_layout.len())
            != Some(samples.samples.len())
    {
        bail!("decoded audio does not match output configuration");
    }
    Ok(())
}
