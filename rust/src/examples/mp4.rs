//! A small FFmpeg + CPAL + GPUI player: cargo mp4 -- path/to/video.mp4
//!
//! FFmpeg separates the file into audio/video packets and decodes both tracks.
//! CPAL consumes audio samples on its callback thread. GPUI stays on the main
//! thread and displays decoded pictures at their presentation timestamps (PTS).
//! Bounded queues keep decoding from loading the whole movie into memory.
//!
//! This learning example autoplays once; close the window to stop. It uses a
//! shared wall clock, not a production audio-device clock with drift correction.
//! Seeking, subtitles, rotation metadata, and playback controls are omitted.

use anyhow::{Context as _, Result, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ffmpeg_next as ffmpeg;
use gpui::{
    App, Bounds, Context, IntoElement, ObjectFit, Render, RenderImage, Window, WindowBounds,
    WindowOptions, div, img, prelude::*, px, rgb, size,
};
use std::{
    path::{Path, PathBuf},
    process::ExitCode,
    sync::{
        Arc,
        mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel},
    },
    time::{Duration, Instant},
};

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
        bail!("{}", at!("Usage: cargo mp4 -- <video-file>"));
    };
    if path == "--help" || path == "-h" {
        println!(
            "Usage: cargo mp4 -- <video-file>\nAutoplays video and audio. Close the window to stop."
        );
        return Ok(());
    }
    if args.next().is_some() {
        bail!("{}", at!("Expected one video file"));
    }
    let path = PathBuf::from(path)
        .canonicalize()
        .context(at!("Finding video file"))?;
    ffmpeg::init().context(at!("Initializing FFmpeg"))?;
    // Validate the video before opening a window so CLI mistakes fail immediately.
    let input = ffmpeg::format::input(&path).context(at!("Opening {}", path.display()))?;
    if input.streams().best(ffmpeg::media::Type::Video).is_none() {
        bail!("{}", at!("No video track in {}", path.display()));
    }
    drop(input);
    env_logger::init();
    gpui_platform::application().run(move |cx: &mut App| {
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(960.0), px(600.0)), cx);
        let title = path.display().to_string();
        let (tx, rx) = sync_channel(8);
        let window = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some(title.into()),
                    ..Default::default()
                }),
                focus: true,
                ..Default::default()
            },
            move |_, cx| {
                cx.new(|_| Player {
                    messages: rx,
                    pending: None,
                    image: None,
                    status: "Loading…".into(),
                    finished: false,
                })
            },
        );
        if let Err(error) = window {
            eprintln!("{}: {error:#}", at!("Creating video window"));
            cx.quit();
            return;
        }
        // FFmpeg's synchronous calls would freeze GPUI if run in render().
        // Closing the window drops rx; the worker then stops at its next send.
        if let Err(error) = std::thread::Builder::new()
            .name("mp4-decoder".into())
            .spawn(move || {
                if let Err(error) = decode(&path, &tx) {
                    let message = format!("{error:#}");
                    // The UI is the error-reporting boundary. A closed UI needs no report.
                    let _ = tx.send(Message::Error(message));
                }
            })
        {
            eprintln!("{}: {error}", at!("Starting video decoder"));
            cx.quit();
            return;
        }
        cx.activate(true);
    });
    Ok(())
}

struct Picture {
    at: Instant,
    image: Arc<RenderImage>,
}

enum Message {
    Picture(Picture),
    Finished,
    Error(String),
}

struct Player {
    messages: Receiver<Message>,
    pending: Option<Picture>,
    image: Option<Arc<RenderImage>>,
    status: String,
    finished: bool,
}

impl Render for Player {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        loop {
            if self.pending.is_none() {
                match self.messages.try_recv() {
                    Ok(Message::Picture(picture)) => self.pending = Some(picture),
                    Ok(Message::Finished) => {
                        self.finished = true;
                        self.status = "Finished".into();
                        break;
                    }
                    Ok(Message::Error(error)) => {
                        eprintln!("{error}");
                        self.status = error;
                        self.finished = true;
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        if !self.finished {
                            self.status = at!("Decoder stopped unexpectedly");
                            eprintln!("{}", self.status);
                        }
                        self.finished = true;
                        break;
                    }
                }
            }
            let Some(picture) = self.pending.as_ref() else {
                break;
            };
            if picture.at > Instant::now() {
                break;
            }
            let Some(picture) = self.pending.take() else {
                break;
            };
            // Adapted from video/video_element.rs: GPUI expects BGRA images.
            // Release old GPU images instead of accumulating every movie frame.
            if let Some(old) = self.image.replace(picture.image) {
                cx.drop_image(old, Some(window));
            }
            self.status.clear();
            // If rendering fell behind, consume past-due frames to catch up.
        }
        if !self.finished {
            window.request_animation_frame();
        }
        let mut content = div().size_full().bg(rgb(0)).flex().flex_col();
        if let Some(image) = &self.image {
            content = content.child(
                img(image.clone())
                    .w_full()
                    .flex_1()
                    .min_h_0()
                    .object_fit(ObjectFit::Contain),
            );
        }
        if !self.status.is_empty() {
            content = content.child(
                div()
                    .p_3()
                    .text_color(rgb(0xffffff))
                    .child(self.status.clone()),
            );
        }
        content
    }
}

// Everything in Audio stays on the decoder thread, including the CPAL stream.
struct Audio {
    index: usize,
    decoder: ffmpeg::decoder::Audio,
    resampler: ffmpeg::software::resampling::Context,
    channels: u16,
    rate: u32,
    samples: SyncSender<Vec<f32>>,
    events: Receiver<Event>,
    _stream: cpal::Stream,
}

fn decode(path: &Path, pictures: &SyncSender<Message>) -> Result<()> {
    let mut input = ffmpeg::format::input(path).context(at!("Opening video"))?;
    let Some(track) = input.streams().best(ffmpeg::media::Type::Video) else {
        bail!("{}", at!("No video track"));
    };
    let index = track.index();
    let time_base = f64::from(track.time_base());
    let start = track.start_time();
    let origin = if start == ffmpeg::ffi::AV_NOPTS_VALUE {
        0.0
    } else {
        start as f64 * time_base
    };
    let fps = f64::from(track.avg_frame_rate());
    let frame_duration = if fps.is_finite() && fps > 0.0 {
        1.0 / fps
    } else {
        1.0 / 30.0
    };
    let mut decoder = ffmpeg::codec::context::Context::from_parameters(track.parameters())
        .context(at!("Reading video parameters"))?
        .decoder()
        .video()
        .context(at!("Opening video decoder"))?;
    let mut scaler = ffmpeg::software::scaling::Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        ffmpeg::format::Pixel::BGRA,
        decoder.width(),
        decoder.height(),
        ffmpeg::software::scaling::Flags::BILINEAR,
    )
    .context(at!("Creating video color converter"))?;
    // Small startup lead allows queues to fill before pictures and sound begin.
    let clock = Instant::now() + Duration::from_millis(250);
    let mut audio = (|| {
        let Some(track) = input.streams().best(ffmpeg::media::Type::Audio) else {
            return Ok(None);
        };
        let index = track.index();
        let decoder = ffmpeg::codec::context::Context::from_parameters(track.parameters())
            .context(at!("Reading audio parameters"))?
            .decoder()
            .audio()
            .context(at!("Opening audio decoder"))?;
        let Some(device) = cpal::default_host().default_output_device() else {
            bail!("{}", at!("No default audio output device"));
        };
        let config = f32_config(&device)?;
        let (channels, rate) = (config.channels, config.sample_rate);
        let resampler = ffmpeg::software::resampling::Context::get(
            decoder.format(),
            layout(decoder.channels(), decoder.channel_layout()),
            decoder.rate(),
            OUTPUT,
            ffmpeg::ChannelLayout::default(i32::from(channels)),
            rate,
        )
        .context(at!("Creating audio resampler"))?;
        let (samples, receiver) = sync_channel(64);
        let (events_tx, events) = sync_channel(4);
        let audio_start = track.start_time();
        let offset = if audio_start == ffmpeg::ffi::AV_NOPTS_VALUE {
            0.0
        } else {
            (audio_start as f64 * f64::from(track.time_base()) - origin).max(0.0)
        };
        let stream = output(
            &device,
            &config,
            receiver,
            events_tx,
            clock + Duration::from_secs_f64(offset),
        )?;
        stream.play().context(at!("Starting audio"))?;
        Ok::<_, anyhow::Error>(Some(Audio {
            index,
            decoder,
            resampler,
            channels,
            rate,
            samples,
            events,
            _stream: stream,
        }))
    })()?;
    let mut next_time = 0.0;
    loop {
        let mut packet = ffmpeg::Packet::empty();
        match packet.read(&mut input) {
            Ok(()) => {}
            Err(ffmpeg::Error::Eof) => break,
            Err(error) => {
                return Err(anyhow::Error::new(error).context(at!("Reading media packet")));
            }
        }
        if packet.stream() == index {
            decoder
                .send_packet(&packet)
                .context(at!("Sending video packet"))?;
            receive_video(
                &mut decoder,
                &mut scaler,
                pictures,
                clock,
                time_base,
                origin,
                frame_duration,
                &mut next_time,
            )?;
        } else if let Some(audio) = &mut audio {
            if packet.stream() == audio.index {
                audio
                    .decoder
                    .send_packet(&packet)
                    .context(at!("Sending audio packet"))?;
                receive(
                    &mut audio.decoder,
                    &mut audio.resampler,
                    audio.channels,
                    audio.rate,
                    &audio.samples,
                    &audio.events,
                )?;
            }
        }
    }
    decoder.send_eof().context(at!("Draining video decoder"))?;
    receive_video(
        &mut decoder,
        &mut scaler,
        pictures,
        clock,
        time_base,
        origin,
        frame_duration,
        &mut next_time,
    )?;
    if let Some(mut audio) = audio {
        audio
            .decoder
            .send_eof()
            .context(at!("Draining audio decoder"))?;
        receive(
            &mut audio.decoder,
            &mut audio.resampler,
            audio.channels,
            audio.rate,
            &audio.samples,
            &audio.events,
        )?;
        loop {
            let mut tail = frame(4096, audio.channels);
            let delay = audio
                .resampler
                .flush(&mut tail)
                .context(at!("Draining audio resampler"))?;
            enqueue(&tail, &audio.samples, &audio.events)?;
            if delay.is_none() {
                break;
            }
        }
        drop(audio.samples);
        match audio
            .events
            .recv_timeout(Duration::from_secs(10))
            .context(at!("Waiting for audio completion"))?
        {
            Event::Finished(delay) => std::thread::sleep(delay),
            Event::Error(error) => bail!("{}", at!("Audio output failed: {error}")),
        }
    }
    // Hold the last picture for its remaining duration, including silent files.
    std::thread::sleep(
        (clock + Duration::from_secs_f64(next_time)).saturating_duration_since(Instant::now()),
    );
    let _ = pictures.send(Message::Finished);
    Ok(())
}

fn receive_video(
    decoder: &mut ffmpeg::decoder::Video,
    scaler: &mut ffmpeg::software::scaling::Context,
    pictures: &SyncSender<Message>,
    clock: Instant,
    time_base: f64,
    origin: f64,
    frame_duration: f64,
    next_time: &mut f64,
) -> Result<()> {
    loop {
        let mut decoded = ffmpeg::frame::Video::empty();
        match decoder.receive_frame(&mut decoded) {
            Ok(()) => {}
            Err(ffmpeg::Error::Eof) => return Ok(()),
            Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => return Ok(()),
            Err(error) => {
                return Err(anyhow::Error::new(error).context(at!("Decoding video frame")));
            }
        }
        // PTS says WHEN a picture belongs on screen, independently of how fast
        // it decoded. Prefer timestamps over a fixed FPS, supporting variable FPS.
        let seconds = match decoded.timestamp() {
            Some(pts) => (pts as f64 * time_base - origin).max(0.0),
            None => *next_time,
        };
        *next_time = seconds + frame_duration;
        let mut bgra = ffmpeg::frame::Video::empty();
        scaler
            .run(&decoded, &mut bgra)
            .context(at!("Converting video to BGRA"))?;
        // Like pack_nv12 in the existing video element, copy row by row: FFmpeg
        // may pad each row, while the GPUI image needs tightly packed pixels.
        let row_bytes = bgra.width() as usize * 4;
        let mut bytes = Vec::with_capacity(row_bytes * bgra.height() as usize);
        for row in 0..bgra.height() as usize {
            let start = row * bgra.stride(0);
            bytes.extend_from_slice(&bgra.data(0)[start..start + row_bytes]);
        }
        let Some(image) = image::RgbaImage::from_raw(bgra.width(), bgra.height(), bytes) else {
            bail!("{}", at!("Invalid video image dimensions"));
        };
        // Despite image::RgbaImage's name, RenderImage requires BGRA byte order.
        let image = Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
            image
        )]));
        pictures
            .send(Message::Picture(Picture {
                at: clock + Duration::from_secs_f64(seconds),
                image,
            }))
            .context(at!("Video window closed"))?;
    }
}

// Audio helpers below are copied from mp3.rs. They convert decoded audio to
// packed f32 and hand it to CPAL without blocking its audio callback.
enum Event {
    Error(cpal::StreamError),
    Finished(Duration), // Estimated time until the last submitted sound plays.
}

fn output(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    samples: Receiver<Vec<f32>>,
    events: SyncSender<Event>,
    start: Instant,
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
                // Silence until the shared video/audio start time. The playback
                // timestamp includes the device's estimated output latency.
                let timestamp = info.timestamp();
                let latency = timestamp
                    .playback
                    .duration_since(&timestamp.callback)
                    .unwrap_or_default();
                let remaining = start.saturating_duration_since(Instant::now() + latency);
                let silence = ((remaining.as_secs_f64() * rate as f64).ceil() as usize)
                    .saturating_mul(channels)
                    .min(buffer.len());
                buffer[..silence].fill(0.0);
                let mut written = silence;
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
            None, // No explicit timeout override for CPAL's stream creation API.
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
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
}

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
    fn silent_video_preserves_timestamps_pixels_and_delayed_frames() -> Result<()> {
        // Width 66 makes FFmpeg's aligned row stride larger than the visible
        // pixel row. B-frames exercise decoder draining at end of file.
        let path =
            std::env::temp_dir().join(format!("opencut-mp4-test-{}.mp4", std::process::id()));
        let ffmpeg = Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/ffmpeg-8.1.2/bin/ffmpeg");
        let result = std::process::Command::new(ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "color=red:size=66x34:rate=25",
                "-frames:v",
                "10",
                "-c:v",
                "mpeg4",
                "-bf",
                "2",
                "-an",
                "-y",
            ])
            .arg(&path)
            .output()
            .context(at!("Generating test video"))?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        ffmpeg::init().context(at!("Initializing test decoder"))?;
        let (tx, rx) = sync_channel(2);
        let worker_path = path.clone();
        let worker = std::thread::spawn(move || decode(&worker_path, &tx));
        let mut times = Vec::new();
        loop {
            match rx
                .recv_timeout(Duration::from_secs(5))
                .context(at!("Receiving test picture"))?
            {
                Message::Picture(picture) => {
                    let bytes = picture.image.as_bytes(0).context(at!("Missing test image pixels"))?;
                    assert_eq!(bytes.len(), 66 * 34 * 4);
                    // RenderImage bytes are B, G, R, A: the red channel is third.
                    assert!(bytes[0] < 10 && bytes[1] < 10 && bytes[2] > 240);
                    times.push(picture.at);
                }
                Message::Finished => break,
                Message::Error(error) => bail!("{}", at!("Test playback failed: {error}")),
            }
        }
        worker.join().expect("test decoder panicked")?;
        std::fs::remove_file(path).context(at!("Removing test video"))?;
        assert_eq!(times.len(), 10);
        for pair in times.windows(2) {
            assert_eq!(pair[1].duration_since(pair[0]), Duration::from_millis(40));
        }
        Ok(())
    }
}
