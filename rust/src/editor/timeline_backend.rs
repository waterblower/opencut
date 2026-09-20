//! Timeline frame preparation with an owned playhead and no playback or UI state.
//!
//! The backend owns its decoding worker. Blocking open and seek wait for that
//! worker; snapshot reads never decode or advance time.
//! Layers are prepared for a renderer to compose over a black canvas; text is
//! retained as text so the renderer can perform font shaping and layout.

use crate::editor::preview_timeline::TimelinePreviewFrame;
use anyhow::{Context as _, Error, Result, anyhow, bail};
use image::RgbaImage;
use opencut_player::engine::{decode::VideoReader, raster::load_image};
use opencut_player::timeline::{
    Clip, MediaKind, TextClipProperties, TimelineEditingState, TimelineTime, TrackKind,
    VideoClipProperties,
};
use std::{
    collections::{HashMap, hash_map::Entry},
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
    timeline: Arc<TimelineEditingState>,
    revision: u64,
    commands: mpsc::Sender<SeekRequest>,
    frame: Arc<Mutex<Arc<TimelineFrame>>>,
    preview: Arc<Mutex<PreviewRequest>>,
}

impl TimelineBackend {
    /// Validates the document and media root, then starts preparing frame zero.
    /// Returns validation and worker-start errors immediately. Media decoding
    /// stays on the worker; its failures are returned by `preview_frame`.
    pub fn new(timeline: TimelineEditingState, media_root: &Path) -> Result<Self> {
        let metadata = media_root.metadata().context(format!(
            "Inspecting timeline media root {}",
            media_root.display()
        ))?;
        if !metadata.is_dir() {
            bail!(
                "Timeline media root is not a directory: {}",
                media_root.display()
            );
        }
        timeline.validate()?;
        let timeline = Arc::new(timeline);
        let frame = Arc::new(Mutex::new(Arc::new(TimelineFrame {
            timestamp: Duration::ZERO,
            width: timeline.settings.width,
            height: timeline.settings.height,
            layers: Vec::new(),
        })));
        let preview = Arc::new(Mutex::new(PreviewRequest {
            revision: 0,
            position: Duration::ZERO,
            result: None,
        }));
        let (commands, requests) = mpsc::channel::<SeekRequest>();
        let backend = Self {
            timeline,
            revision: 0,
            commands,
            frame,
            preview,
        };
        let media_root = media_root.to_owned();
        let frame = Arc::clone(&backend.frame);
        let preview = Arc::clone(&backend.preview);
        thread::Builder::new()
            .name("timeline-preview".into())
            .spawn(move || {
                let mut decoder: Option<(u64, FrameDecoder)> = None;
                // Dropping the backend closes the channel; decoder cleanup stays here.
                for request in requests {
                    if request.reply.is_none() {
                        let preview = preview.lock().unwrap();
                        if preview.revision != request.revision
                            || preview.position != request.position
                        {
                            continue;
                        }
                    }
                    let result = (|| -> Result<Arc<TimelinePreviewFrame>> {
                        if decoder
                            .as_ref()
                            .is_none_or(|(revision, _)| *revision != request.revision)
                        {
                            request.timeline.validate()?;
                            decoder = Some((
                                request.revision,
                                FrameDecoder::new(Arc::clone(&request.timeline), &media_root),
                            ));
                        }
                        let (_, decoder) = decoder.as_mut().unwrap();
                        decoder.seek_sync(request.position)?;
                        let prepared =
                            Arc::new(TimelinePreviewFrame::new(Arc::clone(&decoder.frame)));
                        Ok(prepared)
                    })();
                    let result = match result {
                        Ok(prepared) => Ok(prepared),
                        Err(error) => Err(Arc::new(error)),
                    };
                    {
                        let mut preview = preview.lock().unwrap();
                        if preview.revision == request.revision
                            && preview.position == request.position
                        {
                            if let Ok(prepared) = &result {
                                *frame.lock().unwrap() = Arc::clone(&prepared.frame);
                            }
                            preview.result = Some(result.clone());
                        }
                    }
                    if let Some(reply) = request.reply {
                        let _ = reply.send(result);
                    }
                }
            })
            .context("Starting timeline preview worker")?;
        backend
            .commands
            .send(SeekRequest {
                timeline: Arc::clone(&backend.timeline),
                revision: backend.revision,
                position: Duration::ZERO,
                reply: None,
            })
            .context("Requesting initial timeline frame")?;
        Ok(backend)
    }

    pub fn timeline(&self) -> &TimelineEditingState {
        &self.timeline
    }

    /// Commits validated content and requests a frame at the current playhead.
    /// Rejection preserves content, position, and cached frames. Worker snapshots
    /// keep their previous content until decoding completes.
    pub fn replace_timeline(&mut self, timeline: TimelineEditingState) -> Result<()> {
        timeline.validate()?;
        let mut preview = self.preview.lock().unwrap();
        let position = clamp_position(&timeline, preview.position)?;
        let revision = self.revision.wrapping_add(1);
        let timeline = Arc::new(timeline);
        self.commands
            .send(SeekRequest {
                timeline: Arc::clone(&timeline),
                revision,
                position,
                reply: None,
            })
            .context("Requesting edited timeline frame")?;
        self.timeline = timeline;
        self.revision = revision;
        *preview = PreviewRequest {
            revision,
            position,
            result: None,
        };
        Ok(())
    }

    /// Moves the playhead immediately and prepares its frame on the worker.
    /// Snaps and clamps to the timeline; repeated requests reuse the cached result.
    /// Decode failures preserve the last successful snapshot, not the old playhead.
    pub fn seek(&self, position: Duration) -> Result<()> {
        let position = clamp_position(&self.timeline, position)?;
        let mut preview = self.preview.lock().unwrap();
        if preview.revision != self.revision || preview.position != position {
            self.commands
                .send(SeekRequest {
                    timeline: Arc::clone(&self.timeline),
                    revision: self.revision,
                    position,
                    reply: None,
                })
                .context("Requesting timeline preview frame")?;
            *preview = PreviewRequest {
                revision: self.revision,
                position,
                result: None,
            };
        }
        Ok(())
    }

    /// Requests the given timeline frame and immediately returns its cached
    /// presentation, or `None` while the internal worker prepares it.
    pub fn preview_frame(
        &self,
        position: TimelineTime,
    ) -> Result<Option<Arc<TimelinePreviewFrame>>> {
        self.seek(self.timeline.duration(position))?;
        let preview = self.preview.lock().unwrap();
        match &preview.result {
            Some(Ok(frame)) => Ok(Some(Arc::clone(frame))),
            Some(Err(error)) => Err(anyhow!("{error:?}")),
            None => Ok(None),
        }
    }

    /// Starts an internal worker and waits for position zero to be prepared.
    /// Relative asset paths are resolved against `media_root`.
    pub fn open_sync(timeline: TimelineEditingState, media_root: &Path) -> Result<Self> {
        let mut backend = Self::new(timeline, media_root)?;
        backend.seek_sync(Duration::ZERO)?;
        Ok(backend)
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

    /// The requested playhead, independent of decoding progress or failures.
    pub fn position(&self) -> Duration {
        self.preview.lock().unwrap().position
    }

    /// Waits until the worker publishes the requested frame. A failed seek
    /// preserves the previous snapshot and position.
    pub fn seek_sync(&mut self, position: Duration) -> Result<()> {
        let position = clamp_position(&self.timeline, position)?;
        let (reply, completion) = mpsc::channel();
        self.commands
            .send(SeekRequest {
                timeline: Arc::clone(&self.timeline),
                revision: self.revision,
                position,
                reply: Some(reply),
            })
            .context("Requesting timeline seek")?;
        let prepared = match completion.recv().context("Waiting for timeline seek")? {
            Ok(prepared) => prepared,
            Err(error) => return Err(anyhow!("{error:?}")),
        };
        let mut preview = self.preview.lock().unwrap();
        *self.frame.lock().unwrap() = Arc::clone(&prepared.frame);
        *preview = PreviewRequest {
            revision: self.revision,
            position,
            result: Some(Ok(prepared)),
        };
        Ok(())
    }

    /// Clones a snapshot handle without performing I/O. Older snapshots remain
    /// valid after later seeks and after this backend is dropped.
    pub fn get_current_frame(&self) -> Result<Arc<TimelineFrame>> {
        Ok(Arc::clone(&self.frame.lock().unwrap()))
    }
}

struct SeekRequest {
    timeline: Arc<TimelineEditingState>,
    revision: u64,
    position: Duration,
    reply: Option<mpsc::Sender<Result<Arc<TimelinePreviewFrame>, Arc<Error>>>>,
}

struct PreviewRequest {
    revision: u64,
    position: Duration,
    result: Option<Result<Arc<TimelinePreviewFrame>, Arc<Error>>>,
}

struct FrameDecoder {
    timeline: Arc<TimelineEditingState>,
    media_root: PathBuf,
    readers: HashMap<Ulid, VideoReader>,
    images: HashMap<Ulid, Arc<RgbaImage>>,
    frame: Arc<TimelineFrame>,
}

impl FrameDecoder {
    fn new(timeline: Arc<TimelineEditingState>, media_root: &Path) -> Self {
        let frame = Arc::new(TimelineFrame {
            timestamp: Duration::ZERO,
            width: timeline.settings.width,
            height: timeline.settings.height,
            layers: Vec::new(),
        });
        Self {
            timeline,
            media_root: media_root.to_owned(),
            readers: HashMap::new(),
            images: HashMap::new(),
            frame,
        }
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

fn clamp_position(timeline: &TimelineEditingState, position: Duration) -> Result<Duration> {
    let rate = timeline.settings.frame_rate;
    if rate.numerator == 0 || rate.denominator == 0 {
        bail!("Timeline frame rate must have a positive numerator and denominator");
    }
    let last = (timeline.content_duration() - TimelineTime::ONE_FRAME).max(TimelineTime::ZERO);
    let position = rate
        .frames_from_duration_nearest(position)
        .clamp(TimelineTime::ZERO, last);
    Ok(rate.duration(position))
}

#[cfg(test)]
#[path = "tests/timeline_backend.test.rs"]
mod tests;
