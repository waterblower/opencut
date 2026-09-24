use anyhow::{Context, Result, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use opencut_player::video3::{AudioSamples, PcmFormat};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::Duration,
};

pub struct AudioOutput {
    pub format: PcmFormat,
    samples: Arc<Mutex<VecDeque<f32>>>,
    error: Arc<Mutex<Option<cpal::StreamError>>>,
    stream: cpal::Stream,
}

impl AudioOutput {
    pub fn open() -> Result<Self> {
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
        let samples = Arc::new(Mutex::new(VecDeque::<f32>::with_capacity(capacity)));
        let error = Arc::new(Mutex::new(None));
        let callback_samples = samples.clone();
        let callback_error = error.clone();
        let stream = device
            .build_output_stream(
                &config,
                move |buffer: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    buffer.fill(0.0);
                    // A busy producer or an empty buffer yields silence rather
                    // than blocking the device's audio thread.
                    let Ok(mut samples) = callback_samples.try_lock() else {
                        return;
                    };
                    let count = buffer.len().min(samples.len());
                    let (first, second) = samples.as_slices();
                    let first_count = count.min(first.len());
                    buffer[..first_count].copy_from_slice(&first[..first_count]);
                    buffer[first_count..count].copy_from_slice(&second[..count - first_count]);
                    samples.drain(..count);
                },
                move |error| {
                    let Ok(mut pending) = callback_error.lock() else {
                        return;
                    };
                    if pending.is_none() {
                        *pending = Some(error);
                    }
                },
                None,
            )
            .context("creating audio output stream")?;
        Ok(Self {
            format,
            samples,
            error,
            stream,
        })
    }

    pub fn set_playing(&self, playing: bool) -> Result<()> {
        if playing {
            self.stream.play().context("starting audio output")?;
        } else {
            self.stream.pause().context("pausing audio output")?;
        }
        Ok(())
    }

    /// Submit one decoded block and ensure output is playing.
    pub fn push_samples(&mut self, samples: AudioSamples) -> Result<()> {
        validate_samples(&samples, &self.format)?;
        {
            let mut pending = match self.error.lock() {
                Ok(pending) => pending,
                Err(_) => bail!("audio output error lock is poisoned"),
            };
            if let Some(error) = pending.take() {
                return Err(error).context("audio output failed");
            }
        }
        {
            let mut queued = match self.samples.lock() {
                Ok(queued) => queued,
                Err(_) => bail!("audio output buffer lock is poisoned"),
            };
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
            let samples = match self.samples.lock() {
                Ok(samples) => samples,
                Err(_) => bail!("audio output buffer lock is poisoned"),
            };
            samples.len() / self.format.channel_layout.len()
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

    /// Replace the stream to discard queued PCM and leave output stopped for a seek.
    pub fn clear(&mut self) -> Result<()> {
        // Clearing the software queue alone leaves samples already submitted to
        // the device. A fresh stream also discards that stream's pending output.
        let output = Self::open()?;
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

fn validate_samples(samples: &AudioSamples, format: &PcmFormat) -> Result<()> {
    if samples.format != *format
        || samples.frame_count.checked_mul(format.channel_layout.len())
            != Some(samples.samples.len())
    {
        bail!("decoded audio does not match output configuration");
    }
    Ok(())
}
