//! FFmpeg decoding with CPAL output: cargo mp3 -- example.mp3
//!
//! Follow the sound through this program:
//! file -> compressed packets -> decoded audio -> converted samples -> queue -> speakers.
//!
//! FFmpeg reads the file, decompresses its audio, and converts it to the format
//! the output device needs. CPAL talks to the operating system's audio device.
//! We call FFmpeg through Rust bindings; we do not launch the ffmpeg executable.
//!
//! A few words that are easy to confuse:
//! - A sample is one number describing the signal level for one channel at one
//!   instant. Floating-point audio typically uses -1.0 to 1.0; 0.0 is silence.
//! - A channel is one signal: mono has one, stereo has left and right.
//! - Sample rate counts samples PER CHANNEL per second. Stereo at 48,000 Hz
//!   therefore needs 96,000 f32 values per second.
//! - An FFmpeg audio frame holds a BLOCK of samples for all its channels.
//!   Its sample count is per channel, not the total number of stored values.
//! - A packet holds encoded data from the file; it is not ready for speakers.
//! - A file stream is one track in the file. A CPAL stream is a running audio
//!   connection to a device. They are different uses of the word "stream".
//!
//! Two execution paths work together: the main thread decodes ahead, while
//! CPAL repeatedly calls our callback to request the next buffer of sound.
//! Channels carry owned sample blocks forward and completion/errors backward.
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
// `.context(...)` adds an explanation to an error; `?` returns it to the caller.
// `bail!` creates an error and returns immediately. Only main prints the error.
macro_rules! at {
    ($($arg:tt)*) => { format!("{} at {}:{}", format_args!($($arg)*), file!(), line!()) };
}

// Packed (also called interleaved) stereo means [L0, R0, L1, R1, ...].
// Planar stereo instead stores [L0, L1, ...] and [R0, R1, ...] separately.
// CPAL wants interleaved values, so all decoded formats are converted to this.
const OUTPUT: ffmpeg::format::Sample =
    ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed);

fn main() -> ExitCode {
    // Keeping the actual work in run() lets it use `?` throughout the pipeline.
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    // The first argument is the executable name. args_os also supports paths
    // that are not valid UTF-8. The remaining argument must be the audio file.
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

    // Open the container/demuxer: the part of FFmpeg that separates a file into
    // tracks and packets. Even an audio file can contain another track, such
    // as cover art, so we explicitly select an audio track.
    ffmpeg::init().context(at!("Initializing FFmpeg"))?;
    let mut input = ffmpeg::format::input(path).context(at!("Opening {}", path.display()))?;
    let Some(audio) = input.streams().best(ffmpeg::media::Type::Audio) else {
        bail!("{}", at!("No audio stream in {}", path.display()));
    };
    // Remember which track's packets belong to our decoder. Codec parameters
    // describe how that track was encoded; opening the decoder prepares FFmpeg
    // to turn those compressed packets into uncompressed PCM sample blocks.
    let index = audio.index();
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(audio.parameters())
        .context(at!("Reading codec parameters"))?
        .decoder()
        .audio()
        .context(at!("Opening audio decoder"))?;

    // The host is the platform audio backend (CoreAudio on this Mac). Ask for
    // its default output device, then choose a format the device supports.
    let Some(device) = cpal::default_host().default_output_device() else {
        bail!("{}", at!("No default audio output device"));
    };
    let config = f32_config(&device)?;
    let (channels, rate) = (config.channels, config.sample_rate);
    // The first three arguments describe the decoded input; the last three
    // describe our desired output. "Resampling" here can change sample rate,
    // sample representation, AND channel layout (for example mono to stereo).
    // A 44,100 Hz file can thus play on a 48,000 Hz device at the correct speed.
    let mut resampler = ffmpeg::software::resampling::Context::get(
        decoder.format(),
        layout(decoder.channels(), decoder.channel_layout()),
        decoder.rate(),
        OUTPUT,
        ffmpeg::ChannelLayout::default(i32::from(channels)),
        rate,
    )
    .context(at!("Creating resampler"))?;

    // A bounded queue prevents decoding the entire file into memory ahead of
    // playback. Its capacity is 64 BLOCKS, not 64 samples or a fixed duration:
    // buffered time depends on block sizes and the output sample rate.
    // tx = sender, rx = receiver. Each send transfers ownership of a Vec.
    let (samples_tx, samples_rx) = sync_channel::<Vec<f32>>(64);
    // Messages travel the other way here: callback -> main thread.
    let (events_tx, events_rx) = sync_channel(4);
    let stream = output(&device, &config, samples_rx, events_tx)?;
    // Keep `stream` alive until playback ends; dropping it closes the output.
    // Playback starts before decoding, so the first callback may output silence.
    stream.play().context(at!("Starting audio"))?;
    println!("Playing {} (Ctrl-C to stop)", path.display());

    loop {
        // Reuse the same input reader position, but read a new packet each time.
        let mut packet = ffmpeg::Packet::empty();
        match packet.read(&mut input) {
            Ok(()) if packet.stream() != index => continue,
            // FFmpeg separates "send encoded input" from "receive decoded
            // output". A packet need not produce exactly one audio frame.
            Ok(()) => decoder
                .send_packet(&packet)
                .context(at!("Sending audio packet"))?,
            Err(ffmpeg::Error::Eof) => {
                // End of FILE does not mean the decoder has returned all its
                // sound. Signal that no more packets are coming, then retrieve
                // any frames it was keeping internally (called draining).
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
        // Take every frame currently available before sending the next packet.
        receive(
            &mut decoder,
            &mut resampler,
            channels,
            rate,
            &samples_tx,
            &events_rx,
        )?;
    }
    // The resampler can also retain samples for its conversion filter. Flush
    // those after draining the decoder so the end of the audio is not lost.
    loop {
        let mut frame = frame(4096, channels);
        let delay = resampler
            .flush(&mut frame)
            .context(at!("Draining resampler"))?;
        enqueue(&frame, &samples_tx, &events_rx)?;
        // None means no delayed output remains. Still enqueue this call's
        // output first: the final flush can produce useful samples.
        if delay.is_none() {
            break;
        }
    }

    // Closing the only sample sender is our end-of-audio message. The callback
    // can consume queued blocks first; only then does it see Disconnected.
    drop(samples_tx);
    // Handing samples to CPAL is earlier than hearing them. Wait for the queue
    // to finish, then allow the final device buffer time to reach the speakers.
    match events_rx
        .recv_timeout(Duration::from_secs(10))
        .context(at!("Waiting for audio completion"))?
    {
        Event::Finished(delay) => std::thread::sleep(delay),
        Event::Error(error) => bail!("{}", at!("Audio output failed: {error}")),
    }
    Ok(())
}

// These messages let the callback report back without printing or waiting.
enum Event {
    Error(cpal::StreamError),
    Finished(Duration), // Estimated time until the last submitted sound plays.
}

/// Build the device connection and the callback that fills its audio buffers.
/// File reading and decoding happen elsewhere; this callback just copies samples.
fn output(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Receiver<Vec<f32>>,
    events: SyncSender<Event>,
) -> Result<cpal::Stream> {
    // CPAL takes separate data and error callbacks; each needs an event sender.
    let errors = events.clone();
    let channels = usize::from(config.channels);
    let rate = config.sample_rate;
    // `move` below gives the callback ownership of these values. They survive
    // across calls. Device buffer sizes and decoded block sizes need not match,
    // so keep a partly consumed block and a cursor into it for the next call.
    let mut finished = false;
    let mut pending: Vec<f32> = Vec::new();
    let mut cursor = 0;
    device
        .build_output_stream(
            config,
            move |buffer: &mut [f32], info: &cpal::OutputCallbackInfo| {
                // CPAL lends us this buffer to fill before its playback deadline.
                // Waiting for disk I/O or channel data here could cause glitches.
                let mut written = 0;
                while written < buffer.len() {
                    if cursor == pending.len() {
                        // try_recv returns immediately, unlike blocking recv.
                        match samples.try_recv() {
                            Ok(block) => {
                                pending = block;
                                cursor = 0;
                            }
                            // No block YET: output silence for the rest of this
                            // callback, and try again on the next callback.
                            Err(TryRecvError::Empty) => break,
                            Err(TryRecvError::Disconnected) => {
                                // No blocks remain and no sender exists: done.
                                // Report completion once, even if CPAL calls us
                                // again while the main thread is waiting.
                                if !finished {
                                    finished = true;
                                    let timestamp = info.timestamp();
                                    // Playback timestamp estimates when this
                                    // buffer begins sounding; callback timestamp
                                    // describes when the callback was invoked.
                                    let latency = timestamp
                                        .playback
                                        .duration_since(&timestamp.callback)
                                        .unwrap_or_default();
                                    // values / channels / samples-per-second
                                    // gives seconds of sound in this buffer.
                                    // Using the whole buffer is conservative if
                                    // the last real sample comes before its end.
                                    let buffered = Duration::from_secs_f64(
                                        buffer.len() as f64 / channels as f64 / rate as f64,
                                    );
                                    // Best-effort notification: never block the
                                    // audio callback if the event queue is full.
                                    let _ = events.try_send(Event::Finished(latency + buffered));
                                }
                                break;
                            }
                        }
                    }
                    // Copy only what fits in BOTH the destination buffer and
                    // the current sample block. A callback may consume several
                    // blocks, or only part of one block.
                    let count = (buffer.len() - written).min(pending.len() - cursor);
                    buffer[written..written + count]
                        .copy_from_slice(&pending[cursor..cursor + count]);
                    written += count;
                    cursor += count;
                }
                // Always initialize the unused output; zero is silent audio.
                buffer[written..].fill(0.0);
            },
            move |error| {
                let _ = errors.try_send(Event::Error(error));
            },
            None, // No explicit timeout override for CPAL's stream creation API.
        )
        .context(at!("Creating audio stream"))
}

/// Pull all currently available decoded frames, convert them, and queue them.
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
            // Decoder EOF: it has finished draining, not merely run out of
            // output for the moment.
            Err(ffmpeg::Error::Eof) => return Ok(()),
            // EAGAIN means "send more input before asking for more output".
            // This is normal flow control, not a damaged-file error.
            Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => return Ok(()),
            Err(error) => return Err(anyhow::Error::new(error).context(at!("Decoding audio"))),
        }
        // Some files omit speaker positions. Supply a conventional layout
        // based on channel count so the resampler has a usable description.
        let decoded_layout = layout(decoder.channels(), decoded.channel_layout());
        decoded.set_channel_layout(decoded_layout);
        // Estimate output samples PER CHANNEL: input count * output/input rate.
        // Round upward because fractional samples still need storage. The extra
        // 256 is this example's fixed allowance for delayed filter output, not
        // a universal bound for every possible resampling configuration.
        let capacity = (decoded.samples() as u64 * u64::from(rate))
            .div_ceil(u64::from(decoded.rate())) as usize
            + 256;
        let mut converted = frame(capacity, channels);
        // Conversion fills the allocated frame and updates its sample count
        // to describe the output actually produced, which can vary per call.
        resampler
            .run(&decoded, &mut converted)
            .context(at!("Resampling audio"))?;
        enqueue(&converted, samples, events)?;
    }
}

/// Copy an FFmpeg frame into a Rust-owned block and send it toward playback.
/// This runs on the decoding thread, where short waits are acceptable.
fn enqueue(
    frame: &ffmpeg::frame::Audio,
    samples: &SyncSender<Vec<f32>>,
    events: &Receiver<Event>,
) -> Result<()> {
    // Packed audio puts all channels in plane 0. FFmpeg may add alignment bytes
    // after its useful data; those are not sound and must not enter the queue.
    // Example: 100 stereo samples/channel * 2 channels * 4 bytes/f32 = 800 bytes.
    let bytes = frame.samples() * usize::from(frame.channels()) * size_of::<f32>();
    if bytes == 0 {
        return Ok(());
    }
    // Read each native-endian group of four bytes as an f32. This is a copy,
    // not decompression: FFmpeg already produced floating-point PCM. Owning
    // the Vec lets the audio callback use it after this FFmpeg frame is dropped.
    let mut block: Vec<f32> = frame.data(0)[..bytes]
        .chunks_exact(size_of::<f32>())
        .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect();
    // A full queue means the decoder is ahead of playback (backpressure).
    // Retry while checking device errors, but do not wait forever if playback
    // stops. This timeout measures enqueue waiting, not the file's duration.
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        if let Ok(Event::Error(error)) = events.try_recv() {
            bail!("{}", at!("Audio output failed: {error}"));
        }
        match samples.try_send(block) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Disconnected(_)) => bail!("{}", at!("Audio stream disconnected")),
            Err(TrySendError::Full(returned)) => {
                // A failed send gives ownership back, so we can retry the same
                // block without losing samples or making another audio copy.
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

/// Pick a device configuration accepting f32, our chosen converted output type.
/// The decoder's original sample type may be different.
fn f32_config(device: &cpal::Device) -> Result<cpal::StreamConfig> {
    let default = device
        .default_output_config()
        .context(at!("Reading audio configuration"))?;
    if default.sample_format() == cpal::SampleFormat::F32 {
        return Ok(default.config());
    }
    // If the default format is integer audio, look for an f32 alternative.
    // This example chooses the first matching range's maximum rate; that is a
    // simple supported choice, not a claim that a higher rate improves the file.
    let mut supported = device
        .supported_output_configs()
        .context(at!("Listing audio configurations"))?;
    let Some(range) = supported.find(|range| range.sample_format() == cpal::SampleFormat::F32)
    else {
        bail!("{}", at!("No f32 audio output configuration"));
    };
    Ok(range.with_max_sample_rate().config())
}

/// Allocate packed f32 storage; `samples` is the capacity for EACH channel.
fn frame(samples: usize, channels: u16) -> ffmpeg::frame::Audio {
    ffmpeg::frame::Audio::new(
        OUTPUT,
        samples,
        ffmpeg::ChannelLayout::default(i32::from(channels)),
    )
}

/// A layout describes speaker roles, not just their count (e.g. left + right).
/// Preserve declared roles; use FFmpeg's conventional default when unspecified.
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
        // Three stereo instants should become exactly six values in L/R order,
        // regardless of any extra allocation padding inside the FFmpeg frame.
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
        // Dropping the receiver simulates playback going away. The producer
        // should report an error instead of continuing to queue sound forever.
        let audio = ffmpeg::frame::Audio::new(OUTPUT, 1, ffmpeg::ChannelLayout::MONO);
        let (tx, rx) = sync_channel(1);
        drop(rx);
        let (_event_tx, events) = sync_channel(1);
        assert!(enqueue(&audio, &tx, &events).is_err());
    }
}
