use anyhow::{Context, Result, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use opencut_player::video3::{AudioSamples, PcmFormat};
use std::{
    sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel},
    time::Duration,
};

pub struct AudioOutput {
    pub format: PcmFormat,
    pub samples: SyncSender<Option<AudioSamples>>,
    pub events: Receiver<OutputEvent>,
    pub errors: Receiver<cpal::StreamError>,
    stream: cpal::Stream,
}

pub enum OutputEvent {
    Position(Duration),
    Finished(Duration),
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
        let channels = usize::from(config.channels);
        let rate = config.sample_rate;
        // Eight decoded blocks bound decode-ahead and memory use.
        let (samples, receiver) = sync_channel::<Option<AudioSamples>>(8);
        let (events, event_receiver) = sync_channel(32);
        // Device failures must not be displaced by progress notifications.
        let (errors, error_receiver) = sync_channel(1);
        let mut pending: Option<AudioSamples> = None;
        let mut cursor = 0;
        let mut ended = false;
        let mut reported_end = false;
        let stream = device
            .build_output_stream(
                &config,
                move |buffer: &mut [f32], info: &cpal::OutputCallbackInfo| {
                    let mut written = 0;
                    while written < buffer.len() && !ended {
                        if pending.is_none() {
                            match receiver.try_recv() {
                                Ok(Some(block)) => {
                                    pending = Some(block);
                                    cursor = 0;
                                }
                                Ok(None) | Err(TryRecvError::Disconnected) => {
                                    ended = true;
                                    break;
                                }
                                Err(TryRecvError::Empty) => break,
                            }
                        }
                        let Some(block) = &pending else { break };
                        let count = (buffer.len() - written).min(block.samples.len() - cursor);
                        buffer[written..written + count]
                            .copy_from_slice(&block.samples[cursor..cursor + count]);
                        written += count;
                        cursor += count;
                        let position = Duration::from_micros(block.timestamp.0.max(0) as u64)
                            + Duration::from_secs_f64((cursor / channels) as f64 / f64::from(rate));
                        let _ = events.try_send(OutputEvent::Position(position));
                        if cursor == block.samples.len() {
                            pending = None;
                        }
                    }
                    buffer[written..].fill(0.0);
                    if ended && !reported_end {
                        let time = info.timestamp();
                        let latency = time
                            .playback
                            .duration_since(&time.callback)
                            .unwrap_or_default();
                        let tail = Duration::from_secs_f64(
                            written as f64 / channels as f64 / f64::from(rate),
                        );
                        reported_end = events
                            .try_send(OutputEvent::Finished(latency + tail))
                            .is_ok();
                    }
                },
                move |error| {
                    let _ = errors.try_send(error);
                },
                None,
            )
            .context("creating audio output stream")?;
        Ok(Self {
            format,
            samples,
            events: event_receiver,
            errors: error_receiver,
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
}

pub fn validate_samples(samples: &AudioSamples, format: &PcmFormat) -> Result<()> {
    if samples.format != *format
        || samples.samples.len() != samples.frame_count * format.channel_layout.len()
    {
        bail!("decoded audio does not match output configuration");
    }
    Ok(())
}
