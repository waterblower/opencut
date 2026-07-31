use ffmpeg::{
    Packet, codec,
    format::{self, Pixel},
    media::Type,
    software::scaling::{context::Context as ScalingContext, flag::Flags as ScalingFlags},
    util::{frame::video::Video as DecodedFrame, rational::Rational},
};
use ffmpeg_next as ffmpeg;
use gpui::{
    AbsoluteLength, App, DefiniteLength, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Length, Pixels, RenderImage, Style, Window,
};
use image::{ImageBuffer, Rgba};
use smallvec::SmallVec;
use std::{
    fs,
    path::Path,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use url::Url;

#[path = "ffmpeg/audio.rs"]
mod audio;

use audio::{AudioOutput, decode_audio};

const AV_TIME_BASE: f64 = 1_000_000.0;
const FRAME_LATE_TOLERANCE: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
pub struct VideoOptions {
    pub frame_buffer_capacity: Option<usize>,
    pub looping: Option<bool>,
    pub speed: Option<f64>,
}

impl Default for VideoOptions {
    fn default() -> Self {
        Self {
            frame_buffer_capacity: Some(3),
            looping: Some(false),
            speed: Some(1.0),
        }
    }
}

#[derive(Clone)]
pub struct Video {
    inner: Arc<Internal>,
}

struct Internal {
    state: Arc<PlaybackState>,
    video_worker: Mutex<Option<JoinHandle<()>>>,
    audio_worker: Mutex<Option<JoinHandle<()>>>,
    width: u32,
    height: u32,
    framerate: f64,
    duration: Duration,
    codec: String,
}

impl Drop for Internal {
    fn drop(&mut self) {
        self.state.exit.store(true, Ordering::Release);
        self.state.wake.notify_all();
        if let Some(worker) = self.video_worker.lock().unwrap().take() {
            let _ = worker.join();
        }
        if let Some(worker) = self.audio_worker.lock().unwrap().take() {
            let _ = worker.join();
        }
    }
}

struct PlaybackState {
    control: Mutex<Control>,
    wake: Condvar,
    frame: Mutex<Option<Frame>>,
    frame_ready: AtomicBool,
    sequence: AtomicU64,
    seek_serial: AtomicU64,
    rendered_seek_serial: AtomicU64,
    seek_target_nanos: AtomicU64,
    seek_accurate: AtomicBool,
    audio_seek_serial: AtomicU64,
    audio_seek_target_nanos: AtomicU64,
    eos: AtomicBool,
    exit: AtomicBool,
    looping: AtomicBool,
    muted: AtomicBool,
    volume_bits: AtomicU64,
    audio: Mutex<Option<AudioOutput>>,
}

struct Control {
    base_position: Duration,
    started_at: Instant,
    paused: bool,
    speed: f64,
}

#[derive(Clone)]
struct Frame {
    pixels: Arc<Vec<u8>>,
    width: u32,
    height: u32,
    sequence: u64,
}

impl PlaybackState {
    fn position(&self, duration: Duration) -> Duration {
        let control = self.control.lock().unwrap();
        position_from_control(&control, duration)
    }

    fn seek(&self, position: Duration, duration: Duration, accurate: bool) {
        let position = position.min(duration);
        {
            let mut control = self.control.lock().unwrap();
            control.base_position = position;
            control.started_at = Instant::now();
        }
        self.seek_target_nanos.store(
            position.as_nanos().min(u64::MAX as u128) as u64,
            Ordering::Release,
        );
        self.seek_accurate.store(accurate, Ordering::Release);
        self.seek_serial.fetch_add(1, Ordering::AcqRel);
        self.eos.store(false, Ordering::Release);
        // Scrub previews are intentionally video-only. The final accurate seek
        // tells the FFmpeg audio worker to flush and restart at the target.
        if accurate {
            self.audio_seek_target_nanos.store(
                position.as_nanos().min(u64::MAX as u128) as u64,
                Ordering::Release,
            );
            self.audio_seek_serial.fetch_add(1, Ordering::AcqRel);
        }
        self.wake.notify_all();
    }

    fn restart_for_loop(&self, duration: Duration) -> u64 {
        self.seek(Duration::ZERO, duration, true);
        self.seek_serial.load(Ordering::Acquire)
    }

    fn set_paused(&self, paused: bool, duration: Duration) {
        {
            let mut control = self.control.lock().unwrap();
            if control.paused == paused {
                return;
            }
            control.base_position = position_from_control(&control, duration);
            control.started_at = Instant::now();
            control.paused = paused;
        }
        if let Some(audio) = self.audio.lock().unwrap().as_ref() {
            if paused {
                audio.player.pause();
            } else {
                audio.player.play();
            }
        }
        self.wake.notify_all();
    }

    fn set_speed(&self, speed: f64, duration: Duration) {
        {
            let mut control = self.control.lock().unwrap();
            control.base_position = position_from_control(&control, duration);
            control.started_at = Instant::now();
            control.speed = speed;
        }
        if let Some(audio) = self.audio.lock().unwrap().as_ref() {
            audio.player.set_speed(speed as f32);
        }
        self.wake.notify_all();
    }

    fn publish(&self, pixels: Vec<u8>, width: u32, height: u32, seek_serial: u64) {
        let sequence = self.sequence.fetch_add(1, Ordering::AcqRel) + 1;
        *self.frame.lock().unwrap() = Some(Frame {
            pixels: Arc::new(pixels),
            width,
            height,
            sequence,
        });
        self.rendered_seek_serial
            .store(seek_serial, Ordering::Release);
        self.frame_ready.store(true, Ordering::Release);
    }

    fn seek_frame_pending(&self) -> bool {
        self.rendered_seek_serial.load(Ordering::Acquire)
            != self.seek_serial.load(Ordering::Acquire)
    }
}

fn position_from_control(control: &Control, duration: Duration) -> Duration {
    if control.paused {
        return control.base_position.min(duration);
    }

    control
        .base_position
        .saturating_add(control.started_at.elapsed().mul_f64(control.speed))
        .min(duration)
}

impl Video {
    pub fn new_with_options(url: &Url, options: VideoOptions) -> Result<Self, String> {
        ffmpeg::init().map_err(|error| format!("could not initialize FFmpeg: {error}"))?;
        let source = ffmpeg_source(url);
        let metadata = probe(&source)?;
        let speed = options.speed.unwrap_or(1.0);
        let _frame_buffer_capacity = options.frame_buffer_capacity.unwrap_or(3);
        if !speed.is_finite() || speed <= 0.0 {
            return Err("playback speed must be greater than zero".to_string());
        }

        let state = Arc::new(PlaybackState {
            control: Mutex::new(Control {
                base_position: Duration::ZERO,
                started_at: Instant::now(),
                paused: false,
                speed,
            }),
            wake: Condvar::new(),
            frame: Mutex::new(None),
            frame_ready: AtomicBool::new(false),
            sequence: AtomicU64::new(0),
            seek_serial: AtomicU64::new(0),
            rendered_seek_serial: AtomicU64::new(0),
            seek_target_nanos: AtomicU64::new(0),
            seek_accurate: AtomicBool::new(true),
            audio_seek_serial: AtomicU64::new(0),
            audio_seek_target_nanos: AtomicU64::new(0),
            eos: AtomicBool::new(false),
            exit: AtomicBool::new(false),
            looping: AtomicBool::new(options.looping.unwrap_or(false)),
            muted: AtomicBool::new(false),
            volume_bits: AtomicU64::new(1.0_f64.to_bits()),
            audio: Mutex::new(AudioOutput::open(speed)),
        });
        let video_worker_state = state.clone();
        let worker_duration = metadata.duration;
        let worker_framerate = metadata.framerate;
        let video_source = source.clone();
        let video_worker = thread::Builder::new()
            .name("opencut-ffmpeg-video".to_string())
            .spawn(move || {
                if let Err(error) = decode_video(
                    &video_source,
                    worker_duration,
                    worker_framerate,
                    &video_worker_state,
                ) {
                    eprintln!("FFmpeg video worker stopped: {error}");
                    video_worker_state.eos.store(true, Ordering::Release);
                }
            })
            .map_err(|error| format!("could not start FFmpeg decoder: {error}"))?;
        let audio_worker = if state.audio.lock().unwrap().is_some() {
            let audio_worker_state = state.clone();
            Some(
                thread::Builder::new()
                    .name("opencut-ffmpeg-audio".to_string())
                    .spawn(move || {
                        if let Err(error) = decode_audio(&source, &audio_worker_state) {
                            eprintln!("FFmpeg audio worker stopped: {error}");
                        }
                    })
                    .map_err(|error| format!("could not start FFmpeg audio decoder: {error}"))?,
            )
        } else {
            None
        };

        Ok(Self {
            inner: Arc::new(Internal {
                state,
                video_worker: Mutex::new(Some(video_worker)),
                audio_worker: Mutex::new(audio_worker),
                width: metadata.width,
                height: metadata.height,
                framerate: metadata.framerate,
                duration: metadata.duration,
                codec: metadata.codec,
            }),
        })
    }

    pub fn display_size(&self) -> (u32, u32) {
        (self.inner.width, self.inner.height)
    }

    pub fn framerate(&self) -> f64 {
        self.inner.framerate
    }

    pub fn duration(&self) -> Duration {
        self.inner.duration
    }

    pub fn position(&self) -> Duration {
        self.inner.state.position(self.inner.duration)
    }

    pub fn paused(&self) -> bool {
        self.inner.state.control.lock().unwrap().paused
    }

    pub fn set_paused(&self, paused: bool) {
        self.inner.state.set_paused(paused, self.inner.duration);
    }

    pub fn eos(&self) -> bool {
        self.inner.state.eos.load(Ordering::Acquire)
    }

    pub fn restart_stream(&self) -> Result<(), String> {
        self.inner
            .state
            .seek(Duration::ZERO, self.inner.duration, true);
        Ok(())
    }

    pub fn seek(&self, position: Duration, accurate: bool) -> Result<(), String> {
        self.inner
            .state
            .seek(position, self.inner.duration, accurate);
        Ok(())
    }

    pub fn speed(&self) -> f64 {
        self.inner.state.control.lock().unwrap().speed
    }

    pub fn set_speed(&self, speed: f64) -> Result<(), String> {
        if !speed.is_finite() || speed <= 0.0 {
            return Err("playback speed must be greater than zero".to_string());
        }
        self.inner.state.set_speed(speed, self.inner.duration);
        Ok(())
    }

    pub fn volume(&self) -> f64 {
        f64::from_bits(self.inner.state.volume_bits.load(Ordering::Acquire))
    }

    pub fn set_volume(&self, volume: f64) {
        let volume = volume.clamp(0.0, 1.0);
        self.inner
            .state
            .volume_bits
            .store(volume.to_bits(), Ordering::Release);
        if let Some(audio) = self.inner.state.audio.lock().unwrap().as_ref() {
            audio
                .player
                .set_volume(if self.muted() { 0.0 } else { volume as f32 });
        }
    }

    pub fn muted(&self) -> bool {
        self.inner.state.muted.load(Ordering::Acquire)
    }

    pub fn set_muted(&self, muted: bool) {
        self.inner.state.muted.store(muted, Ordering::Release);
        if let Some(audio) = self.inner.state.audio.lock().unwrap().as_ref() {
            audio
                .player
                .set_volume(if muted { 0.0 } else { self.volume() as f32 });
        }
    }

    fn frame(&self) -> Option<Frame> {
        self.inner.state.frame.lock().unwrap().clone()
    }

    fn take_frame_ready(&self) -> bool {
        self.inner.state.frame_ready.swap(false, Ordering::AcqRel)
    }

    fn seek_frame_pending(&self) -> bool {
        self.inner.state.seek_frame_pending()
    }
}

pub fn read_video_codec(video: &Video) -> Option<String> {
    Some(video.inner.codec.clone())
}

pub fn current_frame_rgba(video: &Video) -> Option<(Vec<u8>, u32, u32)> {
    let frame = video.frame()?;
    let mut rgba = frame.pixels.as_ref().clone();
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Some((rgba, frame.width, frame.height))
}

pub fn video(video: Video) -> VideoElement {
    VideoElement::new(video)
}

pub struct VideoElement {
    video: Video,
    display_width: Option<Pixels>,
    display_height: Option<Pixels>,
    element_id: Option<ElementId>,
}

impl VideoElement {
    fn new(video: Video) -> Self {
        Self {
            video,
            display_width: None,
            display_height: None,
            element_id: None,
        }
    }

    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.element_id = Some(id.into());
        self
    }

    pub fn size(mut self, width: Pixels, height: Pixels) -> Self {
        self.display_width = Some(width);
        self.display_height = Some(height);
        self
    }

    pub fn buffer_capacity(self, _capacity: usize) -> Self {
        self
    }

    fn fitted_bounds(
        &self,
        bounds: gpui::Bounds<Pixels>,
        frame_width: u32,
        frame_height: u32,
    ) -> gpui::Bounds<Pixels> {
        let container_width: f32 = bounds.size.width.into();
        let container_height: f32 = bounds.size.height.into();
        let scale = (container_width / frame_width as f32)
            .min(container_height / frame_height as f32)
            .max(0.0);
        let width = frame_width as f32 * scale;
        let height = frame_height as f32 * scale;

        gpui::Bounds::new(
            gpui::point(
                bounds.origin.x + gpui::px((container_width - width) / 2.0),
                bounds.origin.y + gpui::px((container_height - height) / 2.0),
            ),
            gpui::size(gpui::px(width), gpui::px(height)),
        )
    }
}

impl Element for VideoElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        self.element_id.clone()
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let (video_width, video_height) = self.video.display_size();
        let width = self
            .display_width
            .unwrap_or_else(|| gpui::px(video_width as f32));
        let height = self
            .display_height
            .unwrap_or_else(|| gpui::px(video_height as f32));
        let style = Style {
            size: gpui::Size {
                width: Length::Definite(DefiniteLength::Absolute(AbsoluteLength::Pixels(width))),
                height: Length::Definite(DefiniteLength::Absolute(AbsoluteLength::Pixels(height))),
            },
            ..Style::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: gpui::Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        _: &mut App,
    ) {
        let seek_frame_pending = !self.video.eos() && self.video.seek_frame_pending();
        if (!self.video.paused() && !self.video.eos())
            || self.video.take_frame_ready()
            || seek_frame_pending
        {
            window.request_animation_frame();
        }
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: gpui::Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let Some(frame) = self.video.frame() else {
            return;
        };
        let render_state: gpui::Entity<Option<(u64, Arc<RenderImage>)>> =
            window.use_state(cx, |_, _| None);
        let needs_upload = render_state
            .read(cx)
            .as_ref()
            .is_none_or(|(sequence, _)| *sequence != frame.sequence);

        if needs_upload
            && let Some(buffer) = ImageBuffer::<Rgba<u8>, _>::from_raw(
                frame.width,
                frame.height,
                frame.pixels.as_ref().clone(),
            )
        {
            let frames = SmallVec::from_elem(image::Frame::new(buffer), 1);
            let image = Arc::new(RenderImage::new(frames));
            let previous = render_state.update(cx, |state, _| {
                state.replace((frame.sequence, image.clone()))
            });
            if let Some((_, previous)) = previous {
                cx.drop_image(previous, Some(window));
            }
        }

        if let Some((_, image)) = render_state.read(cx).as_ref() {
            let destination = self.fitted_bounds(bounds, frame.width, frame.height);
            let _ = window.paint_image(
                destination,
                gpui::Corners::default(),
                image.clone(),
                0,
                false,
            );
        }
    }
}

impl IntoElement for VideoElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

struct Metadata {
    width: u32,
    height: u32,
    framerate: f64,
    duration: Duration,
    codec: String,
}

fn probe(source: &str) -> Result<Metadata, String> {
    let input = format::input(source).map_err(|error| format!("could not open media: {error}"))?;
    let stream = input
        .streams()
        .best(Type::Video)
        .ok_or_else(|| "media has no video stream".to_string())?;
    let context = codec::context::Context::from_parameters(stream.parameters())
        .map_err(|error| format!("could not read video parameters: {error}"))?;
    let decoder = context
        .decoder()
        .video()
        .map_err(|error| format!("could not open video decoder: {error}"))?;
    let framerate = rational_to_f64(stream.avg_frame_rate())
        .filter(|fps| fps.is_finite() && *fps > 0.0)
        .or_else(|| rational_to_f64(stream.rate()))
        .unwrap_or(30.0);
    let duration = if input.duration() > 0 {
        Duration::from_secs_f64(input.duration() as f64 / AV_TIME_BASE)
    } else {
        duration_from_stream(stream.duration(), stream.time_base())
    };

    Ok(Metadata {
        width: decoder.width(),
        height: decoder.height(),
        framerate,
        duration,
        codec: stream.parameters().id().name().to_string(),
    })
}

fn decode_video(
    source: &str,
    duration: Duration,
    framerate: f64,
    state: &PlaybackState,
) -> Result<(), String> {
    let mut input =
        format::input(source).map_err(|error| format!("could not open media: {error}"))?;
    let (stream_index, time_base, start_time, mut decoder) = {
        let stream = input
            .streams()
            .best(Type::Video)
            .ok_or_else(|| "media has no video stream".to_string())?;
        let context = codec::context::Context::from_parameters(stream.parameters())
            .map_err(|error| format!("could not read video parameters: {error}"))?;
        let decoder = context
            .decoder()
            .video()
            .map_err(|error| format!("could not open video decoder: {error}"))?;
        (
            stream.index(),
            stream.time_base(),
            stream.start_time(),
            decoder,
        )
    };
    let mut scaler = ScalingContext::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        Pixel::BGRA,
        decoder.width(),
        decoder.height(),
        ScalingFlags::BILINEAR,
    )
    .map_err(|error| format!("could not create video scaler: {error}"))?;
    let mut packet = Packet::empty();
    let mut seen_seek = state.seek_serial.load(Ordering::Acquire);
    let mut seen_seek_accurate = true;
    let mut fallback_position = Duration::ZERO;

    loop {
        if state.exit.load(Ordering::Acquire) {
            return Ok(());
        }

        let requested_seek = state.seek_serial.load(Ordering::Acquire);
        if requested_seek != seen_seek {
            let target = Duration::from_nanos(state.seek_target_nanos.load(Ordering::Acquire));
            let target = duration_to_av_time(target);
            seen_seek_accurate = state.seek_accurate.load(Ordering::Acquire);
            let result = if seen_seek_accurate {
                input.seek(target, ..target)
            } else {
                // Match GStreamer's KEY_UNIT + SNAP_AFTER preview behavior:
                // jump directly to the next keyframe instead of decoding the
                // whole GOP for every mouse movement.
                input
                    .seek(target, target..)
                    .or_else(|_| input.seek(target, ..target))
            };
            result.map_err(|error| format!("could not seek video: {error}"))?;
            decoder.flush();
            packet = Packet::empty();
            fallback_position =
                Duration::from_nanos(state.seek_target_nanos.load(Ordering::Acquire));
            seen_seek = requested_seek;
            state.eos.store(false, Ordering::Release);
        }

        match packet.read(&mut input) {
            Ok(()) => {
                if packet.stream() != stream_index {
                    packet = Packet::empty();
                    continue;
                }
                decoder
                    .send_packet(&packet)
                    .map_err(|error| format!("could not send video packet: {error}"))?;
                packet = Packet::empty();
                if !receive_frames(
                    &mut decoder,
                    &mut scaler,
                    state,
                    duration,
                    time_base,
                    start_time,
                    framerate,
                    seen_seek,
                    seen_seek_accurate,
                    &mut fallback_position,
                )? {
                    continue;
                }
            }
            Err(ffmpeg::Error::Eof) => {
                let _ = decoder.send_eof();
                let _ = receive_frames(
                    &mut decoder,
                    &mut scaler,
                    state,
                    duration,
                    time_base,
                    start_time,
                    framerate,
                    seen_seek,
                    seen_seek_accurate,
                    &mut fallback_position,
                );
                if state.looping.load(Ordering::Acquire) {
                    state.restart_for_loop(duration);
                    continue;
                }

                state.eos.store(true, Ordering::Release);
                state
                    .rendered_seek_serial
                    .store(seen_seek, Ordering::Release);
                let mut control = state.control.lock().unwrap();
                while !state.exit.load(Ordering::Acquire)
                    && state.seek_serial.load(Ordering::Acquire) == seen_seek
                {
                    control = state.wake.wait(control).unwrap();
                }
            }
            Err(error) => return Err(format!("could not read video packet: {error}")),
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn receive_frames(
    decoder: &mut ffmpeg::decoder::Video,
    scaler: &mut ScalingContext,
    state: &PlaybackState,
    duration: Duration,
    time_base: Rational,
    start_time: i64,
    framerate: f64,
    seek_serial: u64,
    accurate: bool,
    fallback_position: &mut Duration,
) -> Result<bool, String> {
    let mut decoded = DecodedFrame::empty();
    while decoder.receive_frame(&mut decoded).is_ok() {
        if state.seek_serial.load(Ordering::Acquire) != seek_serial {
            return Ok(false);
        }

        let timestamp = decoded.timestamp().or_else(|| decoded.pts());
        let position = timestamp
            .and_then(|timestamp| timestamp_to_duration(timestamp, start_time, time_base))
            .unwrap_or(*fallback_position)
            .min(duration);
        let frame_duration = Duration::from_secs_f64(1.0 / framerate);
        *fallback_position = position.saturating_add(frame_duration);

        let current = state.position(duration);
        let paused = state.control.lock().unwrap().paused;
        if paused {
            if accurate {
                let nearest_frame_tolerance = frame_duration.div_f64(2.0);
                if position.saturating_add(nearest_frame_tolerance) < current {
                    continue;
                }
            }
        } else {
            if !wait_for_frame(state, position, duration, seek_serial) {
                return Ok(false);
            }
            if state.position(duration) > position.saturating_add(FRAME_LATE_TOLERANCE) {
                continue;
            }
        }

        let mut rgba = DecodedFrame::empty();
        scaler
            .run(&decoded, &mut rgba)
            .map_err(|error| format!("could not convert video frame: {error}"))?;
        let pixels = copy_rgba(&rgba)?;
        if state.seek_serial.load(Ordering::Acquire) != seek_serial {
            return Ok(false);
        }
        state.publish(pixels, rgba.width(), rgba.height(), seek_serial);

        if paused {
            wait_while_paused(state, seek_serial);
            return Ok(false);
        }
    }

    Ok(true)
}

fn wait_while_paused(state: &PlaybackState, seek_serial: u64) {
    let mut control = state.control.lock().unwrap();
    while control.paused
        && !state.exit.load(Ordering::Acquire)
        && state.seek_serial.load(Ordering::Acquire) == seek_serial
    {
        control = state.wake.wait(control).unwrap();
    }
}

fn wait_for_frame(
    state: &PlaybackState,
    position: Duration,
    duration: Duration,
    seek_serial: u64,
) -> bool {
    loop {
        if state.exit.load(Ordering::Acquire)
            || state.seek_serial.load(Ordering::Acquire) != seek_serial
        {
            return false;
        }

        let control = state.control.lock().unwrap();
        let current = position_from_control(&control, duration);
        if !control.paused && current.saturating_add(Duration::from_millis(2)) >= position {
            return true;
        }
        let wait = if control.paused {
            Duration::from_millis(20)
        } else {
            position
                .saturating_sub(current)
                .div_f64(control.speed)
                .min(Duration::from_millis(20))
        };
        let _ = state.wake.wait_timeout(control, wait).unwrap();
    }
}

fn copy_rgba(frame: &DecodedFrame) -> Result<Vec<u8>, String> {
    let row_bytes = frame.width() as usize * 4;
    let stride = frame.stride(0);
    let source = frame.data(0);
    let mut pixels = Vec::with_capacity(row_bytes * frame.height() as usize);
    for row in 0..frame.height() as usize {
        let start = row
            .checked_mul(stride)
            .ok_or_else(|| "video frame row offset overflowed".to_string())?;
        let end = start
            .checked_add(row_bytes)
            .ok_or_else(|| "video frame row size overflowed".to_string())?;
        pixels.extend_from_slice(
            source
                .get(start..end)
                .ok_or_else(|| "video frame row was truncated".to_string())?,
        );
    }
    Ok(pixels)
}

pub fn generate_thumbnail(video_path: &Path, output_path: &Path) -> Result<(), String> {
    ffmpeg::init().map_err(|error| format!("could not initialize FFmpeg: {error}"))?;
    if let Some(directory) = output_path.parent() {
        fs::create_dir_all(directory)
            .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
    }
    if output_path.is_file() {
        return Ok(());
    }

    let mut input = format::input(video_path)
        .map_err(|error| format!("could not open {}: {error}", video_path.display()))?;
    let (stream_index, mut decoder) = {
        let stream = input
            .streams()
            .best(Type::Video)
            .ok_or_else(|| "media has no video stream".to_string())?;
        let context = codec::context::Context::from_parameters(stream.parameters())
            .map_err(|error| format!("could not read video parameters: {error}"))?;
        let decoder = context
            .decoder()
            .video()
            .map_err(|error| format!("could not open video decoder: {error}"))?;
        (stream.index(), decoder)
    };
    let mut scaler = ScalingContext::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        Pixel::RGBA,
        320,
        180,
        ScalingFlags::BILINEAR,
    )
    .map_err(|error| format!("could not create thumbnail scaler: {error}"))?;
    let mut packet = Packet::empty();

    loop {
        packet
            .read(&mut input)
            .map_err(|error| format!("could not read first frame: {error}"))?;
        if packet.stream() != stream_index {
            packet = Packet::empty();
            continue;
        }
        decoder
            .send_packet(&packet)
            .map_err(|error| format!("could not decode first frame: {error}"))?;
        let mut decoded = DecodedFrame::empty();
        if decoder.receive_frame(&mut decoded).is_ok() {
            let mut rgba = DecodedFrame::empty();
            scaler
                .run(&decoded, &mut rgba)
                .map_err(|error| format!("could not convert first frame: {error}"))?;
            let pixels = copy_rgba(&rgba)?;
            let temporary_path = output_path.with_extension("png.part");
            image::save_buffer_with_format(
                &temporary_path,
                &pixels,
                rgba.width(),
                rgba.height(),
                image::ColorType::Rgba8,
                image::ImageFormat::Png,
            )
            .map_err(|error| format!("could not encode thumbnail: {error}"))?;
            return fs::rename(&temporary_path, output_path)
                .map_err(|error| format!("could not finish thumbnail: {error}"));
        }
        packet = Packet::empty();
    }
}

fn ffmpeg_source(url: &Url) -> String {
    url.to_file_path()
        .ok()
        .map(|path| path.to_string_lossy().into_owned())
        .unwrap_or_else(|| url.as_str().to_string())
}

fn rational_to_f64(value: Rational) -> Option<f64> {
    (value.denominator() != 0).then(|| value.numerator() as f64 / value.denominator() as f64)
}

fn duration_from_stream(duration: i64, time_base: Rational) -> Duration {
    if duration <= 0 {
        return Duration::ZERO;
    }
    Duration::from_secs_f64(duration as f64 * f64::from(time_base))
}

fn timestamp_to_duration(timestamp: i64, start_time: i64, time_base: Rational) -> Option<Duration> {
    let timestamp = if start_time > i64::MIN / 2 {
        timestamp.saturating_sub(start_time)
    } else {
        timestamp
    };
    let seconds = timestamp as f64 * f64::from(time_base);
    (seconds.is_finite() && seconds >= 0.0).then(|| Duration::from_secs_f64(seconds))
}

fn duration_to_av_time(duration: Duration) -> i64 {
    (duration.as_secs_f64() * AV_TIME_BASE).round() as i64
}
