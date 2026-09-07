//! FFmpeg decoding with CPAL output: cargo mp3 -- example.mp3
use anyhow::{Context, Result, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ffmpeg_next as ffmpeg;
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel};
use std::{
    path::Path,
    process::ExitCode,
    time::{Duration, Instant},
};

/// Formats a message with the source location required by the repository guidelines.
macro_rules! at {
    ($($arg:tt)*) => { format!("{} at {}:{}", format_args!($($arg)*), file!(), line!()) };
}

const OUTPUT: ffmpeg::format::Sample =
    ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed);

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1);
    let Some(path) = args.next() else {
        bail!("{}", at!("Usage: cargo mp3 -- <file.mp3>"));
    };
    if path == "--help" || path == "-h" {
        println!(
            "Usage: cargo mp3 -- <file.mp3>\nPlays through the default audio device. Ctrl-C stops playback."
        );
        return Ok(());
    }
    if args.next().is_some() {
        bail!("{}", at!("Expected one file"));
    }
    let path = Path::new(&path);

    ffmpeg::init().context(at!("Initializing FFmpeg"))?;
    let mut input = ffmpeg::format::input(path).context(at!("Opening {}", path.display()))?;
    let Some(audio) = input.streams().best(ffmpeg::media::Type::Audio) else {
        bail!("{}", at!("No audio stream in {}", path.display()));
    };
    let index = audio.index();
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(audio.parameters())
        .context(at!("Reading codec parameters"))?
        .decoder()
        .audio()
        .context(at!("Opening audio decoder"))?;

    let Some(device) = cpal::default_host().default_output_device() else {
        bail!("{}", at!("No default audio output device"));
    };
    let config = f32_config(&device)?;
    let (channels, rate) = (config.channels, config.sample_rate);
    let mut resampler = ffmpeg::software::resampling::Context::get(
        decoder.format(),
        layout(decoder.channels(), decoder.channel_layout()),
        decoder.rate(),
        OUTPUT,
        ffmpeg::ChannelLayout::default(i32::from(channels)),
        rate,
    )
    .context(at!("Creating resampler"))?;

    // Roughly half a second of buffering, handed over in decoded blocks.
    let (samples_tx, samples_rx) = sync_channel::<Vec<f32>>(64);
    let (events_tx, events_rx) = sync_channel(4);
    let stream = output(&device, &config, samples_rx, events_tx)?;
    stream.play().context(at!("Starting audio"))?;
    println!("Playing {} (Ctrl-C to stop)", path.display());

    loop {
        let mut packet = ffmpeg::Packet::empty();
        match packet.read(&mut input) {
            Ok(()) if packet.stream() != index => continue,
            Ok(()) => decoder
                .send_packet(&packet)
                .context(at!("Sending audio packet"))?,
            Err(ffmpeg::Error::Eof) => {
                decoder.send_eof().context(at!("Draining decoder"))?;
                receive(
                    &mut decoder,
                    &mut resampler,
                    channels,
                    rate,
                    &samples_tx,
                    &events_rx,
                )?;
                break;
            }
            Err(error) => {
                return Err(anyhow::Error::new(error).context(at!("Reading packet")));
            }
        }
        receive(
            &mut decoder,
            &mut resampler,
            channels,
            rate,
            &samples_tx,
            &events_rx,
        )?;
    }
    loop {
        let mut frame = frame(4096, channels);
        let delay = resampler
            .flush(&mut frame)
            .context(at!("Draining resampler"))?;
        enqueue(&frame, &samples_tx, &events_rx)?;
        if delay.is_none() {
            break;
        }
    }

    drop(samples_tx);
    match events_rx
        .recv_timeout(Duration::from_secs(10))
        .context(at!("Waiting for audio completion"))?
    {
        Event::Finished(delay) => std::thread::sleep(delay),
        Event::Error(error) => bail!("{}", at!("Audio output failed: {error}")),
    }
    Ok(())
}

enum Event {
    Error(cpal::StreamError),
    Finished(Duration),
}

fn output(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Receiver<Vec<f32>>,
    events: SyncSender<Event>,
) -> Result<cpal::Stream> {
    let errors = events.clone();
    let channels = usize::from(config.channels);
    let rate = config.sample_rate;
    let mut finished = false;
    let mut pending: Vec<f32> = Vec::new();
    let mut cursor = 0;
    device
        .build_output_stream(
            config,
            move |buffer: &mut [f32], info: &cpal::OutputCallbackInfo| {
                let mut written = 0;
                while written < buffer.len() {
                    if cursor == pending.len() {
                        match samples.try_recv() {
                            Ok(block) => {
                                pending = block;
                                cursor = 0;
                            }
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => {
                                if !finished {
                                    finished = true;
                                    let timestamp = info.timestamp();
                                    let latency = timestamp
                                        .playback
                                        .duration_since(&timestamp.callback)
                                        .unwrap_or_default();
                                    let buffered = Duration::from_secs_f64(
                                        buffer.len() as f64 / channels as f64 / rate as f64,
                                    );
                                    let _ = events.try_send(Event::Finished(latency + buffered));
                                }
                                break;
                            }
                        }
                    }
                    let count = (buffer.len() - written).min(pending.len() - cursor);
                    buffer[written..written + count]
                        .copy_from_slice(&pending[cursor..cursor + count]);
                    written += count;
                    cursor += count;
                }
                buffer[written..].fill(0.0);
            },
            move |error| {
                let _ = errors.try_send(Event::Error(error));
            },
            None,
        )
        .context(at!("Creating audio stream"))
}

fn receive(
    decoder: &mut ffmpeg::decoder::Audio,
    resampler: &mut ffmpeg::software::resampling::Context,
    channels: u16,
    rate: u32,
    samples: &SyncSender<Vec<f32>>,
    events: &Receiver<Event>,
) -> Result<()> {
    loop {
        let mut decoded = ffmpeg::frame::Audio::empty();
        match decoder.receive_frame(&mut decoded) {
            Ok(()) => {}
            Err(ffmpeg::Error::Eof) => return Ok(()),
            Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => return Ok(()),
            Err(error) => return Err(anyhow::Error::new(error).context(at!("Decoding audio"))),
        }
        let decoded_layout = layout(decoder.channels(), decoded.channel_layout());
        decoded.set_channel_layout(decoded_layout);
        // Reserve enough output for upsampling plus the filter's delayed samples.
        let capacity = (decoded.samples() as u64 * u64::from(rate))
            .div_ceil(u64::from(decoded.rate())) as usize
            + 256;
        let mut converted = frame(capacity, channels);
        resampler
            .run(&decoded, &mut converted)
            .context(at!("Resampling audio"))?;
        enqueue(&converted, samples, events)?;
    }
}

fn enqueue(
    frame: &ffmpeg::frame::Audio,
    samples: &SyncSender<Vec<f32>>,
    events: &Receiver<Event>,
) -> Result<()> {
    // `frame.data(0)` includes FFmpeg's alignment padding, so trim to the real samples.
    let bytes = frame.samples() * usize::from(frame.channels()) * size_of::<f32>();
    if bytes == 0 {
        return Ok(());
    }
    let mut block: Vec<f32> = frame.data(0)[..bytes]
        .chunks_exact(size_of::<f32>())
        .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Ok(Event::Error(error)) = events.try_recv() {
            bail!("{}", at!("Audio output failed: {error}"));
        }
        match samples.try_send(block) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Disconnected(_)) => bail!("{}", at!("Audio stream disconnected")),
            Err(TrySendError::Full(returned)) => {
                if Instant::now() >= deadline {
                    bail!("{}", at!("Audio device stopped consuming samples"));
                }
                block = returned;
                // Only the decoder waits; the audio callback always uses try_recv.
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
}

/// Picks an f32 output configuration, since every sample is decoded as f32.
fn f32_config(device: &cpal::Device) -> Result<cpal::StreamConfig> {
    let default = device
        .default_output_config()
        .context(at!("Reading audio configuration"))?;
    if default.sample_format() == cpal::SampleFormat::F32 {
        return Ok(default.config());
    }
    let mut supported = device
        .supported_output_configs()
        .context(at!("Listing audio configurations"))?;
    let Some(range) = supported.find(|range| range.sample_format() == cpal::SampleFormat::F32)
    else {
        bail!("{}", at!("No f32 audio output configuration"));
    };
    Ok(range.with_max_sample_rate().config())
}

fn frame(samples: usize, channels: u16) -> ffmpeg::frame::Audio {
    ffmpeg::frame::Audio::new(
        OUTPUT,
        samples,
        ffmpeg::ChannelLayout::default(i32::from(channels)),
    )
}

fn layout(channels: u16, declared: ffmpeg::ChannelLayout) -> ffmpeg::ChannelLayout {
    if declared.is_empty() {
        ffmpeg::ChannelLayout::default(i32::from(channels))
    } else {
        declared
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queues_interleaved_samples_without_ffmpeg_padding() {
        let mut audio = ffmpeg::frame::Audio::new(OUTPUT, 3, ffmpeg::ChannelLayout::STEREO);
        audio
            .plane_mut::<(f32, f32)>(0)
            .copy_from_slice(&[(0.1, -0.1), (0.2, -0.2), (0.3, -0.3)]);
        let (tx, rx) = sync_channel(1);
        let (_event_tx, events) = sync_channel(1);
        enqueue(&audio, &tx, &events).unwrap();
        assert_eq!(
            rx.try_recv().unwrap(),
            vec![0.1, -0.1, 0.2, -0.2, 0.3, -0.3]
        );
    }

    #[test]
    fn disconnected_output_stops_decoding() {
        let audio = ffmpeg::frame::Audio::new(OUTPUT, 1, ffmpeg::ChannelLayout::MONO);
        let (tx, rx) = sync_channel(1);
        drop(rx);
        let (_event_tx, events) = sync_channel(1);
        assert!(enqueue(&audio, &tx, &events).is_err());
    }
}
