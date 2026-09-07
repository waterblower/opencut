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
        bail!("Usage: cargo mp3 -- <file.mp3> at {}:{}", file!(), line!());
    };
    if path == "--help" || path == "-h" {
        println!(
            "Usage: cargo mp3 -- <file.mp3>\nPlays through the default audio device. Ctrl-C stops playback."
        );
        return Ok(());
    }
    if args.next().is_some() {
        bail!("Expected one file at {}:{}", file!(), line!());
    }
    let path = Path::new(&path);
    if !path.is_file() {
        bail!(
            "Not a local file: {} at {}:{}",
            path.display(),
            file!(),
            line!()
        );
    }
    ffmpeg::init().context(format!("Initializing FFmpeg at {}:{}", file!(), line!()))?;
    let mut input = ffmpeg::format::input(path).context(format!(
        "Opening {} at {}:{}",
        path.display(),
        file!(),
        line!()
    ))?;
    let Some(audio) = input.streams().best(ffmpeg::media::Type::Audio) else {
        bail!("No audio stream at {}:{}", file!(), line!());
    };
    let index = audio.index();
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(audio.parameters())
        .context(format!(
            "Reading codec parameters at {}:{}",
            file!(),
            line!()
        ))?
        .decoder()
        .audio()
        .context(format!("Opening audio decoder at {}:{}", file!(), line!()))?;
    let Some(device) = cpal::default_host().default_output_device() else {
        bail!("No default audio output device at {}:{}", file!(), line!());
    };
    let supported = device.default_output_config().context(format!(
        "Reading audio configuration at {}:{}",
        file!(),
        line!()
    ))?;
    let config = supported.config();
    let (samples_tx, samples_rx) =
        sync_channel(config.sample_rate as usize * config.channels as usize / 2);
    let (events_tx, events_rx) = sync_channel(4);
    let stream = match supported.sample_format() {
        cpal::SampleFormat::F32 => output::<f32>(&device, &config, samples_rx, events_tx),
        cpal::SampleFormat::F64 => output::<f64>(&device, &config, samples_rx, events_tx),
        cpal::SampleFormat::I8 => output::<i8>(&device, &config, samples_rx, events_tx),
        cpal::SampleFormat::I16 => output::<i16>(&device, &config, samples_rx, events_tx),
        cpal::SampleFormat::I24 => output::<cpal::I24>(&device, &config, samples_rx, events_tx),
        cpal::SampleFormat::I32 => output::<i32>(&device, &config, samples_rx, events_tx),
        cpal::SampleFormat::I64 => output::<i64>(&device, &config, samples_rx, events_tx),
        cpal::SampleFormat::U8 => output::<u8>(&device, &config, samples_rx, events_tx),
        cpal::SampleFormat::U16 => output::<u16>(&device, &config, samples_rx, events_tx),
        cpal::SampleFormat::U32 => output::<u32>(&device, &config, samples_rx, events_tx),
        cpal::SampleFormat::U64 => output::<u64>(&device, &config, samples_rx, events_tx),
        format => bail!(
            "Unsupported output format {format} at {}:{}",
            file!(),
            line!()
        ),
    }?;
    stream
        .play()
        .context(format!("Starting audio at {}:{}", file!(), line!()))?;
    println!("Playing {} (Ctrl-C to stop)", path.display());
    let mut resampler = None;
    loop {
        let mut packet = ffmpeg::Packet::empty();
        match packet.read(&mut input) {
            Ok(()) => {
                if packet.stream() != index {
                    continue;
                }
                decoder.send_packet(&packet).context(format!(
                    "Sending audio packet at {}:{}",
                    file!(),
                    line!()
                ))?;
            }
            Err(ffmpeg::Error::Eof) => {
                decoder.send_eof().context(format!(
                    "Draining decoder at {}:{}",
                    file!(),
                    line!()
                ))?;
                receive(
                    &mut decoder,
                    &mut resampler,
                    &config,
                    &samples_tx,
                    &events_rx,
                )?;
                break;
            }
            Err(error) => {
                return Err(anyhow::Error::new(error).context(format!(
                    "Reading packet at {}:{}",
                    file!(),
                    line!()
                )));
            }
        }
        receive(
            &mut decoder,
            &mut resampler,
            &config,
            &samples_tx,
            &events_rx,
        )?;
    }
    if let Some(mut resampler) = resampler {
        loop {
            let mut frame = ffmpeg::frame::Audio::new(
                ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
                4096,
                ffmpeg::ChannelLayout::default(i32::from(config.channels)),
            );
            let delay = resampler.flush(&mut frame).context(format!(
                "Draining resampler at {}:{}",
                file!(),
                line!()
            ))?;
            enqueue(&frame, &samples_tx, &events_rx)?;
            if delay.is_none() {
                break;
            }
        }
    } else {
        bail!("No audio frames decoded at {}:{}", file!(), line!());
    }
    drop(samples_tx);
    match events_rx
        .recv_timeout(Duration::from_secs(10))
        .context(format!(
            "Waiting for audio completion at {}:{}",
            file!(),
            line!()
        ))? {
        Event::Finished(delay) => std::thread::sleep(delay),
        Event::Error(error) => bail!("Audio output failed: {error} at {}:{}", file!(), line!()),
    }
    drop(stream);
    Ok(())
}

enum Event {
    Error(cpal::StreamError),
    Finished(Duration),
}

fn output<T: cpal::SizedSample + cpal::FromSample<f32>>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Receiver<f32>,
    events: SyncSender<Event>,
) -> Result<cpal::Stream> {
    let errors = events.clone();
    let channels = usize::from(config.channels);
    let rate = config.sample_rate;
    let mut finished = false;
    // Preserve channel alignment if the producer runs dry halfway through a frame.
    let mut pending = vec![0.0_f32; channels];
    let mut filled = 0;
    device
        .build_output_stream(
            config,
            move |buffer: &mut [T], info: &cpal::OutputCallbackInfo| {
                let buffer_duration =
                    Duration::from_secs_f64(buffer.len() as f64 / channels as f64 / rate as f64);
                for output_frame in buffer.chunks_mut(channels) {
                    while filled < channels {
                        match samples.try_recv() {
                            Ok(value) => {
                                pending[filled] = value;
                                filled += 1;
                            }
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => {
                                if !finished {
                                    let timestamp = info.timestamp();
                                    let delay = timestamp
                                        .playback
                                        .duration_since(&timestamp.callback)
                                        .unwrap_or_default();
                                    let _ =
                                        events.try_send(Event::Finished(delay + buffer_duration));
                                    finished = true;
                                }
                                break;
                            }
                        }
                    }
                    if filled == channels {
                        for (sample, value) in output_frame.iter_mut().zip(&pending) {
                            *sample = T::from_sample(*value);
                        }
                        filled = 0;
                    } else {
                        output_frame.fill(T::from_sample(0.0));
                    }
                }
            },
            move |error| {
                let _ = errors.try_send(Event::Error(error));
            },
            None,
        )
        .context(format!("Creating audio stream at {}:{}", file!(), line!()))
}

fn receive(
    decoder: &mut ffmpeg::decoder::Audio,
    resampler: &mut Option<ffmpeg::software::resampling::Context>,
    config: &cpal::StreamConfig,
    samples: &SyncSender<f32>,
    events: &Receiver<Event>,
) -> Result<()> {
    loop {
        let mut decoded = ffmpeg::frame::Audio::empty();
        match decoder.receive_frame(&mut decoded) {
            Ok(()) => {}
            Err(ffmpeg::Error::Eof) => return Ok(()),
            Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => return Ok(()),
            Err(error) => {
                return Err(anyhow::Error::new(error).context(format!(
                    "Decoding audio at {}:{}",
                    file!(),
                    line!()
                )));
            }
        }
        if decoded.channel_layout().is_empty() {
            decoded.set_channel_layout(ffmpeg::ChannelLayout::default(i32::from(
                decoder.channels(),
            )));
        }
        if resampler.is_none() {
            *resampler = Some(
                ffmpeg::software::resampling::Context::get(
                    decoded.format(),
                    decoded.channel_layout(),
                    decoded.rate(),
                    ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
                    ffmpeg::ChannelLayout::default(i32::from(config.channels)),
                    config.sample_rate,
                )
                .context(format!(
                    "Creating resampler at {}:{}",
                    file!(),
                    line!()
                ))?,
            );
        }
        let Some(resampler) = resampler else {
            unreachable!()
        };
        // Reserve enough output for upsampling plus the filter's delayed samples.
        let capacity = (decoded.samples() as u64 * u64::from(config.sample_rate))
            .div_ceil(u64::from(decoded.rate())) as usize
            + 256;
        let mut converted = ffmpeg::frame::Audio::new(
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
            capacity,
            ffmpeg::ChannelLayout::default(i32::from(config.channels)),
        );
        resampler.run(&decoded, &mut converted).context(format!(
            "Resampling audio at {}:{}",
            file!(),
            line!()
        ))?;
        enqueue(&converted, samples, events)?;
    }
}

fn enqueue(
    frame: &ffmpeg::frame::Audio,
    samples: &SyncSender<f32>,
    events: &Receiver<Event>,
) -> Result<()> {
    if frame.samples() == 0 {
        return Ok(());
    }
    let bytes = frame.samples() * usize::from(frame.channels()) * size_of::<f32>();
    for chunk in frame.data(0)[..bytes].chunks_exact(4) {
        let sample = f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        let mut blocked_since = None;
        loop {
            if let Ok(Event::Error(error)) = events.try_recv() {
                bail!("Audio output failed: {error} at {}:{}", file!(), line!());
            }
            match samples.try_send(sample) {
                Ok(()) => break,
                Err(TrySendError::Full(_)) => {
                    let started = blocked_since.get_or_insert(Instant::now());
                    if started.elapsed() >= Duration::from_secs(2) {
                        bail!(
                            "Audio device stopped consuming samples at {}:{}",
                            file!(),
                            line!()
                        );
                    }
                    // Only the decoder waits; the audio callback always uses try_recv.
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(TrySendError::Disconnected(_)) => {
                    bail!("Audio stream disconnected at {}:{}", file!(), line!());
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queues_interleaved_samples_without_ffmpeg_padding() {
        let mut frame = ffmpeg::frame::Audio::new(
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
            3,
            ffmpeg::ChannelLayout::STEREO,
        );
        frame
            .plane_mut::<(f32, f32)>(0)
            .copy_from_slice(&[(0.1, -0.1), (0.2, -0.2), (0.3, -0.3)]);
        let (tx, rx) = sync_channel(6);
        let (_event_tx, events) = sync_channel(1);
        enqueue(&frame, &tx, &events).unwrap();
        assert_eq!(
            rx.try_iter().collect::<Vec<_>>(),
            vec![0.1, -0.1, 0.2, -0.2, 0.3, -0.3]
        );
    }

    #[test]
    fn disconnected_output_stops_decoding() {
        let frame = ffmpeg::frame::Audio::new(
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
            1,
            ffmpeg::ChannelLayout::MONO,
        );
        let (tx, rx) = sync_channel(1);
        drop(rx);
        let (_event_tx, events) = sync_channel(1);
        assert!(enqueue(&frame, &tx, &events).is_err());
    }
}
