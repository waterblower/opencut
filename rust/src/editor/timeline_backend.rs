//! Blocking timeline frame preparation without playback or UI state.
//!
//! The backend owns its decoding worker. Blocking open and seek wait for that
//! worker; snapshot reads never decode or advance time.
//! Layers are prepared for a renderer to compose over a black canvas; text is
//! retained as text so the renderer can perform font shaping and layout.

use crate::editor::preview_timeline::TimelinePreviewFrame;
use anyhow::{Context as _, Result, bail};
use image::RgbaImage;
use opencut_player::engine::{decode::VideoReader, raster::load_image};
use opencut_player::timeline::{
    Clip, MediaKind, TextClipProperties, TimelineSerialization, TimelineTime, TrackKind,
    VideoClipProperties,
};
use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};
use ulid::Ulid;

/// An immutable snapshot. Layers are ordered from bottom to top.
#[derive(Clone, Debug)]
pub struct TimelineFrame {
    pub timestamp: Duration,
    pub width: u32,
    pub height: u32,
    pub layers: Vec<TimelineLayer>,
}

/// Video and image pixels are unpremultiplied RGBA in source dimensions.
/// Visual properties retain their document units; no transforms are baked in.
#[derive(Clone, Debug)]
pub enum TimelineLayer {
    Video {
        clip_id: Ulid,
        pixels: Arc<RgbaImage>,
        properties: VideoClipProperties,
    },
    Image {
        clip_id: Ulid,
        pixels: Arc<RgbaImage>,
        properties: VideoClipProperties,
    },
    Text {
        clip_id: Ulid,
        properties: TextClipProperties,
    },
}

pub struct TimelineBackend {
    timeline: Arc<TimelineSerialization>,
    commands: mpsc::Sender<SeekRequest>,
    frame: Arc<Mutex<Arc<TimelineFrame>>>,
}

impl TimelineBackend {
    /// Creates a preview backend at position zero without waiting for media I/O.
    /// Initialization failures are returned by `preview_frame`.
    pub fn new(_timeline: TimelineSerialization, _media_root: &Path) -> Self {
        unimplemented!("start the internal worker without waiting for its first frame")
    }

    pub fn timeline(&self) -> &TimelineSerialization {
        &self.timeline
    }

    /// Edits the authoritative document. Copy-on-write preserves worker snapshots;
    /// requests identify cached frames by the snapshot they were prepared from.
    pub fn timeline_mut(&mut self) -> &mut TimelineSerialization {
        Arc::make_mut(&mut self.timeline)
    }

    /// Requests the given timeline frame and immediately returns its cached
    /// presentation, or `None` while the internal worker prepares it.
    pub fn preview_frame(
        &self,
        _position: TimelineTime,
    ) -> Result<Option<Arc<TimelinePreviewFrame>>> {
        unimplemented!("schedule and cache still frames on the internal worker")
    }

    /// Starts an internal worker and waits for position zero to be prepared.
    /// Relative asset paths are resolved against `media_root`.
    pub fn open_sync(timeline: TimelineSerialization, media_root: &Path) -> Result<Self> {
        validate(&timeline)?;
        let timeline = Arc::new(timeline);
        let worker_timeline = Arc::clone(&timeline);
        let media_root = media_root.to_owned();
        let (commands, requests) = mpsc::channel::<SeekRequest>();
        let (ready, initialized) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("timeline-preview".into())
            .spawn(move || {
                let mut decoder = match FrameDecoder::open_sync(worker_timeline, &media_root) {
                    Ok(decoder) => decoder,
                    Err(error) => {
                        let _ = ready.send(Err(error));
                        return;
                    }
                };
                let frame = Arc::new(Mutex::new(Arc::clone(&decoder.frame)));
                if ready.send(Ok(Arc::clone(&frame))).is_err() {
                    return;
                }
                // Dropping the backend closes the command channel. Cleanup stays
                // on this worker; there is no thread join in the caller's Drop.
                for request in requests {
                    let result = (|| {
                        if !Arc::ptr_eq(&decoder.timeline, &request.timeline) {
                            validate(&request.timeline)?;
                            decoder = FrameDecoder::open_sync(request.timeline, &media_root)?;
                        }
                        decoder.seek_sync(request.position)?;
                        *frame.lock().unwrap() = Arc::clone(&decoder.frame);
                        Ok(())
                    })();
                    let _ = request.reply.send(result);
                }
            })
            .context("Starting timeline preview worker")?;
        let frame = initialized
            .recv()
            .context("Waiting for timeline initialization")??;
        Ok(Self {
            timeline,
            commands,
            frame,
        })
    }

    pub fn frame_size(&self) -> (u32, u32) {
        (self.timeline.settings.width, self.timeline.settings.height)
    }

    pub fn framerate(&self) -> Option<f64> {
        Some(self.timeline.settings.frame_rate.frames_per_second())
    }

    /// Includes audio clips and invisible tracks in the document's duration.
    pub fn duration(&self) -> Duration {
        self.timeline.duration(self.timeline.content_duration())
    }

    pub fn position(&self) -> Duration {
        self.frame.lock().unwrap().timestamp
    }

    /// Waits until the worker publishes the requested frame. A failed seek
    /// preserves the previous snapshot and position.
    pub fn seek_sync(&mut self, position: Duration) -> Result<()> {
        let (reply, completion) = mpsc::channel();
        self.commands
            .send(SeekRequest {
                timeline: Arc::clone(&self.timeline),
                position,
                reply,
            })
            .context("Requesting timeline seek")?;
        completion.recv().context("Waiting for timeline seek")?
    }

    /// Clones a snapshot handle without performing I/O. Older snapshots remain
    /// valid after later seeks and after this backend is dropped.
    pub fn get_current_frame(&self) -> Result<Arc<TimelineFrame>> {
        Ok(Arc::clone(&self.frame.lock().unwrap()))
    }
}

struct SeekRequest {
    timeline: Arc<TimelineSerialization>,
    position: Duration,
    reply: mpsc::Sender<Result<()>>,
}

struct FrameDecoder {
    timeline: Arc<TimelineSerialization>,
    media_root: PathBuf,
    readers: HashMap<Ulid, VideoReader>,
    images: HashMap<Ulid, Arc<RgbaImage>>,
    frame: Arc<TimelineFrame>,
}

impl FrameDecoder {
    /// Owns an immutable document and returns with position zero prepared.
    /// Relative asset paths are resolved against `media_root`.
    fn open_sync(timeline: Arc<TimelineSerialization>, media_root: &Path) -> Result<Self> {
        let frame = Arc::new(TimelineFrame {
            timestamp: Duration::ZERO,
            width: timeline.settings.width,
            height: timeline.settings.height,
            layers: Vec::new(),
        });
        let mut backend = Self {
            timeline,
            media_root: media_root.to_owned(),
            readers: HashMap::new(),
            images: HashMap::new(),
            frame,
        };
        backend.seek_sync(Duration::ZERO)?;
        Ok(backend)
    }

    /// Snaps to the nearest timeline frame and clamps to the last valid frame.
    /// Empty timelines remain at zero. A failed seek preserves the previous
    /// snapshot and position, and can be retried after the media is repaired.
    fn seek_sync(&mut self, position: Duration) -> Result<()> {
        let last =
            (self.timeline.content_duration() - TimelineTime::ONE_FRAME).max(TimelineTime::ZERO);
        let position = self
            .timeline
            .settings
            .frame_rate
            .frames_from_duration_nearest(position)
            .clamp(TimelineTime::ZERO, last);
        let frame = self.prepare(position)?;
        self.frame = Arc::new(frame);
        Ok(())
    }

    fn prepare(&mut self, position: TimelineTime) -> Result<TimelineFrame> {
        let mut layers = Vec::new();
        // The first document track is the top track. Within a track, later
        // document clips paint over earlier ones.
        for track in self.timeline.tracks.iter().rev() {
            if !track.visible || track.kind == TrackKind::Audio {
                continue;
            }
            for clip in self.timeline.clips_on_track(track.id) {
                if position < clip.timeline_start()
                    || position >= clip.timeline_end(self.timeline.settings.frame_rate)
                {
                    continue;
                }
                match clip {
                    Clip::Audio(_) => {}
                    Clip::Text(clip) => layers.push(TimelineLayer::Text {
                        clip_id: clip.id,
                        properties: clip.properties.clone(),
                    }),
                    Clip::Video(media) => {
                        // Document references were checked during open.
                        let asset = self.timeline.asset(media.asset_id).unwrap();
                        let path = self.media_root.join(&asset.path);
                        match asset.kind {
                            MediaKind::Video => {
                                if let Entry::Vacant(entry) = self.readers.entry(asset.id) {
                                    let reader = VideoReader::open(&path).context(format!(
                                        "Opening timeline video {}",
                                        path.display()
                                    ))?;
                                    entry.insert(reader);
                                }
                                let source = self.timeline.source_position_at(clip, position);
                                let result = self
                                    .readers
                                    .get_mut(&asset.id)
                                    .unwrap()
                                    .at(source.as_secs_f64());
                                let pixels = match result {
                                    Ok(pixels) => pixels,
                                    Err(error) => {
                                        // A failed decoder may be partially advanced. Reopen it
                                        // on retry instead of reusing that uncertain state.
                                        self.readers.remove(&asset.id);
                                        return Err(error).context(format!(
                                            "Preparing timeline clip {} from {} at {:.6}s",
                                            media.id,
                                            path.display(),
                                            source.as_secs_f64(),
                                        ));
                                    }
                                };
                                layers.push(TimelineLayer::Video {
                                    clip_id: media.id,
                                    pixels: Arc::new(pixels),
                                    properties: media.video_properties,
                                });
                            }
                            MediaKind::Image => {
                                if let Entry::Vacant(entry) = self.images.entry(asset.id) {
                                    let pixels = load_image(&path).context(format!(
                                        "Loading timeline image {}",
                                        path.display()
                                    ))?;
                                    entry.insert(Arc::new(pixels));
                                }
                                layers.push(TimelineLayer::Image {
                                    clip_id: media.id,
                                    pixels: Arc::clone(self.images.get(&asset.id).unwrap()),
                                    properties: media.video_properties,
                                });
                            }
                            MediaKind::Audio => unreachable!("visual assets were validated"),
                        }
                    }
                }
            }
        }
        Ok(TimelineFrame {
            timestamp: self.timeline.duration(position),
            width: self.timeline.settings.width,
            height: self.timeline.settings.height,
            layers,
        })
    }
}

fn validate(timeline: &TimelineSerialization) -> Result<()> {
    let settings = timeline.settings;
    if settings.width == 0 || settings.height == 0 {
        bail!("Timeline canvas dimensions must be positive");
    }
    if settings.frame_rate.numerator == 0 || settings.frame_rate.denominator == 0 {
        bail!("Timeline frame rate must have a positive numerator and denominator");
    }
    if settings.audio_sample_rate == 0 {
        bail!("Timeline audio sample rate must be positive");
    }
    let mut track_ids = HashSet::new();
    for track in &timeline.tracks {
        if !track_ids.insert(track.id) {
            bail!("Duplicate timeline track {}", track.id);
        }
    }
    let mut asset_ids = HashSet::new();
    for asset in &timeline.assets {
        if !asset_ids.insert(asset.id) {
            bail!("Duplicate timeline asset {}", asset.id);
        }
    }
    let mut clip_ids = HashSet::new();
    for clip in &timeline.clips {
        if matches!(clip, Clip::Audio(_)) {
            continue;
        }
        if !clip_ids.insert(clip.id()) {
            bail!("Duplicate visual clip {}", clip.id());
        }
        let Some(track) = timeline.track(clip.track_id()) else {
            bail!(
                "Visual clip {} references missing track {}",
                clip.id(),
                clip.track_id()
            );
        };
        if clip.timeline_start() < TimelineTime::ZERO
            || clip.frame_length(settings.frame_rate) <= TimelineTime::ZERO
        {
            bail!("Visual clip {} has an invalid time range", clip.id());
        }
        match clip {
            Clip::Video(media) => {
                if track.kind != TrackKind::Video {
                    bail!("Video clip {} requires a video track", media.id);
                }
                let Some(asset) = timeline.asset(media.asset_id) else {
                    bail!(
                        "Visual clip {} references missing asset {}",
                        media.id,
                        media.asset_id
                    );
                };
                if asset.kind == MediaKind::Audio {
                    bail!(
                        "Visual clip {} references audio asset {}",
                        media.id,
                        asset.id
                    );
                }
                if media.source_in < TimelineTime::ZERO {
                    bail!("Visual clip {} has a negative source trim", media.id);
                }
                let properties = media.video_properties;
                if !properties.position_x.is_finite()
                    || !properties.position_y.is_finite()
                    || !properties.scale.is_finite()
                    || properties.scale < 0.0
                {
                    bail!("Visual clip {} has invalid transform properties", media.id);
                }
            }
            Clip::Text(text) => {
                if track.kind != TrackKind::Text {
                    bail!("Text clip {} requires a text track", text.id);
                }
                let properties = &text.properties;
                if !properties.position_x.is_finite()
                    || !properties.position_y.is_finite()
                    || !properties.font_size.is_finite()
                    || properties.font_size <= 0.0
                {
                    bail!("Text clip {} has invalid layout properties", text.id);
                }
            }
            Clip::Audio(_) => {}
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "tests/timeline_backend.test.rs"]
mod tests;
