//! Synchronous timeline frame preparation. Callers own scheduling and published snapshots.

use crate::{image::load_image, video_frame::frame_to_rgba};
use anyhow::{Context as _, Result};
use image::RgbaImage;
use media_backend::{VideoBackend, VideoDecoder};
use std::{
    collections::{HashMap, hash_map::Entry},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};
use timeline::{
    Clip, MediaKind, TextClipProperties, TimelineEditingState, TimelineTime, TrackKind,
    VideoClipProperties,
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

pub struct TimelineDecoder {
    project_root: PathBuf,
    readers: HashMap<Ulid, VideoDecoder>,
    scaler: Option<ffmpeg_next::software::scaling::Context>,
    images: HashMap<Ulid, Arc<RgbaImage>>,
}

impl TimelineDecoder {
    pub fn new(project_root: &Path) -> Self {
        Self {
            project_root: project_root.to_owned(),
            readers: HashMap::new(),
            scaler: None,
            images: HashMap::new(),
        }
    }

    /// Snaps to the nearest timeline frame and clamps to the last valid frame.
    /// Empty timelines remain at zero. Returned frames own their pixels and survive later calls.
    /// Recreate the decoder when asset IDs are rebound to different media.
    pub fn frame_at(
        &mut self,
        timeline: &TimelineEditingState,
        position: Duration,
    ) -> Result<TimelineFrame> {
        timeline.validate()?;
        let last = (timeline.content_duration() - TimelineTime::ONE_FRAME).max(TimelineTime::ZERO);
        let position = timeline
            .settings
            .frame_rate
            .frames_from_duration_nearest(position)
            .clamp(TimelineTime::ZERO, last);
        self.prepare(timeline, position)
    }

    fn prepare(
        &mut self,
        timeline: &TimelineEditingState,
        position: TimelineTime,
    ) -> Result<TimelineFrame> {
        let mut layers = Vec::new();
        // The first document track is the top track. Within a track, later
        // document clips paint over earlier ones.
        for track in timeline.tracks.iter().rev() {
            if !track.visible || track.kind == TrackKind::Audio {
                continue;
            }
            for clip in timeline.clips_on_track(track.id) {
                if position < clip.timeline_start()
                    || position >= clip.timeline_end(timeline.settings.frame_rate)
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
                        // Document references were checked by frame_at.
                        let asset = timeline.asset(media.asset_id).unwrap();
                        let path = self.project_root.join(&asset.path);
                        match asset.kind {
                            MediaKind::Video => {
                                if let Entry::Vacant(entry) = self.readers.entry(asset.id) {
                                    let reader = (|| {
                                        ffmpeg_next::init()?;
                                        let metadata = VideoBackend::probe(&path)?;
                                        VideoDecoder::open(
                                            &path,
                                            metadata.video.stream_index,
                                            metadata.origin_microseconds,
                                        )
                                    })()
                                    .context(format!(
                                        "Opening timeline video {}",
                                        path.display()
                                    ))?;
                                    entry.insert(reader);
                                }
                                let source = timeline.source_position_at(clip, position);
                                let result = (|| {
                                    let reader = self.readers.get_mut(&asset.id).unwrap();
                                    reader.seek(source)?;
                                    let frame = reader
                                        .next_frame()?
                                        .context("Video has no frame after seeking")?;
                                    frame_to_rgba(&frame, &mut self.scaler)
                                })();
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
            timestamp: timeline.duration(position),
            width: timeline.settings.width,
            height: timeline.settings.height,
            layers,
        })
    }
}
