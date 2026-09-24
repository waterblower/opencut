use anyhow::{Context, Result, bail};
use cpal::{
    StreamInstant,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
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
    time::Duration,
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
        let buffer = Arc::new(Mutex::new(OutputBuffer {
            samples: VecDeque::with_capacity(capacity),
            device_tail: Duration::ZERO,
        }));
        let callback_buffer = buffer.clone();
        let callback_error_sender = error_sender.clone();
        let channels = usize::from(config.channels);
        let sample_rate = config.sample_rate;
        // Only the device callback needs the timestamp of its last submission.
        let mut last_submission: Option<(StreamInstant, Duration)> = None;
        let stream = device
            .build_output_stream(
                &config,
                move |buffer: &mut [f32], info: &cpal::OutputCallbackInfo| {
                    buffer.fill(0.0);
                    // A busy producer or an empty buffer yields silence rather
                    // than blocking the device's audio thread.
                    let Ok(mut queued) = callback_buffer.try_lock() else {
                        return;
                    };
                    let samples = &mut queued.samples;
                    let count = buffer.len().min(samples.len());
                    let (first, second) = samples.as_slices();
                    let first_count = count.min(first.len());
                    buffer[..first_count].copy_from_slice(&first[..first_count]);
                    buffer[first_count..count].copy_from_slice(&second[..count - first_count]);
                    samples.drain(..count);
                    let timestamp = info.timestamp();
                    if count > 0 {
                        let duration = Duration::from_secs_f64(
                            (count / channels) as f64 / f64::from(sample_rate),
                        );
                        last_submission = Some((timestamp.playback, duration));
                    }
                    // Silence must not extend the tail. Compare the callback clock
                    // with the predicted playback time, including device latency.
                    queued.device_tail = match last_submission {
                        Some((playback, duration)) => {
                            match timestamp.callback.duration_since(&playback) {
                                Some(elapsed) => duration.saturating_sub(elapsed),
                                None => {
                                    playback
                                        .duration_since(&timestamp.callback)
                                        .unwrap_or_default()
                                        + duration
                                }
                            }
                        }
                        None => Duration::ZERO,
                    };
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

    pub fn set_playing(&self, playing: bool) -> Result<()> {
        if playing {
            self.stream.play().context("starting audio output")?;
        } else {
            self.stream.pause().context("pausing audio output")?;
        }
        Ok(())
    }

    /// 将一块 PCM 音频数据放入软件队列，并确保设备输出流正在运行。
    /// 返回成功只表示入队成功、输出流已启动，不表示数据已送到设备，更不表示播放结束。
    /// 随后设备回调会从队列取出 samples，填入设备输出缓冲，
    /// 再由设备按采样率逐个播放；播放结束需要另外等待队列和设备中的尾部音频耗尽。
    pub fn push_samples(&mut self, samples: AudioSamples) -> Result<()> {
        validate_samples(&samples, &self.format)?;
        {
            let mut buffer = match self.buffer.lock() {
                Ok(queued) => queued,
                Err(_) => bail!("audio output buffer lock is poisoned"),
            };
            let queued = &mut buffer.samples;
            if samples.samples.len() > queued.capacity() - queued.len() {
                bail!("audio output buffer is full");
            }
            queued.extend(samples.samples);
        }
        // Release the buffer before starting the stream: play may invoke its callback.
        self.set_playing(true)?;
        Ok(())
    }

    /// Wait until queued PCM reaches the reserve for the next decode cycle.
    pub fn compute_time_to_wait(&self, cycle_elapsed: Duration) -> Result<Duration> {
        let queued_frames = {
            let buffer = match self.buffer.lock() {
                Ok(buffer) => buffer,
                Err(_) => bail!("audio output buffer lock is poisoned"),
            };
            buffer.samples.len() / self.format.channel_layout.len()
        };
        let queued_duration =
            Duration::from_secs_f64(queued_frames as f64 / f64::from(self.format.sample_rate));
        // Keep 50 ms available for device callbacks and scheduling jitter. A
        // slower decode cycle needs at least its observed duration as reserve.
        // The queue already reflects consumption during this cycle; elapsed
        // estimates the next cycle's cost, rather than being subtracted again.
        let refill_reserve = Duration::from_millis(50).max(cycle_elapsed);
        // Refill immediately when the reserve is low, including after an underrun.
        Ok(queued_duration.saturating_sub(refill_reserve))
    }

    /// Estimate the remaining tail from the queue and device callback timestamps.
    /// Zero means the callback clock has passed the last samples' predicted end.
    pub fn remaining_duration(&self) -> Result<Duration> {
        let buffer = match self.buffer.lock() {
            Ok(buffer) => buffer,
            Err(_) => bail!("audio output buffer lock is poisoned"),
        };
        let queued_frames = buffer.samples.len() / self.format.channel_layout.len();
        let queued_duration =
            Duration::from_secs_f64(queued_frames as f64 / f64::from(self.format.sample_rate));
        // Queue removal and device-tail updates share a lock: an empty queue
        // cannot be mistaken for completion while its samples move to the device.
        Ok(queued_duration + buffer.device_tail)
    }

    /// Replace the stream to discard queued PCM and leave output stopped for a seek.
    pub fn clear(&mut self) -> Result<()> {
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
            let buffer = Arc::new(Mutex::new(OutputBuffer {
                samples: VecDeque::with_capacity(capacity),
                device_tail: Duration::ZERO,
            }));
            let callback_buffer = buffer.clone();
            let callback_error_sender = error_sender.clone();
            let channels = usize::from(config.channels);
            let sample_rate = config.sample_rate;
            // Only the device callback needs the timestamp of its last submission.
            let mut last_submission: Option<(StreamInstant, Duration)> = None;
            let stream = device
                .build_output_stream(
                    &config,
                    move |buffer: &mut [f32], info: &cpal::OutputCallbackInfo| {
                        buffer.fill(0.0);
                        // A busy producer or an empty buffer yields silence rather
                        // than blocking the device's audio thread.
                        let Ok(mut queued) = callback_buffer.try_lock() else {
                            return;
                        };
                        let samples = &mut queued.samples;
                        let count = buffer.len().min(samples.len());
                        let (first, second) = samples.as_slices();
                        let first_count = count.min(first.len());
                        buffer[..first_count].copy_from_slice(&first[..first_count]);
                        buffer[first_count..count].copy_from_slice(&second[..count - first_count]);
                        samples.drain(..count);
                        let timestamp = info.timestamp();
                        if count > 0 {
                            let duration = Duration::from_secs_f64(
                                (count / channels) as f64 / f64::from(sample_rate),
                            );
                            last_submission = Some((timestamp.playback, duration));
                        }
                        // Silence must not extend the tail. Compare the callback clock
                        // with the predicted playback time, including device latency.
                        queued.device_tail = match last_submission {
                            Some((playback, duration)) => {
                                match timestamp.callback.duration_since(&playback) {
                                    Some(elapsed) => duration.saturating_sub(elapsed),
                                    None => {
                                        playback
                                            .duration_since(&timestamp.callback)
                                            .unwrap_or_default()
                                            + duration
                                    }
                                }
                            }
                            None => Duration::ZERO,
                        };
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

struct OutputBuffer {
    samples: VecDeque<f32>,
    device_tail: Duration,
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
