//! Timeline frame preparation for playback and scrubbing. Clips of the same asset on the
//! same track share one video reader, which keeps decoding forward and only seeks on
//! backward or distant jumps.

#[cfg(target_os = "macos")]
use crate::gpu::GpuResources;
use crate::image::load_image;
use anyhow::{Context as _, Result, bail};
#[cfg(target_os = "macos")]
use core_video::pixel_buffer::CVPixelBuffer;
#[cfg(target_os = "macos")]
use ffmpeg_next::format;
use gpui::RenderImage;
use image::{Frame, RgbaImage};
use media_backend::{MediaInfo, VideoDecoder, VideoFrame};
use smallvec::smallvec;
use std::{
    collections::{HashMap, hash_map::Entry},
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
    readers: HashMap<ReaderKey, VideoReader>, // 同轨道同素材的片段共用一个读取器并顺序前进；不释放。
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

    /// The caller validates the timeline, so clips on one track never overlap. Every call on
    /// one decoder must pass the same timeline: readers are cached by track and asset ID.
    /// Positions past the end clamp to the last frame.
    pub fn frame_at(
        &mut self,
        timeline: &TimelineEditingState,
        position: TimelineFrameIndex,
    ) -> Result<TimelineFrameComposition> {
        let last_frame_index = (timeline.content_duration() - TimelineFrameIndex::ONE_FRAME)
            .max(TimelineFrameIndex::ZERO);
        let position = position.clamp(TimelineFrameIndex::ZERO, last_frame_index);
        let mut layers: Vec<PreparedLayer> = Vec::new();
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
                                    let key = ReaderKey {
                                        track_id: track.id,
                                        asset_id: asset.id,
                                    };
                                    #[rustfmt::skip]
                                    let reader = match self.readers.entry(key) {
                                        Entry::Occupied(entry) => {
                                            entry.into_mut()
                                        },
                                        Entry::Vacant(entry) => {
                                            entry.insert(
                                                VideoReader::open(&asset_path).context(format!(
                                                    "Opening timeline video {}",
                                                    asset_path.display()
                                                ))?,
                                            )
                                        },
                                    };
                                    let source = timeline.source_position_at(clip, position);
                                    let frame = reader.picture_at(source).context(format!(
                                        "Preparing timeline clip {} from {} at {:.6}s",
                                        media.id(),
                                        asset_path.display(),
                                        source.as_secs_f64(),
                                    ))?;
                                    layers.push(PreparedLayer::VideoFrame {
                                        clip_id: media.id(),
                                        frame,
                                        properties: media.video_properties,
                                    });
                                }
                                #[cfg(not(target_os = "macos"))]
                                bail!("Timeline video requires macOS: {}", asset_path.display());
                            }
                            MediaKind::Image => {
                                let image = match self.images.entry(asset.id) {
                                    Entry::Occupied(entry) => Arc::clone(entry.get()),
                                    Entry::Vacant(entry) => {
                                        let load_started = Instant::now();
                                        let pixels = load_image(&asset_path).context(format!(
                                            "Loading timeline image {}",
                                            asset_path.display()
                                        ))?;
                                        let image = Arc::clone(
                                            entry.insert(bgra_image(swap_red_blue(pixels))),
                                        );
                                        eprintln!(
                                            "[clip-switch] load image {}: {:?}",
                                            asset_path.display(),
                                            load_started.elapsed(),
                                        );
                                        image
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
        Ok(TimelineFrameComposition {
            frame_index: position,
            timestamp: timeline.position_at_frame(position),
            width: timeline.settings.width,
            height: timeline.settings.height,
            layers,
        })
    }
}

/// One reader per track and asset: a track shows at most one clip at a time, so clips
/// sharing a key never need two source positions in the same frame.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct ReaderKey {
    track_id: Ulid,
    asset_id: Ulid,
}

#[cfg(target_os = "macos")]
struct VideoReader {
    gpu: GpuResources,
    decoder: VideoDecoder,
    next: Option<VideoFrame>,            // 已解码、尚未到展示时间的帧。
    shown: Option<(i64, CVPixelBuffer)>, // (源 PTS 微秒, GPU surface)。
}

#[cfg(target_os = "macos")]
impl VideoReader {
    fn open(path: &Path) -> Result<Self> {
        let started = Instant::now();
        // 只打开一次：读取元数据后，同一个 demuxer 交给解码器。
        let input = format::input(path).context("opening timeline video demuxer")?;
        let metadata = MediaInfo::from_av_input(&input, path)?;
        let probed = Instant::now();
        let decoder = VideoDecoder::from_av_input(
            input,
            metadata.video.stream_index,
            metadata.origin_microseconds,
        )?;
        let decoder_opened = Instant::now();
        let gpu = GpuResources::new((
            metadata.video.width as usize,
            metadata.video.height as usize,
        ))?;
        let gpu_created = Instant::now();
        eprintln!(
            "[clip-switch] open reader {}: probe={:?}, decoder_open={:?}, gpu_resources={:?}, total={:?}",
            path.display(),
            probed.duration_since(started),
            decoder_opened.duration_since(probed),
            gpu_created.duration_since(decoder_opened),
            gpu_created.duration_since(started),
        );
        Ok(Self {
            gpu,
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
        let started = Instant::now();
        if !continues {
            self.decoder.seek(source)?;
            self.next = None;
            selected = Some(
                self.decoder
                    .next_frame()?
                    .context("Video has no frame after seeking")?,
            );
        }
        let sought = Instant::now();
        let mut forward_frames = 0_u32;
        loop {
            if self.next.is_none() {
                self.next = self.decoder.next_frame()?; // None：EOF，保留最后一帧。
            }
            match &self.next {
                Some(frame) if frame.timestamp.0 <= target.saturating_add(TIMESTAMP_TOLERANCE) => {
                    selected = self.next.take(); // 落后时跳过中间帧，不做转换。
                    forward_frames += 1;
                }
                _ => break,
            }
        }
        let decoded = Instant::now();
        if let Some(frame) = selected {
            let surface = self.gpu.convert(&frame)?;
            self.shown = Some((frame.timestamp.0, surface));
        }
        let converted = Instant::now();
        // 只在 seek 或追帧时打印，片段内逐帧播放不刷屏。
        if !continues || forward_frames > 1 {
            eprintln!(
                "[clip-switch] picture_at {} µs: seeked={}, seek={:?}, forward_frames={}, forward_decode={:?}, convert={:?}",
                target,
                !continues,
                sought.duration_since(started),
                forward_frames,
                decoded.duration_since(sought),
                converted.duration_since(decoded),
            );
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
