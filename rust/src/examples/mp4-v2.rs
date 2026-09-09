//! mp4.rs plus transport controls: cargo mp4-v2 -- path/to/video.mp4
//!
//! Space toggles pause, Left/Right step one frame in either direction without
//! limit, and a non-interactive progress bar tracks position. Everything else
//! works like mp4.rs: FFmpeg demuxes and decodes both tracks on a worker
//! thread, CPAL consumes audio on its callback thread, GPUI displays pictures
//! on the main thread, and bounded queues keep the movie out of memory.
//!
//! Two structural changes make the controls possible.
//!
//! First, WHERE time lives. mp4.rs baked a fixed wall clock into every
//! timestamp, which cannot pause or step. Here the decoder emits
//! media-relative seconds only, and a single shared `Clock` maps media time to
//! wall time. Pausing, resuming, and stepping just edit that mapping, so video
//! and audio follow together without talking to each other.
//!
//! Second, the decoder takes requests. Stepping forward is free because the
//! next picture is already queued, but stepping back needs a frame nobody kept,
//! so the UI sends a `Rewind` and the decoder seeks, decodes forward, and
//! resumes streaming from there. An epoch number tags every picture so the UI
//! can discard the ones already in flight when a rewind is issued.
//!
//! Subtitles, rotation metadata, and scrubbing are still omitted.

use anyhow::{Context as _, Result, bail};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ffmpeg_next as ffmpeg;
use gpui::{
    App, Bounds, Context, FocusHandle, IntoElement, KeyDownEvent, Render, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, relative, rgb, size,
};
use std::{
    path::{Path, PathBuf},
    process::ExitCode,
    sync::{
        Arc, Mutex, MutexGuard,
        mpsc::{Receiver, RecvTimeoutError, SyncSender, TrySendError, sync_channel},
    },
    time::{Duration, Instant},
};

#[cfg(target_os = "macos")]
use core_video::pixel_buffer::CVPixelBuffer;
#[cfg(not(target_os = "macos"))]
use gpui::{ObjectFit, RenderImage, img};

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

// Startup lead: media time 0 is this far in the future, so the queues fill
// before the first picture and the first sample are due.
const PREROLL: Duration = Duration::from_millis(250);

// How far audio may drift from the clock before it is forcibly realigned.
// Steady playback stays well inside this, so ordinary output is bit-exact and
// click-free; only a pause, resume, or frame step crosses it.
const RESYNC: f64 = 0.05;

// Timestamps are floats, so "strictly before" needs a margin well under one
// frame to avoid a backward step landing on the frame it started from.
const EPSILON: f64 = 1e-4;

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
        bail!("{}", at!("Usage: cargo mp4-v2 -- <video-file>"));
    };
    if path == "--help" || path == "-h" {
        println!(
            "Usage: cargo mp4-v2 -- <video-file>\n\
             Space: pause/resume, Left/Right: step one frame. Close the window to stop."
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
        let (tx, messages) = sync_channel(8);
        let (commands, rx) = sync_channel(4);
        // The UI thread writes this clock and the audio callback reads it.
        // Nothing else is shared, so pause and step stay a single edit.
        let timeline = Arc::new(Mutex::new(Clock::started()));
        let shared = timeline.clone();
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
            move |window, cx| {
                // Keyboard transport needs focus on the element that renders.
                let focus = cx.focus_handle();
                window.focus(&focus, cx);
                cx.new(|_| Player {
                    messages,
                    commands,
                    timeline,
                    focus,
                    pending: None,
                    image: None,
                    shown: 0.0,
                    epoch: 0,
                    displayed: 0,
                    duration: 0.0,
                    status: "Loading…".into(),
                    ended: false,
                    failed: false,
                })
            },
        );
        if let Err(error) = window {
            eprintln!("{}: {error:#}", at!("Creating video window"));
            cx.quit();
            return;
        }
        // FFmpeg's synchronous calls would freeze GPUI if run in render().
        // Closing the window drops the receiver; the worker then stops at its
        // next send.
        if let Err(error) = std::thread::Builder::new()
            .name("mp4-decoder".into())
            .spawn(move || {
                if let Err(error) = decode(&path, &tx, &rx, &shared) {
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

/// Maps media time to wall time for every thread that needs to agree on
/// "now". While playing, media time is `Instant::now() - origin`, which is
/// negative during the pre-roll. While paused, it is frozen at `paused`.
#[derive(Clone, Copy)]
struct Clock {
    origin: Instant,
    paused: Option<f64>,
}

impl Clock {
    fn started() -> Self {
        Clock {
            origin: Instant::now() + PREROLL,
            paused: None,
        }
    }

    fn time(&self, now: Instant) -> f64 {
        if let Some(time) = self.paused {
            return time;
        }
        if now >= self.origin {
            now.duration_since(self.origin).as_secs_f64()
        } else {
            -self.origin.duration_since(now).as_secs_f64()
        }
    }

    fn pause(&mut self, now: Instant) {
        self.paused = Some(self.time(now));
    }

    fn resume(&mut self, now: Instant) {
        let Some(time) = self.paused.take() else {
            return;
        };
        let elapsed = Duration::from_secs_f64(time.abs());
        // Instant arithmetic can be unrepresentable near the monotonic epoch.
        let origin = if time >= 0.0 {
            now.checked_sub(elapsed)
        } else {
            now.checked_add(elapsed)
        };
        self.origin = origin.unwrap_or(now);
    }

    /// Moves to an exact media time, keeping the current play/pause state so a
    /// step that lands while playing does not silently pause the player.
    fn seek(&mut self, time: f64, now: Instant) {
        let playing = self.paused.is_none();
        self.paused = Some(time);
        if playing {
            self.resume(now);
        }
    }
}

struct Picture {
    /// Which decoder epoch produced this. Pictures queued before a rewind
    /// carry the old value and are discarded on arrival.
    epoch: u64,
    time: f64,
    image: DecodedImage,
}

enum Message {
    Duration(f64),
    Picture(Picture),
    EndOfStream,
    Error(String),
}

/// A request for the last frame strictly before `before`, which is the only
/// thing backward stepping cannot answer from data already in flight.
struct Rewind {
    epoch: u64,
    before: f64,
}

struct Player {
    messages: Receiver<Message>,
    commands: SyncSender<Rewind>,
    timeline: Arc<Mutex<Clock>>,
    focus: FocusHandle,
    pending: Option<Picture>,
    image: Option<DisplayImage>,
    shown: f64,
    /// Latest epoch requested, versus the epoch actually on screen. They differ
    /// exactly while a rewind is outstanding.
    epoch: u64,
    displayed: u64,
    duration: f64,
    status: String,
    ended: bool,
    failed: bool,
}

impl Render for Player {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let clock = *lock(&self.timeline);
        let time = clock.time(Instant::now());
        self.advance(time, window, cx);
        // Nothing changes while paused, and nothing changes once the last
        // picture is on screen. Both stop the animation loop; a keypress
        // notifies the view instead. A pending rewind is the exception: its
        // answer arrives asynchronously and has to be picked up.
        let settled = self.ended
            && self.pending.is_none()
            && time >= self.duration
            && self.epoch == self.displayed;
        let waiting = self.epoch != self.displayed;
        if !self.failed && (clock.paused.is_none() || waiting) && !settled {
            window.request_animation_frame();
        }

        let mut content = div()
            .track_focus(&self.focus)
            .key_context("Mp4Player")
            .on_key_down(cx.listener(|player, event: &KeyDownEvent, window, cx| {
                match event.keystroke.key.as_str() {
                    "space" => player.toggle(),
                    "right" => player.step_forward(window, cx),
                    "left" => player.step_backward(),
                    _ => return,
                }
                cx.notify();
            }))
            .size_full()
            .bg(rgb(0))
            .flex()
            .flex_col();
        if let Some(image) = &self.image {
            #[cfg(target_os = "macos")]
            {
                let surface = image.clone();
                content = content.child(
                    gpui::canvas(
                        |_, _, _| (),
                        move |bounds, _, window, cx| {
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
                            // Count the actual video canvas paint callback, not
                            // decoded frames or calls to Player::render.
                            log_video_paint_fps(window, cx);
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
        let progress = if self.duration > 0.0 {
            (time / self.duration).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        let hint = if waiting {
            "Stepping back…"
        } else if clock.paused.is_some() {
            "Paused · Space play · ←/→ step"
        } else {
            "Playing · Space pause · ←/→ step"
        };
        content = content.child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .px_4()
                .py_3()
                .child(
                    // Display only: the bar reports position and never seeks.
                    div()
                        .w_full()
                        .h(px(4.0))
                        .rounded_full()
                        .bg(rgb(0x2a2a2a))
                        .child(
                            div()
                                .h_full()
                                .w(relative(progress))
                                .rounded_full()
                                .bg(rgb(0x4ea1ff)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .text_xs()
                        .text_color(rgb(0x9a9a9a))
                        .child(format!("{} / {}", timecode(time), timecode(self.duration)))
                        .child(hint),
                ),
        );
        if !self.status.is_empty() {
            content = content.child(
                div()
                    .px_4()
                    .pb_3()
                    .text_color(rgb(0xffffff))
                    .child(self.status.clone()),
            );
        }
        content
    }
}

impl Player {
    /// Brings the display up to `time`. Past-due pictures are consumed in one
    /// pass so a slow repaint catches up rather than falling further behind.
    fn advance(&mut self, time: f64, window: &mut Window, cx: &mut Context<Self>) {
        while !self.failed {
            if self.pending.is_none() && !self.receive(Duration::ZERO) {
                return;
            }
            let Some(picture) = self.pending.as_ref() else {
                return;
            };
            // A rewind answer is always behind the clock, so it shows at once.
            if picture.time > time {
                return;
            }
            let Some(picture) = self.pending.take() else {
                return;
            };
            self.show(picture, window, cx);
        }
    }

    /// Reads messages until a usable picture lands in `pending`, the channel
    /// runs dry, or the stream ends. `wait` is zero for the render loop and
    /// brief for a frame step, where blocking the UI momentarily beats a
    /// keypress doing nothing.
    fn receive(&mut self, wait: Duration) -> bool {
        let deadline = Instant::now() + wait;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            match self.messages.recv_timeout(remaining) {
                Ok(Message::Picture(picture)) => {
                    // Anything from before the last rewind is already wrong.
                    if picture.epoch != self.epoch {
                        continue;
                    }
                    self.pending = Some(picture);
                    self.status.clear();
                    return true;
                }
                Ok(Message::Duration(duration)) => self.duration = duration,
                Ok(Message::EndOfStream) => {
                    self.ended = true;
                    self.status.clear();
                    return false;
                }
                Ok(Message::Error(error)) => {
                    eprintln!("{error}");
                    self.status = error;
                    self.failed = true;
                    return false;
                }
                Err(RecvTimeoutError::Timeout) => return false,
                Err(RecvTimeoutError::Disconnected) => {
                    if !self.ended {
                        self.status = at!("Decoder stopped unexpectedly");
                        eprintln!("{}", self.status);
                    }
                    self.failed = true;
                    return false;
                }
            }
        }
    }

    fn show(&mut self, picture: Picture, _window: &mut Window, _cx: &mut Context<Self>) {
        #[cfg(target_os = "macos")]
        let image = match video_surface(&picture.image) {
            Ok(surface) => surface,
            Err(error) => {
                self.status = format!("{error:#}");
                eprintln!("{}", self.status);
                self.failed = true;
                return;
            }
        };
        #[cfg(not(target_os = "macos"))]
        let image = picture.image;
        #[cfg(not(target_os = "macos"))]
        if let Some(old) = self.image.replace(image) {
            _cx.drop_image(old, Some(_window));
        }
        #[cfg(target_os = "macos")]
        {
            self.image = Some(image);
        }
        self.shown = picture.time;
        // The first picture of a new epoch answers a rewind, so the clock
        // moves onto it instead of the other way round.
        if picture.epoch != self.displayed {
            self.displayed = picture.epoch;
            lock(&self.timeline).seek(picture.time, Instant::now());
        }
    }

    fn toggle(&mut self) {
        let now = Instant::now();
        let mut clock = lock(&self.timeline);
        if clock.paused.is_some() {
            clock.resume(now);
        } else {
            clock.pause(now);
        }
    }

    /// Stepping implies pausing, and the clock lands exactly on the shown
    /// frame so audio realigns to it on the next callback.
    fn step_forward(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        lock(&self.timeline).pause(Instant::now());
        if self.pending.is_none() && !self.receive(Duration::from_millis(50)) {
            return;
        }
        let Some(picture) = self.pending.take() else {
            return;
        };
        let time = picture.time;
        self.show(picture, window, cx);
        lock(&self.timeline).seek(time, Instant::now());
    }

    /// The previous frame was never kept, so ask the decoder to seek for it.
    /// Everything already in flight belongs to the old epoch and is discarded
    /// as it arrives, and `show` moves the clock when the answer appears.
    fn step_backward(&mut self) {
        lock(&self.timeline).pause(Instant::now());
        if self.shown <= 0.0 || self.epoch != self.displayed {
            return;
        }
        self.epoch += 1;
        self.pending = None;
        self.ended = false;
        let request = Rewind {
            epoch: self.epoch,
            before: self.shown,
        };
        if self.commands.try_send(request).is_err() {
            self.status = at!("Decoder stopped accepting requests");
            eprintln!("{}", self.status);
            self.failed = true;
        }
    }
}

// Store measurement history on the canvas itself. Updating it does not notify
// the view or request extra repaints, which would distort the measurement.
#[cfg(target_os = "macos")]
fn log_video_paint_fps(window: &mut Window, cx: &mut App) {
    let now = Instant::now();
    let stats = window.use_state(cx, |_, _| (now, 0_u64));
    stats.update(cx, |(started, paints), _| {
        *paints += 1;
        let elapsed = now.duration_since(*started).as_secs_f64();
        if elapsed < 1.0 {
            return;
        }
        // Repainting the same video frame counts as another paint. This measures
        // GPUI paint submissions, not GPU completion or physical display refresh.
        eprintln!(
            "GPUI video paint: {:.1} FPS ({} paints in {:.3}s; includes repeated frames)",
            *paints as f64 / elapsed,
            *paints,
            elapsed,
        );
        *started = now;
        *paints = 0;
    });
}

/// Per-file video timing facts, used to turn PTS into media seconds.
#[derive(Clone, Copy)]
struct Track {
    index: usize,
    time_base: f64,
    origin: f64,
    frame_duration: f64,
}

impl Track {
    /// PTS says WHEN a picture belongs on screen, independently of how fast it
    /// decoded. Preferring timestamps over a fixed FPS supports variable FPS.
    fn seconds(&self, pts: Option<i64>, fallback: f64) -> f64 {
        match pts {
            Some(pts) => (pts as f64 * self.time_base - self.origin).max(0.0),
            None => fallback,
        }
    }
}

/// Outbound pictures and inbound rewinds share one waiting loop. A full
/// picture queue only drains while the UI is playing, so a rewind has to be
/// able to interrupt the wait or backward stepping would deadlock on pause.
struct Sink<'a> {
    pictures: &'a SyncSender<Message>,
    commands: &'a Receiver<Rewind>,
    epoch: u64,
    requested: Option<f64>,
}

impl Sink<'_> {
    fn poll(&mut self) {
        while let Ok(request) = self.commands.try_recv() {
            self.epoch = request.epoch;
            self.requested = Some(request.before);
        }
    }

    /// Waits for queue space, abandoning the message when a rewind arrives:
    /// the UI has already stopped accepting this epoch.
    fn send(&mut self, message: Message) -> Result<()> {
        let mut message = message;
        loop {
            self.poll();
            if self.requested.is_some() {
                return Ok(());
            }
            match self.pictures.try_send(message) {
                Ok(()) => return Ok(()),
                Err(TrySendError::Full(returned)) => {
                    message = returned;
                    std::thread::sleep(Duration::from_millis(1));
                }
                Err(TrySendError::Disconnected(_)) => bail!("{}", at!("Video window closed")),
            }
        }
    }
}

// A block of packed f32 audio tagged with the media time of its first sample.
// The tag is what lets the audio callback follow pause and frame stepping.
struct Block {
    time: f64,
    samples: Vec<f32>,
}

// Everything in Audio stays on the decoder thread, including the CPAL stream.
struct Audio {
    index: usize,
    decoder: ffmpeg::decoder::Audio,
    resampler: ffmpeg::software::resampling::Context,
    channels: u16,
    rate: u32,
    position: f64,
    blocks: SyncSender<Block>,
    errors: Receiver<cpal::StreamError>,
    _stream: cpal::Stream,
}

fn decode(
    path: &Path,
    pictures: &SyncSender<Message>,
    commands: &Receiver<Rewind>,
    timeline: &Arc<Mutex<Clock>>,
) -> Result<()> {
    let mut input = ffmpeg::format::input(path).context(at!("Opening video"))?;
    let Some(stream) = input.streams().best(ffmpeg::media::Type::Video) else {
        bail!("{}", at!("No video track"));
    };
    let time_base = f64::from(stream.time_base());
    let start = stream.start_time();
    let fps = f64::from(stream.avg_frame_rate());
    let track = Track {
        index: stream.index(),
        time_base,
        origin: if start == ffmpeg::ffi::AV_NOPTS_VALUE {
            0.0
        } else {
            start as f64 * time_base
        },
        frame_duration: if fps.is_finite() && fps > 0.0 {
            1.0 / fps
        } else {
            1.0 / 30.0
        },
    };
    // The container duration is the most reliable total; fall back to the
    // video track when the container does not declare one.
    let total = input.duration();
    let duration = if total > 0 {
        total as f64 / ffmpeg::ffi::AV_TIME_BASE as f64
    } else if stream.duration() > 0 {
        stream.duration() as f64 * time_base
    } else {
        0.0
    };
    let mut context = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
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
    let mut audio = (|| {
        let Some(stream) = input.streams().best(ffmpeg::media::Type::Audio) else {
            return Ok(None);
        };
        let index = stream.index();
        let decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())
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
        let (blocks, receiver) = sync_channel(64);
        let (reporter, errors) = sync_channel(4);
        let audio_start = stream.start_time();
        // Media time of the first audio sample, relative to the video origin.
        let position = if audio_start == ffmpeg::ffi::AV_NOPTS_VALUE {
            0.0
        } else {
            (audio_start as f64 * f64::from(stream.time_base()) - track.origin).max(0.0)
        };
        let stream = output(&device, &config, receiver, reporter, timeline.clone())?;
        stream.play().context(at!("Starting audio"))?;
        Ok::<_, anyhow::Error>(Some(Audio {
            index,
            decoder,
            resampler,
            channels,
            rate,
            position,
            blocks,
            errors,
            _stream: stream,
        }))
    })()?;
    let mut sink = Sink {
        pictures,
        commands,
        epoch: 0,
        requested: None,
    };
    sink.send(Message::Duration(duration))?;
    let mut next_time = 0.0;
    loop {
        // Demux and decode until the file ends or a rewind interrupts.
        loop {
            sink.poll();
            if let Some(before) = sink.requested.take() {
                let found = locate(&mut input, &mut decoder, track, before)
                    .context(at!("Stepping back before {before:.3}s"))?;
                let Some(decoded) = found else {
                    continue;
                };
                let time = track.seconds(decoded.timestamp(), 0.0);
                next_time = time + track.frame_duration;
                if let Some(audio) = &mut audio {
                    // Audio restarts from the same place. Blocks still queued
                    // carry their old times, so the callback drops or delays
                    // them as the clock dictates.
                    audio.decoder.flush();
                    audio.position = time;
                }
                send_picture(&mut sink, &mut scaler, &decoded, time)?;
                continue;
            }
            let mut packet = ffmpeg::Packet::empty();
            match packet.read(&mut input) {
                Ok(()) => {}
                Err(ffmpeg::Error::Eof) => break,
                Err(error) => {
                    return Err(anyhow::Error::new(error).context(at!("Reading media packet")));
                }
            }
            if packet.stream() == track.index {
                decoder
                    .send_packet(&packet)
                    .context(at!("Sending video packet"))?;
                receive_video(&mut decoder, &mut scaler, &mut sink, track, &mut next_time)?;
            } else if let Some(audio) = &mut audio
                && packet.stream() == audio.index
            {
                audio
                    .decoder
                    .send_packet(&packet)
                    .context(at!("Sending audio packet"))?;
                receive(
                    &mut audio.decoder,
                    &mut audio.resampler,
                    audio.channels,
                    audio.rate,
                    &mut audio.position,
                    &audio.blocks,
                    &audio.errors,
                )?;
            }
        }
        decoder.send_eof().context(at!("Draining video decoder"))?;
        receive_video(&mut decoder, &mut scaler, &mut sink, track, &mut next_time)?;
        if let Some(audio) = &mut audio {
            audio
                .decoder
                .send_eof()
                .context(at!("Draining audio decoder"))?;
            receive(
                &mut audio.decoder,
                &mut audio.resampler,
                audio.channels,
                audio.rate,
                &mut audio.position,
                &audio.blocks,
                &audio.errors,
            )?;
            loop {
                let mut tail = frame(4096, audio.channels);
                let delay = audio
                    .resampler
                    .flush(&mut tail)
                    .context(at!("Draining audio resampler"))?;
                enqueue(
                    &tail,
                    audio.rate,
                    &mut audio.position,
                    &audio.blocks,
                    &audio.errors,
                )?;
                if delay.is_none() {
                    break;
                }
            }
        }
        // Decoding is done, but this thread owns the CPAL stream, so it must
        // outlive the queued audio tail, any pause taken at the end, and any
        // backward step from there. The UI decides when playback is over;
        // repeating end-of-stream is idempotent there and is this thread's
        // only way to notice a closed window.
        loop {
            sink.poll();
            if sink.requested.is_some() {
                break;
            }
            if let Some(audio) = &audio
                && let Ok(error) = audio.errors.try_recv()
            {
                bail!("{}", at!("Audio output failed: {error}"));
            }
            match pictures.try_send(Message::EndOfStream) {
                Ok(()) | Err(TrySendError::Full(_)) => {
                    std::thread::sleep(Duration::from_millis(100))
                }
                Err(TrySendError::Disconnected(_)) => return Ok(()),
            }
        }
    }
}

fn receive_video(
    decoder: &mut ffmpeg::decoder::Video,
    scaler: &mut VideoConverter,
    sink: &mut Sink<'_>,
    track: Track,
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
        let time = track.seconds(decoded.timestamp(), *next_time);
        *next_time = time + track.frame_duration;
        send_picture(sink, scaler, &decoded, time)?;
    }
}

fn send_picture(
    sink: &mut Sink<'_>,
    scaler: &mut VideoConverter,
    decoded: &ffmpeg::frame::Video,
    time: f64,
) -> Result<()> {
    let converted = scaler.convert(decoded)?;
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
        let Some(image) = image::RgbaImage::from_raw(converted.width(), converted.height(), bytes)
        else {
            bail!("{}", at!("Invalid video image dimensions"));
        };
        Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(
            image
        )]))
    };
    let epoch = sink.epoch;
    sink.send(Message::Picture(Picture { epoch, time, image }))
}

/// Seeks backwards and decodes forward to the last frame strictly before
/// `before`, leaving the demuxer positioned to resume streaming from there.
///
/// Keyframes can be many seconds apart, so the search window widens until one
/// lands early enough to decode something; the final attempt starts at the
/// head of the file, which is why a long-GOP file can make this slow.
fn locate(
    input: &mut ffmpeg::format::context::Input,
    decoder: &mut ffmpeg::decoder::Video,
    track: Track,
    before: f64,
) -> Result<Option<ffmpeg::frame::Video>> {
    for lead in [0.5, 2.0, 8.0, f64::INFINITY] {
        let start = (before - lead).max(0.0);
        let ts = ((start + track.origin) * ffmpeg::ffi::AV_TIME_BASE as f64) as i64;
        eprintln!("PROBE seek ts={ts} result={:?}", input.seek(ts, ..ts));
        decoder.flush();
        let mut best = None;
        'packets: loop {
            let mut packet = ffmpeg::Packet::empty();
            match packet.read(input) {
                Ok(()) => {}
                Err(ffmpeg::Error::Eof) => { eprintln!("PROBE read EOF"); break },
                Err(error) => {
                    return Err(anyhow::Error::new(error).context(at!("Reading while seeking")));
                }
            }
            if packet.stream() != track.index {
                continue;
            }
            decoder
                .send_packet(&packet)
                .context(at!("Sending video packet while seeking"))?;
            loop {
                let mut decoded = ffmpeg::frame::Video::empty();
                match decoder.receive_frame(&mut decoded) {
                    Ok(()) => {}
                    Err(ffmpeg::Error::Eof) => { eprintln!("PROBE decoder EOF"); break 'packets },
                    Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => break,
                    Err(error) => {
                        return Err(
                            anyhow::Error::new(error).context(at!("Decoding while seeking"))
                        );
                    }
                }
                // receive_frame hands back presentation order, so the last
                // frame kept is the one immediately before the target. A frame
                // without a PTS cannot be placed, so it ends the scan.
                eprintln!("PROBE frame pts={:?} secs={}", decoded.timestamp(), track.seconds(decoded.timestamp(), f64::INFINITY));
                if track.seconds(decoded.timestamp(), f64::INFINITY) >= before - EPSILON {
                    break 'packets;
                }
                best = Some(decoded);
            }
        }
        eprintln!("PROBE lead={lead} start={start} best={:?}", best.as_ref().map(|f| f.timestamp()));
        if best.is_some() {
            return Ok(best);
        }
        if start <= 0.0 {
            break;
        }
    }
    Ok(None)
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

/// Builds the output stream. Unlike mp4.rs, the callback follows the shared
/// clock instead of a fixed start instant, which is what makes pausing and
/// stepping work without ever touching the CPAL stream itself.
fn output(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    blocks: Receiver<Block>,
    reporter: SyncSender<cpal::StreamError>,
    timeline: Arc<Mutex<Clock>>,
) -> Result<cpal::Stream> {
    let channels = usize::from(config.channels);
    let rate = f64::from(config.sample_rate);
    let mut clock = *lock(&timeline);
    let mut pending = Block {
        time: 0.0,
        samples: Vec::new(),
    };
    let mut cursor = 0;
    device
        .build_output_stream(
            config,
            move |buffer: &mut [f32], info: &cpal::OutputCallbackInfo| {
                // try_lock keeps the audio thread wait-free. Losing the race
                // just reuses last callback's snapshot for a few milliseconds.
                if let Ok(current) = timeline.try_lock() {
                    clock = *current;
                }
                if let Some(paused) = clock.paused {
                    // Paused output is silent, but the queue must keep moving.
                    // The decoder blocks on a full queue, and a wedged decoder
                    // has no frames left to hand to forward stepping. Only
                    // audio the clock has already passed is dropped, so
                    // resuming in place still plays the right samples.
                    loop {
                        if cursor == pending.samples.len() {
                            let Ok(block) = blocks.try_recv() else {
                                break;
                            };
                            pending = block;
                            cursor = 0;
                        }
                        let frames = (pending.samples.len() / channels) as f64;
                        if pending.time + frames / rate > paused {
                            break;
                        }
                        cursor = pending.samples.len();
                    }
                    buffer.fill(0.0);
                    return;
                }
                // This buffer is heard after the device latency, so the media
                // time it must carry is the clock as of then, not as of now.
                let timestamp = info.timestamp();
                let latency = timestamp
                    .playback
                    .duration_since(&timestamp.callback)
                    .unwrap_or_default();
                let head = clock.time(Instant::now() + latency);
                let mut written = 0;
                while written < buffer.len() {
                    // Media time owed by the next sample slot in this buffer.
                    let need = head + (written / channels) as f64 / rate;
                    if cursor == pending.samples.len() {
                        // Underrun and end of stream both fall through to
                        // silence; the callback must never block or allocate.
                        let Ok(block) = blocks.try_recv() else {
                            break;
                        };
                        pending = block;
                        cursor = 0;
                        continue;
                    }
                    let have = pending.time + (cursor / channels) as f64 / rate;
                    if need - have > RESYNC {
                        // The clock ran ahead, so this audio is already late:
                        // discard it rather than play it out of sync.
                        let stale = (((need - have) * rate) as usize * channels)
                            .min(pending.samples.len() - cursor);
                        cursor += stale;
                        continue;
                    }
                    if have - need > RESYNC {
                        // The clock moved back, or playback has not started
                        // yet. Hold the audio and pad until it comes due.
                        let idle = (((have - need) * rate) as usize * channels)
                            .min(buffer.len() - written);
                        buffer[written..written + idle].fill(0.0);
                        written += idle;
                        continue;
                    }
                    let count = (buffer.len() - written).min(pending.samples.len() - cursor);
                    buffer[written..written + count]
                        .copy_from_slice(&pending.samples[cursor..cursor + count]);
                    written += count;
                    cursor += count;
                }
                buffer[written..].fill(0.0);
            },
            move |error| {
                let _ = reporter.try_send(error);
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
    position: &mut f64,
    blocks: &SyncSender<Block>,
    errors: &Receiver<cpal::StreamError>,
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
        enqueue(&converted, rate, position, blocks, errors)?;
    }
}

/// Tags the frame with `position`, advances it by the frame's own length, and
/// waits for room in the queue. Unlike mp4.rs there is no wait deadline: a
/// pause legitimately slows the callback down for as long as the user likes,
/// so a dead device is detected through the error channel instead.
fn enqueue(
    frame: &ffmpeg::frame::Audio,
    rate: u32,
    position: &mut f64,
    blocks: &SyncSender<Block>,
    errors: &Receiver<cpal::StreamError>,
) -> Result<()> {
    let bytes = frame.samples() * usize::from(frame.channels()) * size_of::<f32>();
    if bytes == 0 {
        return Ok(());
    }
    let mut block = Block {
        time: *position,
        samples: frame.data(0)[..bytes]
            .chunks_exact(size_of::<f32>())
            .map(|chunk| f32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect(),
    };
    *position += frame.samples() as f64 / f64::from(rate);
    loop {
        if let Ok(error) = errors.try_recv() {
            bail!("{}", at!("Audio output failed: {error}"));
        }
        match blocks.try_send(block) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Disconnected(_)) => bail!("{}", at!("Audio stream disconnected")),
            Err(TrySendError::Full(returned)) => {
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

// A panic while holding the clock must not silence the rest of playback: the
// clock is plain data, so recovering the poisoned value loses nothing.
fn lock(timeline: &Mutex<Clock>) -> MutexGuard<'_, Clock> {
    timeline.lock().unwrap_or_else(|error| error.into_inner())
}

fn timecode(seconds: f64) -> String {
    let seconds = if seconds.is_finite() && seconds > 0.0 {
        seconds as u64
    } else {
        0
    };
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Width 66 makes FFmpeg's aligned row stride larger than the visible pixel
    // row. B-frames exercise decoder draining at end of file.
    fn fixture(name: &str) -> Result<PathBuf> {
        let path =
            std::env::temp_dir().join(format!("opencut-mp4v2-{name}-{}.mp4", std::process::id()));
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
        Ok(path)
    }

    #[test]
    fn silent_video_preserves_timestamps_pixels_and_delayed_frames() -> Result<()> {
        let path = fixture("stream")?;
        let (tx, rx) = sync_channel(2);
        let (commands, requests) = sync_channel(4);
        let timeline = Arc::new(Mutex::new(Clock::started()));
        let worker_path = path.clone();
        let worker = std::thread::spawn(move || decode(&worker_path, &tx, &requests, &timeline));
        let mut times = Vec::new();
        let mut duration = 0.0;
        loop {
            match rx
                .recv_timeout(Duration::from_secs(5))
                .context(at!("Receiving test picture"))?
            {
                Message::Duration(total) => duration = total,
                Message::Picture(picture) => {
                    assert_eq!(picture.epoch, 0);
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
                    times.push(picture.time);
                }
                Message::EndOfStream => break,
                Message::Error(error) => bail!("{}", at!("Test playback failed: {error}")),
            }
        }
        // The decoder heartbeats end-of-stream until the receiver goes away.
        drop(rx);
        drop(commands);
        worker.join().expect("test decoder panicked")?;
        std::fs::remove_file(path).context(at!("Removing test video"))?;
        assert_eq!(times.len(), 10);
        assert!((duration - 0.4).abs() < 0.05, "duration: {duration}");
        for pair in times.windows(2) {
            assert!(
                (pair[1] - pair[0] - 0.04).abs() < 1e-6,
                "spacing: {} to {}",
                pair[0],
                pair[1]
            );
        }
        Ok(())
    }

    #[test]
    fn backward_stepping_finds_each_previous_frame_to_the_start() -> Result<()> {
        let path = fixture("rewind")?;
        let mut input = ffmpeg::format::input(&path).context(at!("Opening rewind fixture"))?;
        let (index, time_base, parameters) = {
            let stream = input
                .streams()
                .best(ffmpeg::media::Type::Video)
                .context(at!("No video track in rewind fixture"))?;
            (
                stream.index(),
                f64::from(stream.time_base()),
                stream.parameters(),
            )
        };
        let track = Track {
            index,
            time_base,
            origin: 0.0,
            frame_duration: 0.04,
        };
        let mut decoder = ffmpeg::codec::context::Context::from_parameters(parameters)
            .context(at!("Reading rewind parameters"))?
            .decoder()
            .video()
            .context(at!("Opening rewind decoder"))?;
        // Walk backwards from the last frame; every step must land exactly one
        // frame earlier, including across the seek to the head of the file.
        let mut before = 0.36;
        for expected in [8, 7, 6, 5, 4, 3, 2, 1, 0] {
            let found = locate(&mut input, &mut decoder, track, before)
                .context(at!("Rewinding before {before:.3}s"))?
                .context(at!("No frame before {before:.3}s"))?;
            let time = track.seconds(found.timestamp(), f64::NAN);
            assert!(
                (time - f64::from(expected) * 0.04).abs() < 1e-6,
                "expected frame {expected}, got {time}"
            );
            before = time;
        }
        // Nothing precedes the first frame.
        assert!(locate(&mut input, &mut decoder, track, 0.0)?.is_none());
        std::fs::remove_file(path).context(at!("Removing rewind fixture"))?;
        Ok(())
    }

    #[test]
    fn clock_pause_freezes_and_resume_continues() {
        let now = Instant::now();
        let mut clock = Clock {
            origin: now - Duration::from_secs(5),
            paused: None,
        };
        assert!((clock.time(now) - 5.0).abs() < 1e-6);
        clock.pause(now);
        // Paused media time ignores wall time entirely.
        assert!((clock.time(now + Duration::from_secs(60)) - 5.0).abs() < 1e-6);
        clock.resume(now + Duration::from_secs(60));
        assert!((clock.time(now + Duration::from_secs(60)) - 5.0).abs() < 1e-6);
        assert!((clock.time(now + Duration::from_secs(61)) - 6.0).abs() < 1e-6);
    }

    #[test]
    fn clock_seek_keeps_play_state() {
        let now = Instant::now();
        let mut clock = Clock {
            origin: now,
            paused: None,
        };
        clock.seek(30.0, now);
        assert!(clock.paused.is_none(), "seeking must not pause a player");
        assert!((clock.time(now + Duration::from_secs(1)) - 31.0).abs() < 1e-6);
        clock.pause(now);
        clock.seek(2.0, now);
        assert_eq!(clock.paused, Some(2.0));
    }

    #[test]
    fn clock_preroll_reports_negative_media_time() {
        let clock = Clock::started();
        assert!(clock.time(Instant::now()) < 0.0);
    }
}

#[cfg(test)]
mod performance {
    use super::*;

    // Run explicitly with OPENCUT_VIDEO_BENCHMARK=/path/to/a/silent/video.mp4
    // cargo test --no-default-features --features mp4 --bin mp4-v2 throughput -- --ignored --nocapture
    #[test]
    #[ignore = "requires a local silent video fixture"]
    fn throughput() -> Result<()> {
        let path = std::env::var_os("OPENCUT_VIDEO_BENCHMARK")
            .context(at!("Set OPENCUT_VIDEO_BENCHMARK to a silent video file"))?;
        ffmpeg::init().context(at!("Initializing benchmark decoder"))?;
        let (tx, rx) = sync_channel(8);
        let (commands, requests) = sync_channel(4);
        let timeline = Arc::new(Mutex::new(Clock::started()));
        let started = Instant::now();
        let worker =
            std::thread::spawn(move || decode(Path::new(&path), &tx, &requests, &timeline));
        let mut count = 0;
        let mut last_frame = started;
        loop {
            match rx
                .recv_timeout(Duration::from_secs(30))
                .context(at!("Receiving benchmark video"))?
            {
                Message::Duration(_) => {}
                Message::Picture(picture) => {
                    #[cfg(target_os = "macos")]
                    let _surface = video_surface(&picture.image)?;
                    count += 1;
                    last_frame = Instant::now();
                }
                Message::EndOfStream => break,
                Message::Error(error) => bail!("{}", at!("Benchmark decoder: {error}")),
            }
        }
        drop(rx);
        drop(commands);
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
