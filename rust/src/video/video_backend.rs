use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

use gst::prelude::*;

use gst_audio::prelude::StreamVolumeExt as _;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_audio as gst_audio;

use gstreamer_video as gst_video;

use parking_lot::Mutex;
use url::Url;

const PREFETCH_BEHIND: Duration = Duration::from_secs(10);
const PREFETCH_AHEAD: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub(crate) struct VideoBackend {
    pipeline: gst::Pipeline,
    sink: gst_app::AppSink,
    volume_control: gst_audio::StreamVolume,
    current_frame: Arc<Mutex<Option<gst::Sample>>>,
    cached_position: Duration,
    frame_cache: Arc<Mutex<FrameCache>>,
    pending_cached_seek: Arc<Mutex<Option<Duration>>>,
    prefetch: Option<PrefetchController>,
}

impl Drop for VideoBackend {
    fn drop(&mut self) {
        if let Err(error) = self.pipeline.set_state(gst::State::Null) {
            log::error!("could not stop video pipeline: {error}");
        }
    }
}

impl VideoBackend {
    // pub fn from_file(path: Path) -> Result<Self, String> {}
    pub(crate) fn open(uri: &Url) -> Result<Self, String> {
        gst::init().map_err(|error| format!("could not initialize GStreamer: {error}"))?;
        let caps = gst_video::VideoCapsBuilder::new()
            .format(gst_video::VideoFormat::Nv12)
            .pixel_aspect_ratio(gst::Fraction::new(1, 1))
            .build();
        let converter = gst::ElementFactory::make("videoconvert")
            .build()
            .map_err(|error| format!("could not create video converter: {error}"))?;
        let sink = gst_app::AppSink::builder()
            .name("opencut_player_video")
            .drop(true)
            .max_buffers(3)
            .enable_last_sample(false)
            .caps(&caps)
            .build();
        let video_sink = gst::Bin::new();
        video_sink
            .add(&converter)
            .map_err(|error| format!("could not add video converter: {error}"))?;
        video_sink
            .add(&sink)
            .map_err(|error| format!("could not add video sink: {error}"))?;
        converter
            .link(&sink)
            .map_err(|error| format!("could not link video sink: {error}"))?;
        let converter_sink_pad = converter
            .static_pad("sink")
            .ok_or_else(|| "video converter has no sink pad".to_string())?;
        let ghost_pad = gst::GhostPad::builder_with_target(&converter_sink_pad)
            .map_err(|error| format!("could not create video sink pad: {error}"))?
            .name("sink")
            .build();
        ghost_pad
            .set_active(true)
            .map_err(|error| format!("could not activate video sink pad: {error}"))?;
        video_sink
            .add_pad(&ghost_pad)
            .map_err(|error| format!("could not expose video sink pad: {error}"))?;
        let playbin = gst::ElementFactory::make("playbin")
            .property("uri", uri.as_str())
            .property("video-sink", &video_sink)
            .build()
            .map_err(|error| format!("could not create video pipeline: {error}"))?;
        let pipeline = playbin
            .downcast::<gst::Pipeline>()
            .map_err(|_| "video pipeline had an unexpected type".to_string())?;

        let volume_control = pipeline
            .clone()
            .upcast::<gst::Element>()
            .dynamic_cast::<gst_audio::StreamVolume>()
            .map_err(|_| "video pipeline does not support stream volume".to_string())?;
        let mut video = Self::from_pipeline(pipeline, sink, volume_control)?;
        video.prefetch = Some(PrefetchController::new(uri, video.frame_cache.clone())?);
        video.set_paused(false);
        Ok(video)
    }

    pub(crate) fn from_pipeline(
        pipeline: gst::Pipeline,
        sink: gst_app::AppSink,
        volume_control: gst_audio::StreamVolume,
    ) -> Result<Self, String> {
        gst::init().map_err(|error| format!("could not initialize GStreamer: {error}"))?;
        let current_frame = Arc::new(Mutex::new(None));
        let frame_cache = Arc::new(Mutex::new(FrameCache::default()));
        let pending_cached_seek = Arc::new(Mutex::new(None));
        sink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample({
                    let current_frame = current_frame.clone();
                    let pending_cached_seek = pending_cached_seek.clone();
                    move |sink| {
                        let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                        let pending_cached_seek = pending_cached_seek.lock();
                        if pending_cached_seek.is_none() {
                            *current_frame.lock() = Some(sample);
                        }
                        Ok(gst::FlowSuccess::Ok)
                    }
                })
                .new_preroll({
                    let current_frame = current_frame.clone();
                    let pending_cached_seek = pending_cached_seek.clone();
                    move |sink| {
                        let sample = sink.pull_preroll().map_err(|_| gst::FlowError::Eos)?;
                        let pending_cached_seek = pending_cached_seek.lock();
                        if pending_cached_seek.is_none() {
                            *current_frame.lock() = Some(sample);
                        }
                        Ok(gst::FlowSuccess::Ok)
                    }
                })
                .build(),
        );
        pipeline
            .set_state(gst::State::Paused)
            .map_err(|error| format!("could not prepare video: {error}"))?;
        if let Err(error) = pipeline.state(gst::ClockTime::from_seconds(5)).0 {
            let _ = pipeline.set_state(gst::State::Null);
            return Err(format!("video did not finish preparing: {error}"));
        }

        // GStreamer negotiates `framerate=0/1` for variable-frame-rate sources such as
        // screen recordings. That is a valid "unknown rate" marker, not a broken file, so
        // it must not fail the load — the container's nominal rate is still available from
        // the probed asset when a caller needs one.

        Ok(VideoBackend {
            current_frame,
            pipeline,
            sink,
            volume_control,
            cached_position: Duration::ZERO,
            frame_cache,
            pending_cached_seek,
            prefetch: None,
        })
    }

    pub(crate) fn frame_size(&self) -> Option<(u32, u32)> {
        let caps = self.cap().ok()?;
        let info = gst_video::VideoInfo::from_caps(&caps)
            .expect("negotiated AppSink caps must describe raw video");
        Some((info.width(), info.height()))
    }

    /// The negotiated frame rate, or `None` for variable-frame-rate sources where
    /// GStreamer reports `0/1`.
    pub(crate) fn framerate(&self) -> Option<f64> {
        let caps = self.cap().ok()?;
        let info = match gst_video::VideoInfo::from_caps(&caps) {
            Ok(info) => info,
            Err(_) => return None,
        };
        let framerate = frame_rate_from_caps(&info);
        return framerate;
    }

    pub(crate) fn duration(&self) -> Duration {
        let duration = Duration::from_nanos(
            self.pipeline()
                .query_duration::<gst::ClockTime>()
                .map_or(0, |duration| duration.nseconds()),
        );
        return duration;
    }

    pub(crate) fn position(&self) -> Duration {
        if let Some(position) = *self.pending_cached_seek.lock() {
            return position;
        }
        let Some(position) = self.pipeline.query_position::<gst::ClockTime>() else {
            return self.cached_position;
        };
        Duration::from_nanos(position.nseconds())
    }

    pub(crate) fn paused(&self) -> bool {
        self.pipeline.current_state() != gst::State::Playing
    }

    pub fn current_state(&self) -> gst::State {
        self.pipeline.current_state()
    }

    pub(crate) fn set_paused(&self, paused: bool) {
        if !paused {
            let pending_position = *self.pending_cached_seek.lock();
            if let Some(position) = pending_position {
                let result = self.pipeline.seek(
                    1.0,
                    gst::SeekFlags::FLUSH | gst::SeekFlags::ACCURATE,
                    gst::SeekType::Set,
                    gst::ClockTime::from_nseconds(duration_ns(position)),
                    gst::SeekType::None,
                    gst::ClockTime::NONE,
                );
                if let Err(error) = result {
                    log::error!("could not synchronize video after scrubbing: {error}");
                    return;
                }
                *self.pending_cached_seek.lock() = None;
            }
        }
        let state = if paused {
            gst::State::Paused
        } else {
            gst::State::Playing
        };
        if let Err(error) = self.pipeline.set_state(state) {
            log::error!("could not change video playback state: {error}");
        }
    }

    pub(crate) fn seek(&mut self, position: Duration, accurate: bool) -> Result<(), String> {
        let (cached_frame, needs_recenter) = {
            let frame_cache = self.frame_cache.lock();
            (
                frame_cache.frame_at(position),
                frame_cache.needs_recenter(position),
            )
        };
        if needs_recenter && let Some(prefetch) = &self.prefetch {
            let start = position.saturating_sub(PREFETCH_BEHIND);
            let end = position.saturating_add(PREFETCH_AHEAD).min(self.duration());
            let plan = self.frame_cache.lock().prepare_window(start, end, position);
            prefetch.request(plan);
        }

        if let Some(sample) = cached_frame {
            let mut pending_cached_seek = self.pending_cached_seek.lock();
            *pending_cached_seek = Some(position);
            *self.current_frame.lock() = Some(sample);
            self.cached_position = position;
            return Ok(());
        }

        *self.pending_cached_seek.lock() = None;
        let mut flags = gst::SeekFlags::FLUSH;
        if accurate {
            flags |= gst::SeekFlags::ACCURATE;
        } else {
            flags |= gst::SeekFlags::KEY_UNIT | gst::SeekFlags::SNAP_AFTER;
        }
        self.pipeline
            .seek(
                1.0,
                flags,
                gst::SeekType::Set,
                gst::ClockTime::from_nseconds(duration_ns(position)),
                gst::SeekType::None,
                gst::ClockTime::NONE,
            )
            .map_err(|error| format!("could not seek video: {error}"))?;
        self.cached_position = position;
        Ok(())
    }

    #[allow(dead_code)] // Used by the player binary, but not the editor binary.
    pub(crate) fn volume(&self) -> f64 {
        self.volume_control
            .volume(gst_audio::StreamVolumeFormat::Linear)
    }

    pub(crate) fn set_volume(&self, volume: f64) {
        self.volume_control.set_volume(
            gst_audio::StreamVolumeFormat::Linear,
            volume.clamp(0.0, 1.0),
        );
    }

    #[allow(dead_code)] // Used by the player binary, but not the editor binary.
    pub(crate) fn muted(&self) -> bool {
        self.volume_control.is_muted()
    }

    pub(crate) fn set_muted(&self, muted: bool) {
        self.volume_control.set_mute(muted);
    }

    pub(crate) fn pipeline(&self) -> gst::Pipeline {
        self.pipeline.clone()
    }

    pub fn get_current_frame(&self) -> Option<gst::Sample> {
        self.current_frame.lock().clone()
    }

    //////////////////////
    // Private  Methods //
    //////////////////////
    fn cap(&self) -> Result<gst::Caps, String> {
        let pad = self
            .sink
            .static_pad("sink")
            .expect("AppSink must have a static sink pad");

        let Some(caps) = pad.current_caps() else {
            let _ = self.pipeline().set_state(gst::State::Null);
            return Err("video caps were not negotiated".to_string());
        };
        return Ok(caps);
    }
}

/// Reads the negotiated frame rate, returning `None` when it is unusable.
///
/// GStreamer uses `0/1` to mean "variable or unknown frame rate", which is what
/// variable-frame-rate sources such as screen recordings negotiate even when the
/// container declares a nominal rate. A zero denominator is likewise unusable.
fn frame_rate_from_caps(info: &gst_video::VideoInfo) -> Option<f64> {
    frame_rate_from_fraction(info.fps().numer(), info.fps().denom())
}

fn frame_rate_from_fraction(numerator: i32, denominator: i32) -> Option<f64> {
    if numerator <= 0 || denominator <= 0 {
        return None;
    }
    let frame_rate = numerator as f64 / denominator as f64;
    frame_rate.is_finite().then_some(frame_rate)
}

fn duration_ns(duration: Duration) -> u64 {
    duration.as_nanos().min(u64::MAX as u128) as u64
}

#[derive(Debug, Default)]
struct FrameCache {
    frames: BTreeMap<u64, CachedFrame>,
    bytes: usize,
    byte_limit: Option<usize>,
    center_ns: Option<u64>,
    requested_range: Option<(u64, u64)>,
}

impl FrameCache {
    fn frame_at(&self, position: Duration) -> Option<gst::Sample> {
        let position = duration_ns(position);
        let (_, frame) = self.frames.range(..=position).next_back()?;
        (position < frame.end_ns).then(|| frame.sample.clone())
    }

    fn needs_recenter(&self, position: Duration) -> bool {
        let Some(center) = self.center_ns else {
            return true;
        };
        let position = duration_ns(position);
        let moved_backward = center
            .checked_sub(duration_ns(PREFETCH_BEHIND) / 2)
            .is_some_and(|threshold| position <= threshold);
        let moved_forward = position >= center.saturating_add(duration_ns(PREFETCH_AHEAD) / 2);
        moved_backward || moved_forward
    }

    fn prepare_window(&mut self, start: Duration, end: Duration, target: Duration) -> PrefetchPlan {
        let start = duration_ns(start);
        let end = duration_ns(end);
        let target = duration_ns(target);
        if !self.needs_recenter(Duration::from_nanos(target)) {
            return PrefetchPlan::default();
        }

        let plan = match self.center_ns {
            Some(old_center) if target > old_center => {
                let cached_end = self
                    .frames
                    .values()
                    .map(|frame| frame.end_ns)
                    .max()
                    .unwrap_or(start)
                    .clamp(start, end);
                PrefetchPlan {
                    leading: None,
                    trailing: (cached_end < end).then(|| PrefetchRequest {
                        start: Duration::from_nanos(cached_end),
                        end: Duration::from_nanos(end),
                    }),
                }
            }
            Some(old_center) if target < old_center => {
                let cached_start = self
                    .frames
                    .first_key_value()
                    .map(|(pts, _)| *pts)
                    .unwrap_or(end)
                    .clamp(start, end);
                PrefetchPlan {
                    leading: (start < cached_start).then(|| PrefetchRequest {
                        start: Duration::from_nanos(start),
                        end: Duration::from_nanos(cached_start),
                    }),
                    trailing: None,
                }
            }
            _ => PrefetchPlan {
                leading: (start < target).then(|| PrefetchRequest {
                    start: Duration::from_nanos(start),
                    end: Duration::from_nanos(target),
                }),
                trailing: (target < end).then(|| PrefetchRequest {
                    start: Duration::from_nanos(target),
                    end: Duration::from_nanos(end),
                }),
            },
        };
        self.center_ns = Some(target);
        self.requested_range = Some((start, end));
        self.frames
            .retain(|pts, frame| *pts < end && frame.end_ns > start);
        self.bytes = self.frames.values().map(|frame| frame.bytes).sum();
        self.recalculate_byte_limit();
        plan
    }

    fn insert(&mut self, sample: gst::Sample) {
        let Some((start, end)) = self.requested_range else {
            return;
        };
        let Some(buffer) = sample.buffer() else {
            return;
        };
        let Some(pts) = buffer.pts() else {
            return;
        };
        let pts = pts.nseconds();
        let frame_duration = buffer
            .duration()
            .map(|duration| duration.nseconds())
            .filter(|duration| *duration > 0)
            .or_else(|| {
                let info = gst_video::VideoInfo::from_caps(sample.caps()?).ok()?;
                let fps = info.fps();
                if fps.numer() <= 0 || fps.denom() <= 0 {
                    return None;
                }
                Some(1_000_000_000_u64.saturating_mul(fps.denom() as u64) / fps.numer() as u64)
            });
        let frame_end = pts.saturating_add(frame_duration.unwrap_or(1));
        if pts >= end || frame_end <= start {
            return;
        }

        let bytes = buffer.size();
        if let Some(replaced) = self.frames.insert(
            pts,
            CachedFrame {
                end_ns: frame_end,
                duration_ns: frame_duration,
                sample,
                bytes,
            },
        ) {
            self.bytes = self.bytes.saturating_sub(replaced.bytes);
        }
        self.bytes = self.bytes.saturating_add(bytes);
        self.recalculate_byte_limit();
        while self
            .byte_limit
            .is_some_and(|byte_limit| self.bytes > byte_limit)
        {
            let Some((_, frame)) = self.frames.pop_first() else {
                break;
            };
            self.bytes = self.bytes.saturating_sub(frame.bytes);
            self.recalculate_byte_limit();
        }
    }

    fn recalculate_byte_limit(&mut self) {
        let measured_duration_ns = self
            .frames
            .values()
            .filter_map(|frame| frame.duration_ns)
            .fold(0_u64, u64::saturating_add);
        if measured_duration_ns == 0 {
            self.byte_limit = None;
            return;
        }
        let window_ns = PREFETCH_BEHIND.saturating_add(PREFETCH_AHEAD).as_nanos();
        let projected_bytes = (self.bytes as u128)
            .saturating_mul(window_ns)
            .div_ceil(measured_duration_ns as u128)
            .min(usize::MAX as u128) as usize;
        let largest_frame = self
            .frames
            .values()
            .map(|frame| frame.bytes)
            .max()
            .unwrap_or(0);
        self.byte_limit = Some(projected_bytes.max(largest_frame));
    }
}

#[derive(Debug)]
struct CachedFrame {
    end_ns: u64,
    duration_ns: Option<u64>,
    sample: gst::Sample,
    bytes: usize,
}

#[derive(Debug)]
struct PrefetchController {
    requests: Option<mpsc::Sender<PrefetchPlan>>,
    worker: Option<thread::JoinHandle<()>>,
}

impl PrefetchController {
    fn new(uri: &Url, frame_cache: Arc<Mutex<FrameCache>>) -> Result<Self, String> {
        let decoder = gst::ElementFactory::make("uridecodebin")
            .property("uri", uri.as_str())
            .build()
            .map_err(|error| format!("could not create preview decoder: {error}"))?;
        let queue = gst::ElementFactory::make("queue")
            .build()
            .map_err(|error| format!("could not create preview queue: {error}"))?;
        let converter = gst::ElementFactory::make("videoconvert")
            .build()
            .map_err(|error| format!("could not create preview converter: {error}"))?;
        let scaler = gst::ElementFactory::make("videoscale")
            .property("add-borders", true)
            .build()
            .map_err(|error| format!("could not create preview scaler: {error}"))?;
        let caps = gst_video::VideoCapsBuilder::new()
            .format(gst_video::VideoFormat::Nv12)
            .width(960)
            .pixel_aspect_ratio(gst::Fraction::new(1, 1))
            .build();
        let sink = gst_app::AppSink::builder()
            .name("opencut_scrub_prefetch")
            .sync(false)
            .drop(true)
            .max_buffers(3)
            .enable_last_sample(false)
            .caps(&caps)
            .build();
        sink.set_callbacks(
            gst_app::AppSinkCallbacks::builder()
                .new_sample({
                    let frame_cache = frame_cache.clone();
                    move |sink| {
                        let sample = sink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                        frame_cache.lock().insert(sample);
                        Ok(gst::FlowSuccess::Ok)
                    }
                })
                .new_preroll({
                    let frame_cache = frame_cache.clone();
                    move |sink| {
                        let sample = sink.pull_preroll().map_err(|_| gst::FlowError::Eos)?;
                        frame_cache.lock().insert(sample);
                        Ok(gst::FlowSuccess::Ok)
                    }
                })
                .build(),
        );

        let pipeline = gst::Pipeline::new();
        pipeline
            .add_many([&decoder, &queue, &converter, &scaler, sink.upcast_ref()])
            .map_err(|error| format!("could not assemble preview pipeline: {error}"))?;
        gst::Element::link_many([&queue, &converter, &scaler, sink.upcast_ref()])
            .map_err(|error| format!("could not link preview pipeline: {error}"))?;
        let queue_sink = queue
            .static_pad("sink")
            .ok_or_else(|| "preview queue has no sink pad".to_string())?;
        decoder.connect_pad_added(move |_, pad| {
            if queue_sink.is_linked() {
                return;
            }
            let caps = pad.current_caps().unwrap_or_else(|| pad.query_caps(None));
            let is_video = caps
                .structure(0)
                .is_some_and(|structure| structure.name().starts_with("video/"));
            if is_video {
                let _ = pad.link(&queue_sink);
            }
        });
        pipeline
            .set_state(gst::State::Paused)
            .map_err(|error| format!("could not prepare preview pipeline: {error}"))?;
        if let Err(error) = pipeline.state(gst::ClockTime::from_seconds(5)).0 {
            let _ = pipeline.set_state(gst::State::Null);
            return Err(format!(
                "preview pipeline did not finish preparing: {error}"
            ));
        }
        let bus = pipeline
            .bus()
            .ok_or_else(|| "preview pipeline has no message bus".to_string())?;

        let (requests, receiver) = mpsc::channel::<PrefetchPlan>();
        let worker = thread::Builder::new()
            .name("video-preview-prefetch".to_string())
            .spawn(move || {
                let mut pending = VecDeque::<PrefetchRequest>::new();
                let mut active = false;
                loop {
                    if !active && let Some(request) = pending.pop_front() {
                        let result = pipeline.seek(
                            1.0,
                            gst::SeekFlags::FLUSH
                                | gst::SeekFlags::ACCURATE
                                | gst::SeekFlags::SEGMENT,
                            gst::SeekType::Set,
                            gst::ClockTime::from_nseconds(duration_ns(request.start)),
                            gst::SeekType::Set,
                            gst::ClockTime::from_nseconds(duration_ns(request.end)),
                        );
                        if let Err(error) = result {
                            log::error!("could not seek preview prefetch pipeline: {error}");
                            continue;
                        }
                        if let Err(error) = pipeline.set_state(gst::State::Playing) {
                            log::error!("could not start preview prefetch pipeline: {error}");
                            continue;
                        }
                        active = true;
                    }

                    let received = if active || !pending.is_empty() {
                        match receiver.recv_timeout(Duration::from_millis(5)) {
                            Ok(plan) => Some(plan),
                            Err(mpsc::RecvTimeoutError::Timeout) => None,
                            Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        }
                    } else {
                        match receiver.recv() {
                            Ok(plan) => Some(plan),
                            Err(_) => break,
                        }
                    };
                    if let Some(mut plan) = received {
                        while let Ok(newer_plan) = receiver.try_recv() {
                            plan = newer_plan;
                        }
                        plan.replace(&mut pending);
                        active = false;
                        continue;
                    }

                    let Some(message) = bus.timed_pop_filtered(
                        gst::ClockTime::ZERO,
                        &[
                            gst::MessageType::SegmentDone,
                            gst::MessageType::Eos,
                            gst::MessageType::Error,
                        ],
                    ) else {
                        continue;
                    };
                    match message.view() {
                        gst::MessageView::SegmentDone(..) | gst::MessageView::Eos(..) => {
                            active = false;
                        }
                        gst::MessageView::Error(error) => {
                            log::error!(
                                "preview prefetch pipeline failed: {}{}",
                                error.error(),
                                error
                                    .debug()
                                    .map(|debug| format!(" ({debug})"))
                                    .unwrap_or_default()
                            );
                            active = false;
                            pending.clear();
                        }
                        _ => {}
                    }
                }
                if let Err(error) = pipeline.set_state(gst::State::Null) {
                    log::error!("could not stop preview prefetch pipeline: {error}");
                }
            })
            .map_err(|error| format!("could not start preview prefetch worker: {error}"))?;

        Ok(Self {
            requests: Some(requests),
            worker: Some(worker),
        })
    }

    fn request(&self, plan: PrefetchPlan) {
        if plan.leading.is_none() && plan.trailing.is_none() {
            return;
        }
        self.requests
            .as_ref()
            .expect("prefetch sender must exist while its controller is alive")
            .send(plan)
            .expect("prefetch worker must outlive its controller");
    }
}

impl Drop for PrefetchController {
    fn drop(&mut self) {
        drop(self.requests.take());
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            log::error!("preview prefetch worker panicked");
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct PrefetchPlan {
    leading: Option<PrefetchRequest>,
    trailing: Option<PrefetchRequest>,
}

impl PrefetchPlan {
    fn replace(self, pending: &mut VecDeque<PrefetchRequest>) {
        pending.clear();
        if let Some(request) = self.trailing {
            pending.push_back(request);
        }
        if let Some(request) = self.leading {
            pending.push_back(request);
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct PrefetchRequest {
    start: Duration,
    end: Duration,
}

#[cfg(test)]
#[path = "video_backend.test.rs"]
mod tests;
