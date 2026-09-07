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
    App, Bounds, Context, IntoElement, Render, Window, WindowBounds, WindowOptions, div,
    prelude::*, px, rgb, size,
};
use std::{
    path::{Path, PathBuf},
    process::ExitCode,
    sync::mpsc::{Receiver, SyncSender, TryRecvError, TrySendError, sync_channel},
    time::{Duration, Instant},
};

#[cfg(target_os = "macos")]
use core_video::pixel_buffer::CVPixelBuffer;
#[cfg(not(target_os = "macos"))]
use gpui::{ObjectFit, RenderImage, img};
#[cfg(not(target_os = "macos"))]
use std::sync::Arc;

// Keep YUV on macOS: Metal converts it to RGB when drawing the video surface.
// Other platforms retain the portable GPUI image path.
#[cfg(target_os = "macos")]
type DecodedImage = ffmpeg::frame::Video;
#[cfg(not(target_os = "macos"))]
type DecodedImage = Arc<RenderImage>;
#[cfg(target_os = "macos")]
type DisplayImage = CVPixelBuffer;
#[cfg(not(target_os = "macos"))]
type DisplayImage = Arc<RenderImage>;

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
    image: DecodedImage,
}

enum Message {
    Picture(Picture),
    Finished,
    Error(String),
}

struct Player {
    messages: Receiver<Message>,
    pending: Option<Picture>,
    image: Option<DisplayImage>,
    status: String,
    finished: bool,
}

impl Render for Player {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
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
            #[cfg(target_os = "macos")]
            match video_surface(&picture.image) {
                Ok(surface) => self.image = Some(surface),
                Err(error) => {
                    self.status = format!("{error:#}");
                    eprintln!("{}", self.status);
                    self.finished = true;
                    break;
                }
            }
            #[cfg(not(target_os = "macos"))]
            if let Some(old) = self.image.replace(picture.image) {
                _cx.drop_image(old, Some(window));
            }
            self.status.clear();
            // If rendering fell behind, consume past-due frames to catch up.
        }
        if !self.finished {
            window.request_animation_frame();
        }
        let mut content = div().size_full().bg(rgb(0)).flex().flex_col();
        if let Some(image) = &self.image {
            #[cfg(target_os = "macos")]
            {
                let surface = image.clone();
                content = content.child(
                    gpui::canvas(
                        |_, _, _| (),
                        move |bounds, _, window, _| {
                            let width = surface.get_width() as f32;
                            let height = surface.get_height() as f32;
                            let scale = (f32::from(bounds.size.width) / width)
                                .min(f32::from(bounds.size.height) / height);
                            let fitted = size(px(width * scale), px(height * scale));
                            let origin = gpui::point(
                                bounds.origin.x + (bounds.size.width - fitted.width) / 2.0,
                                bounds.origin.y + (bounds.size.height - fitted.height) / 2.0,
                            );
                            window.paint_surface(Bounds::new(origin, fitted), surface);
                        },
                    )
                    .w_full()
                    .flex_1()
                    .min_h_0(),
                );
            }
            #[cfg(not(target_os = "macos"))]
            {
                content = content.child(
                    img(image.clone())
                        .w_full()
                        .flex_1()
                        .min_h_0()
                        .object_fit(ObjectFit::Contain),
                );
            }
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
    let mut context = ffmpeg::codec::context::Context::from_parameters(track.parameters())
        .context(at!("Reading video parameters"))?;
    // Ask the codec to distribute frame decoding across its available workers.
    // This must be configured BEFORE opening the decoder.
    context.set_threading(ffmpeg::codec::threading::Config::kind(
        ffmpeg::codec::threading::Type::Frame,
    ));
    let mut decoder = context
        .decoder()
        .video()
        .context(at!("Opening video decoder"))?;
    // Use FFmpeg 8's frame-based scaler so color range/matrix metadata is
    // applied before choosing a conversion path (including optimized YUV paths).
    // SAFETY: allocation has no preconditions; ownership moves into VideoConverter.
    let mut scaler = VideoConverter(unsafe { ffmpeg::ffi::sws_alloc_context() });
    if scaler.0.is_null() {
        bail!("{}", at!("Allocating video converter"));
    }
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
    scaler: &mut VideoConverter,
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
        let converted = scaler.convert(&decoded)?;
        #[cfg(target_os = "macos")]
        let image = converted;
        #[cfg(not(target_os = "macos"))]
        let image = {
            let row_bytes = converted.width() as usize * 4;
            let mut bytes = Vec::with_capacity(row_bytes * converted.height() as usize);
            for row in 0..converted.height() as usize {
                let start = row * converted.stride(0);
                bytes.extend_from_slice(&converted.data(0)[start..start + row_bytes]);
            }
            let Some(image) =
                image::RgbaImage::from_raw(converted.width(), converted.height(), bytes)
            else {
                bail!("{}", at!("Invalid video image dimensions"));
            };
            Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
                image
            )]))
        };
        pictures
            .send(Message::Picture(Picture {
                at: clock + Duration::from_secs_f64(seconds),
                image,
            }))
            .context(at!("Video window closed"))?;
    }
}

// Own the C context so every exit path releases FFmpeg's conversion resources.
struct VideoConverter(*mut ffmpeg::ffi::SwsContext);

impl Drop for VideoConverter {
    fn drop(&mut self) {
        // SAFETY: this wrapper exclusively owns the pointer, including null.
        unsafe {
            ffmpeg::ffi::sws_free_context(&mut self.0);
        }
    }
}

impl VideoConverter {
    fn convert(&mut self, decoded: &ffmpeg::frame::Video) -> Result<ffmpeg::frame::Video> {
        let mut converted = ffmpeg::frame::Video::empty();
        converted.set_width(decoded.width());
        converted.set_height(decoded.height());
        converted.set_color_primaries(decoded.color_primaries());
        converted.set_color_transfer_characteristic(decoded.color_transfer_characteristic());
        #[cfg(target_os = "macos")]
        {
            converted.set_format(ffmpeg::format::Pixel::NV12);
            // GPUI's surface shader currently uses full-range BT.601 YUV.
            converted.set_color_space(ffmpeg::color::Space::SMPTE170M);
            converted.set_color_range(ffmpeg::color::Range::JPEG);
        }
        #[cfg(not(target_os = "macos"))]
        {
            converted.set_format(ffmpeg::format::Pixel::BGRA);
            converted.set_color_space(ffmpeg::color::Space::RGB);
            converted.set_color_range(ffmpeg::color::Range::JPEG);
        }
        // SAFETY: both frames and the context remain alive for the call; output
        // is exclusively borrowed. FFmpeg allocates/refcounts its output planes.
        let result = unsafe {
            ffmpeg::ffi::sws_scale_frame(self.0, converted.as_mut_ptr(), decoded.as_ptr())
        };
        if result < 0 {
            bail!(
                "{}",
                at!("Converting video pixels: {}", ffmpeg::Error::from(result))
            );
        }
        Ok(converted)
    }
}

// Adapted from video/video_element.rs, but allocated only when a new picture
// is displayed, not on every UI repaint. IOSurface lets Metal sample these YUV
// planes directly without the generic image atlas or a 4K BGRA upload.
#[cfg(target_os = "macos")]
fn video_surface(frame: &ffmpeg::frame::Video) -> Result<CVPixelBuffer> {
    use core_foundation::{
        base::TCFType,
        boolean::CFBoolean,
        dictionary::{CFDictionary, CFMutableDictionary},
        string::CFString,
    };
    use core_video::{
        pixel_buffer::{CVPixelBufferKeys, kCVPixelFormatType_420YpCbCr8BiPlanarFullRange},
        r#return::kCVReturnSuccess,
    };
    let width = frame.width() as usize;
    let height = frame.height() as usize;
    let mut attributes = CFMutableDictionary::<CFString, core_foundation::base::CFType>::new();
    attributes.add(
        &CVPixelBufferKeys::MetalCompatibility.into(),
        &CFBoolean::true_value().as_CFType(),
    );
    let iosurface = CFDictionary::<CFString, core_foundation::base::CFType>::from_CFType_pairs(&[]);
    attributes.add(
        &CVPixelBufferKeys::IOSurfaceProperties.into(),
        &iosurface.as_CFType(),
    );
    let surface = match CVPixelBuffer::new(
        kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
        width,
        height,
        Some(&attributes.to_immutable()),
    ) {
        Ok(surface) => surface,
        Err(error) => bail!("{}", at!("Allocating video surface: {error}")),
    };
    if frame.format() != ffmpeg::format::Pixel::NV12 || surface.get_plane_count() != 2 {
        bail!("{}", at!("Expected a two-plane NV12 video surface"));
    }
    if surface.lock_base_address(0) != kCVReturnSuccess {
        bail!("{}", at!("Locking video surface"));
    }
    // Copy only visible row bytes. Both FFmpeg and CoreVideo can pad their
    // strides differently; the UV plane rounds up for odd image dimensions.
    let result = (|| {
        for (plane, rows, bytes) in [
            (0, height, width),
            (1, height.div_ceil(2), width.div_ceil(2) * 2),
        ] {
            let stride = surface.get_bytes_per_row_of_plane(plane);
            // SAFETY: the two-plane buffer is locked for CPU access above.
            let destination = unsafe { surface.get_base_address_of_plane(plane) as *mut u8 };
            if destination.is_null() || stride < bytes || surface.get_height_of_plane(plane) < rows
            {
                bail!("{}", at!("Invalid video surface plane"));
            }
            for row in 0..rows {
                let start = row * frame.stride(plane);
                let Some(source) = frame.data(plane).get(start..start + bytes) else {
                    bail!("{}", at!("Invalid decoded video plane"));
                };
                // SAFETY: surface is locked and exclusively owned here; its
                // checked stride/height cover the destination, and source is a
                // checked slice in a distinct FFmpeg-owned allocation.
                unsafe {
                    std::ptr::copy_nonoverlapping(
                        source.as_ptr(),
                        destination.add(row * stride),
                        bytes,
                    );
                }
            }
        }
        Ok(())
    })();
    let unlocked = surface.unlock_base_address(0);
    result?;
    if unlocked != kCVReturnSuccess {
        bail!("{}", at!("Unlocking video surface: {unlocked}"));
    }
    Ok(surface)
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
                    #[cfg(target_os = "macos")]
                    {
                        assert_eq!((picture.image.width(), picture.image.height()), (66, 34));
                        assert_eq!(picture.image.format(), ffmpeg::format::Pixel::NV12);
                        // Full-range BT.601 red is approximately Y=76,U=85,V=255.
                        assert!((70..85).contains(&picture.image.data(0)[0]));
                        assert!((75..95).contains(&picture.image.data(1)[0]));
                        assert!(
                            picture.image.data(1)[1] > 240,
                            "YUV: {}, {}, {}",
                            picture.image.data(0)[0],
                            picture.image.data(1)[0],
                            picture.image.data(1)[1]
                        );
                        let surface = video_surface(&picture.image)?;
                        assert_eq!((surface.get_width(), surface.get_height()), (66, 34));
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        let bytes = picture
                            .image
                            .as_bytes(0)
                            .context(at!("Missing test image pixels"))?;
                        assert_eq!(bytes.len(), 66 * 34 * 4);
                        assert!(bytes[0] < 10 && bytes[1] < 10 && bytes[2] > 240);
                    }
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

#[cfg(test)]
mod performance {
    use super::*;

    // Run explicitly with OPENCUT_VIDEO_BENCHMARK=/path/to/a/silent/video.mp4
    // cargo test --no-default-features --features mp4 --bin mp4 throughput -- --ignored --nocapture
    #[test]
    #[ignore = "requires a local silent video fixture"]
    fn throughput() -> Result<()> {
        let path = std::env::var_os("OPENCUT_VIDEO_BENCHMARK")
            .context(at!("Set OPENCUT_VIDEO_BENCHMARK to a silent video file"))?;
        ffmpeg::init().context(at!("Initializing benchmark decoder"))?;
        let (tx, rx) = sync_channel(8);
        let started = Instant::now();
        let worker = std::thread::spawn(move || decode(Path::new(&path), &tx));
        let mut count = 0;
        let mut last_frame = started;
        loop {
            match rx
                .recv_timeout(Duration::from_secs(30))
                .context(at!("Receiving benchmark video"))?
            {
                Message::Picture(picture) => {
                    #[cfg(target_os = "macos")]
                    let _surface = video_surface(&picture.image)?;
                    count += 1;
                    last_frame = Instant::now();
                }
                Message::Finished => break,
                Message::Error(error) => bail!("{}", at!("Benchmark decoder: {error}")),
            }
        }
        worker.join().expect("benchmark worker panicked")?;
        assert!(count > 0);
        println!(
            "Decoded and prepared {count} frames in {:.3}s ({:.1} frames/s); excludes GPU presentation",
            last_frame.duration_since(started).as_secs_f64(),
            count as f64 / last_frame.duration_since(started).as_secs_f64()
        );
        Ok(())
    }
}
