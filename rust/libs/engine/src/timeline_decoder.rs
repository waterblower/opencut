//! Timeline frame preparation for playback and scrubbing. Each video clip keeps decoding
//! forward and only seeks on backward or distant jumps.

#[cfg(target_os = "macos")]
use crate::gpu::GpuResources;
use crate::image::load_image;
use anyhow::{Context as _, Result, bail};
#[cfg(target_os = "macos")]
use core_video::pixel_buffer::CVPixelBuffer;
use gpui::RenderImage;
use image::{Frame, RgbaImage};
use media_backend::{VideoBackend, VideoDecoder, VideoFrame};
use smallvec::smallvec;
use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use timeline::{
    Clip, MediaKind, TimelineEditingState, TimelineFrameIndex, TrackKind, VideoClipProperties,
};
use ulid::Ulid;

const FORWARD_DECODE_LIMIT: i64 = 1_000_000; // 目标超出当前帧 1 秒以上时改为 seek，而不是逐帧解码过去。
const TIMESTAMP_TOLERANCE: i64 = 1_000; // 吸收源帧 PTS 与目标时间的微秒取整误差。

pub struct TimelineFrameComposition {
    pub frame_index: TimelineFrameIndex, // 限制在时间线有效帧范围内。
    pub timestamp: Duration,
    pub width: u32,
    pub height: u32,
    pub layers: Vec<PreparedLayer>, // 按绘制顺序排列：底层在前，顶层在后。
}

pub enum PreparedLayer {
    #[cfg(target_os = "macos")]
    VideoFrame {
        clip_id: Ulid,
        frame: CVPixelBuffer, // GPU surface；源帧未变化时复用。
        properties: VideoClipProperties,
    },
    Image {
        clip_id: Ulid,
        image: Arc<RenderImage>, // 按素材缓存，只准备一次。
        properties: VideoClipProperties,
    },
    Text {
        clip_id: Ulid,
    },
}

pub struct TimelineDecoder {
    timeline_directory: PathBuf,
    #[cfg(target_os = "macos")]
    readers: HashMap<Ulid, ClipReader>, // 按 clip 而非素材区分：同一素材的重叠 clip 各自前进。
    images: HashMap<Ulid, Arc<RenderImage>>,
}

impl TimelineDecoder {
    pub fn new(timeline_directory: &Path) -> Self {
        Self {
            timeline_directory: timeline_directory.to_owned(),
            #[cfg(target_os = "macos")]
            readers: HashMap::new(),
            images: HashMap::new(),
        }
    }

    /// The caller validates the timeline. Positions past the end clamp to the last frame.
    pub fn frame_at(
        &mut self,
        timeline: &TimelineEditingState,
        position: TimelineFrameIndex,
    ) -> Result<TimelineFrameComposition> {
        let last_frame_index = (timeline.content_duration() - TimelineFrameIndex::ONE_FRAME)
            .max(TimelineFrameIndex::ZERO);
        let position = position.clamp(TimelineFrameIndex::ZERO, last_frame_index);
        let mut layers: Vec<PreparedLayer> = Vec::new();
        #[cfg(target_os = "macos")]
        let mut active_readers: HashSet<Ulid> = HashSet::new();
        // The first document track is the top track. Within a track, later
        // document clips paint over earlier ones.
        for track in timeline.tracks.iter().rev() {
            if !track.visible || track.kind == TrackKind::Audio {
                continue;
            }

            let clips_at_position = timeline.clips_on_track(track.id).filter(|clip| {
                clip.timeline_start() <= position
                    && position < clip.timeline_end(timeline.settings.frame_rate)
            });
            for clip in clips_at_position {
                match clip {
                    Clip::Audio(_) => {}
                    Clip::Text(clip) => layers.push(PreparedLayer::Text { clip_id: clip.id() }),
                    Clip::Video(media) => {
                        let asset = timeline
                            .asset(media.asset_id)
                            .context("Validated clip references a missing asset")?;
                        let asset_path = self.timeline_directory.join(&asset.path);
                        match asset.kind {
                            MediaKind::Video => {
                                #[cfg(target_os = "macos")]
                                {
                                    let reader = match self.readers.entry(media.id()) {
                                        Entry::Occupied(entry) => entry.into_mut(),
                                        Entry::Vacant(entry) => entry.insert(
                                            ClipReader::open(&asset_path).context(format!(
                                                "Opening timeline video {}",
                                                asset_path.display()
                                            ))?,
                                        ),
                                    };
                                    active_readers.insert(media.id());
                                    let source = timeline.source_position_at(clip, position);
                                    let frame = match reader.picture_at(source) {
                                        Ok(frame) => frame,
                                        Err(error) => {
                                            // A failed decoder may be partially advanced; reopen on retry.
                                            self.readers.remove(&media.id());
                                            return Err(error).context(format!(
                                                "Preparing timeline clip {} from {} at {:.6}s",
                                                media.id(),
                                                asset_path.display(),
                                                source.as_secs_f64(),
                                            ));
                                        }
                                    };
                                    layers.push(PreparedLayer::VideoFrame {
                                        clip_id: media.id(),
                                        frame,
                                        properties: media.video_properties,
                                    });
                                }
                                #[cfg(not(target_os = "macos"))]
                                bail!("Timeline video requires macOS: {}", path.display());
                            }
                            MediaKind::Image => {
                                let image = match self.images.entry(asset.id) {
                                    Entry::Occupied(entry) => Arc::clone(entry.get()),
                                    Entry::Vacant(entry) => {
                                        let pixels = load_image(&asset_path).context(format!(
                                            "Loading timeline image {}",
                                            asset_path.display()
                                        ))?;
                                        Arc::clone(entry.insert(bgra_image(swap_red_blue(pixels))))
                                    }
                                };
                                layers.push(PreparedLayer::Image {
                                    clip_id: media.id(),
                                    image,
                                    properties: media.video_properties,
                                });
                            }
                            MediaKind::Audio => bail!("Visual clip {} uses audio", media.id()),
                        }
                    }
                }
            }
        }
        // Clips outside the current frame release their decoders.
        #[cfg(target_os = "macos")]
        self.readers.retain(|id, _| active_readers.contains(id));
        Ok(TimelineFrameComposition {
            frame_index: position,
            timestamp: timeline.position_at_frame(position),
            width: timeline.settings.width,
            height: timeline.settings.height,
            layers,
        })
    }
}

#[cfg(target_os = "macos")]
struct ClipReader {
    gpu: GpuResources,
    decoder: VideoDecoder,
    next: Option<VideoFrame>,            // 已解码、尚未到展示时间的帧。
    shown: Option<(i64, CVPixelBuffer)>, // (源 PTS 微秒, GPU surface)。
}

#[cfg(target_os = "macos")]
impl ClipReader {
    fn open(path: &Path) -> Result<Self> {
        let metadata = VideoBackend::probe(path)?;
        let decoder = VideoDecoder::open(
            path,
            metadata.video.stream_index,
            metadata.origin_microseconds,
        )?;
        Ok(Self {
            gpu: GpuResources::new((
                metadata.video.width as usize,
                metadata.video.height as usize,
            ))?,
            decoder,
            next: None,
            shown: None,
        })
    }

    /// Latest frame at or before the source position; converts only when the frame changes.
    fn picture_at(&mut self, source: Duration) -> Result<CVPixelBuffer> {
        let target = i64::try_from(source.as_micros()).unwrap_or(i64::MAX);
        let mut selected = None;
        let continues = self.shown.as_ref().is_some_and(|(shown, _)| {
            *shown <= target.saturating_add(TIMESTAMP_TOLERANCE)
                && target - *shown <= FORWARD_DECODE_LIMIT
        });
        if !continues {
            self.decoder.seek(source)?;
            self.next = None;
            selected = Some(
                self.decoder
                    .next_frame()?
                    .context("Video has no frame after seeking")?,
            );
        }
        loop {
            if self.next.is_none() {
                self.next = self.decoder.next_frame()?; // None：EOF，保留最后一帧。
            }
            match &self.next {
                Some(frame) if frame.timestamp.0 <= target.saturating_add(TIMESTAMP_TOLERANCE) => {
                    selected = self.next.take(); // 落后时跳过中间帧，不做转换。
                }
                _ => break,
            }
        }
        if let Some(frame) = selected {
            let surface = self.gpu.convert(&frame)?;
            self.shown = Some((frame.timestamp.0, surface));
        }
        let (_, surface) = self.shown.as_ref().context("Video has no decoded frame")?;
        Ok(surface.clone())
    }
}

fn bgra_image(pixels: RgbaImage) -> Arc<RenderImage> {
    Arc::new(RenderImage::new(smallvec![Frame::new(pixels)]))
}

/// GPUI expects BGRA bytes even though the image crate calls this RGBA.
fn swap_red_blue(mut pixels: RgbaImage) -> RgbaImage {
    for pixel in pixels.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    pixels
}
