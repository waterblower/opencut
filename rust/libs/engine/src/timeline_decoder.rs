//! Timeline frame preparation for playback and scrubbing. Each video clip keeps decoding
//! forward and only seeks on backward or distant jumps.

use crate::displayed_frame::DisplayedFrame;
#[cfg(target_os = "macos")]
use crate::gpu::GpuResources;
use crate::image::load_image;
use anyhow::{Context as _, Result, bail};
use ffmpeg_next::{ffi, format::Pixel, frame::Video, software::scaling, util::color};
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
    Clip, MediaKind, TextClipProperties, TimelineEditingState, TimelineFrameIndex, TrackKind,
    VideoClipProperties,
};
use ulid::Ulid;

const FORWARD_DECODE_LIMIT: i64 = 1_000_000; // 目标超出当前帧 1 秒以上时改为 seek，而不是逐帧解码过去。
const TIMESTAMP_TOLERANCE: i64 = 1_000; // 吸收源帧 PTS 与目标时间的微秒取整误差。

/// Render images are prepared when a frame is decoded, never during UI rendering.
pub struct PreparedFrame {
    pub frame: TimelineFrameIndex, // 已钳制到最后一帧。
    pub timestamp: Duration,
    pub width: u32,
    pub height: u32,
    pub layers: Vec<PreparedLayer>, // 自下而上。
}

pub enum PreparedLayer {
    Picture {
        clip_id: Ulid,
        frame: DisplayedFrame, // GPU surface 或 CPU 图像；源帧未变化时复用。
        properties: VideoClipProperties,
    },
    Text {
        clip_id: Ulid,
        properties: TextClipProperties,
    },
}

impl PreparedFrame {
    pub fn images(&self) -> impl Iterator<Item = &Arc<RenderImage>> {
        self.layers.iter().filter_map(|layer| match layer {
            PreparedLayer::Picture {
                frame: DisplayedFrame::GpuiImage(image),
                ..
            } => Some(image),
            _ => None,
        })
    }
}

pub struct TimelineDecoder {
    project_root: PathBuf,
    readers: HashMap<Ulid, ClipReader>, // 按 clip 而非素材区分：同一素材的重叠 clip 各自前进。
    images: HashMap<Ulid, Arc<RenderImage>>,
}

impl TimelineDecoder {
    pub fn new(project_root: &Path) -> Self {
        Self {
            project_root: project_root.to_owned(),
            readers: HashMap::new(),
            images: HashMap::new(),
        }
    }

    /// The caller validates the timeline. Positions past the end clamp to the last frame.
    pub fn frame_at(
        &mut self,
        timeline: &TimelineEditingState,
        position: TimelineFrameIndex,
    ) -> Result<PreparedFrame> {
        let last = (timeline.content_duration() - TimelineFrameIndex::ONE_FRAME)
            .max(TimelineFrameIndex::ZERO);
        let position = position.clamp(TimelineFrameIndex::ZERO, last);
        let mut layers = Vec::new();
        let mut active_readers = HashSet::new();
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
                    Clip::Text(clip) => layers.push(PreparedLayer::Text {
                        clip_id: clip.id,
                        properties: clip.properties.clone(),
                    }),
                    Clip::Video(media) => {
                        let asset = timeline
                            .asset(media.asset_id)
                            .context("Validated clip references a missing asset")?;
                        let path = self.project_root.join(&asset.path);
                        let image = match asset.kind {
                            MediaKind::Video => {
                                let reader = match self.readers.entry(media.id) {
                                    Entry::Occupied(entry) => entry.into_mut(),
                                    Entry::Vacant(entry) => {
                                        entry.insert(ClipReader::open(&path).context(format!(
                                            "Opening timeline video {}",
                                            path.display()
                                        ))?)
                                    }
                                };
                                active_readers.insert(media.id);
                                let source = timeline.source_position_at(clip, position);
                                match reader.picture_at(source) {
                                    Ok(image) => image,
                                    Err(error) => {
                                        // A failed decoder may be partially advanced; reopen on retry.
                                        self.readers.remove(&media.id);
                                        return Err(error).context(format!(
                                            "Preparing timeline clip {} from {} at {:.6}s",
                                            media.id,
                                            path.display(),
                                            source.as_secs_f64(),
                                        ));
                                    }
                                }
                            }
                            MediaKind::Image => {
                                DisplayedFrame::GpuiImage(match self.images.entry(asset.id) {
                                    Entry::Occupied(entry) => Arc::clone(entry.get()),
                                    Entry::Vacant(entry) => {
                                        let pixels = load_image(&path).context(format!(
                                            "Loading timeline image {}",
                                            path.display()
                                        ))?;
                                        Arc::clone(entry.insert(bgra_image(swap_red_blue(pixels))))
                                    }
                                })
                            }
                            MediaKind::Audio => bail!("Visual clip {} uses audio", media.id),
                        };
                        layers.push(PreparedLayer::Picture {
                            clip_id: media.id,
                            frame: image,
                            properties: media.video_properties,
                        });
                    }
                }
            }
        }
        // Clips outside the current frame release their decoders.
        self.readers.retain(|id, _| active_readers.contains(id));
        Ok(PreparedFrame {
            frame: position,
            timestamp: timeline.position_at_frame(position),
            width: timeline.settings.width,
            height: timeline.settings.height,
            layers,
        })
    }
}

struct ClipReader {
    #[cfg(target_os = "macos")]
    gpu: Option<GpuResources>,
    decoder: VideoDecoder,
    scaler: Option<scaling::Context>,
    next: Option<VideoFrame>,             // 已解码、尚未到展示时间的帧。
    shown: Option<(i64, DisplayedFrame)>, // (源 PTS 微秒, 图像)。
}

impl ClipReader {
    fn open(path: &Path) -> Result<Self> {
        let metadata = VideoBackend::probe(path)?;
        let decoder = VideoDecoder::open(
            path,
            metadata.video.stream_index,
            metadata.origin_microseconds,
        )?;
        Ok(Self {
            #[cfg(target_os = "macos")]
            gpu: GpuResources::new((
                metadata.video.width as usize,
                metadata.video.height as usize,
            ))?,
            decoder,
            scaler: None,
            next: None,
            shown: None,
        })
    }

    /// Latest frame at or before the source position; converts only when the frame changes.
    fn picture_at(&mut self, source: Duration) -> Result<DisplayedFrame> {
        let started = Instant::now();
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
        let decoded = Instant::now();
        if let Some(frame) = selected {
            #[cfg(target_os = "macos")]
            let surface = match &mut self.gpu {
                Some(gpu) => gpu.convert(&frame)?,
                None => None,
            };
            #[cfg(target_os = "macos")]
            let image = match surface {
                Some(buffer) => DisplayedFrame::Surface(buffer),
                None => DisplayedFrame::GpuiImage(frame_to_gpui_image(&frame, &mut self.scaler)?),
            };
            #[cfg(not(target_os = "macos"))]
            let image = DisplayedFrame::GpuiImage(frame_to_gpui_image(&frame, &mut self.scaler)?);
            self.shown = Some((frame.timestamp.0, image));
        }
        let (_, image) = self.shown.as_ref().context("Video has no decoded frame")?;
        Ok(image.clone())
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

fn frame_to_gpui_image(
    frame: &VideoFrame,
    scaler: &mut Option<scaling::Context>,
) -> Result<Arc<gpui::RenderImage>> {
    let started = Instant::now();
    let mut transferred = Video::empty();
    // SAFETY: frame owns its AVFrame; the transfer destination is exclusively owned.
    let hardware = unsafe { !(*frame.native.as_ptr()).hw_frames_ctx.is_null() };
    let source = if hardware {
        let result = unsafe {
            ffi::av_hwframe_transfer_data(transferred.as_mut_ptr(), frame.native.as_ptr(), 0)
        };
        if result < 0 {
            return Err(ffmpeg_next::Error::from(result)).context("Transferring video frame");
        }
        &transferred
    } else {
        &frame.native
    };
    let transferred_at = Instant::now();
    let definition = scaling::context::Definition {
        format: source.format(),
        width: source.width(),
        height: source.height(),
    };
    if scaler
        .as_ref()
        .is_none_or(|scaler| *scaler.input() != definition)
    {
        *scaler = Some(scaling::Context::get(
            source.format(),
            source.width(),
            source.height(),
            Pixel::BGRA,
            source.width(),
            source.height(),
            scaling::Flags::BILINEAR,
        )?);
    }
    let active_scaler = scaler.as_mut().context("Missing video scaler")?;
    let matrix = match frame.color_space {
        color::Space::BT709 => ffi::SWS_CS_ITU709,
        color::Space::BT2020NCL | color::Space::BT2020CL => ffi::SWS_CS_BT2020,
        color::Space::FCC => ffi::SWS_CS_FCC,
        color::Space::SMPTE240M => ffi::SWS_CS_SMPTE240M,
        color::Space::BT470BG | color::Space::SMPTE170M => ffi::SWS_CS_ITU601,
        _ if source.height() >= 720 => ffi::SWS_CS_ITU709,
        _ => ffi::SWS_CS_ITU601,
    };
    // SAFETY: coefficients have static lifetime; scaler is exclusively owned.
    let result = unsafe {
        let coefficients = ffi::sws_getCoefficients(matrix as i32);
        ffi::sws_setColorspaceDetails(
            active_scaler.as_mut_ptr(),
            coefficients,
            i32::from(frame.color_range == color::Range::JPEG),
            coefficients,
            1,
            0,
            1 << 16,
            1 << 16,
        )
    };
    if result < 0 {
        return Err(ffmpeg_next::Error::from(result)).context("Configuring video colors");
    }
    let configured = Instant::now();

    let image = convert_to_gpui_image(source, active_scaler)?;
    let converted = Instant::now();

    // 暂不应用视频的旋转元数据，保留解码像素的原始方向；
    // 带旋转标记的视频可能横置或倒置。

    // eprintln!(
    //     "Timeline image: hardware={hardware}, transfer={:?}, configure={:?}, convert_bgra={:?}",
    //     transferred_at.duration_since(started),
    //     configured.duration_since(transferred_at),
    //     converted.duration_since(configured),
    // );
    Ok(image)
}

/// Converts a CPU frame directly into GPUI's tightly packed BGRA image buffer.
/// The scaler must match the source and output BGRA at the same dimensions.
fn convert_to_gpui_image(
    source: &Video,
    scaler: &mut scaling::Context,
) -> Result<Arc<RenderImage>> {
    let width = source.width();
    let height = source.height();
    let row_bytes = width.checked_mul(4).context("BGRA row size overflow")?;
    let stride = i32::try_from(row_bytes).context("BGRA row too large")?;
    let source_height = i32::try_from(height).context("Video height too large")?;
    // 容器类型是 RgbaImage，实际存储 GPUI 要求的 BGRA 字节；直接写入以避免中间帧复制。
    let mut pixels = RgbaImage::new(width, height);
    let destination = [
        pixels.as_mut().as_mut_ptr(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
        std::ptr::null_mut(),
    ];
    let destination_strides = [stride, 0, 0, 0];
    // SAFETY: source remains alive; pixels is exclusively owned and sized for the
    // scaler's full BGRA output. sws_scale writes synchronously and retains no pointers.
    let rows = unsafe {
        let source_frame = &*source.as_ptr();
        let source_data = source_frame.data.map(|plane| plane as *const u8);
        ffi::sws_scale(
            scaler.as_mut_ptr(),
            source_data.as_ptr(),
            source_frame.linesize.as_ptr(),
            0,
            source_height,
            destination.as_ptr(),
            destination_strides.as_ptr(),
        )
    };
    if rows < 0 {
        return Err(ffmpeg_next::Error::from(rows)).context("Converting video frame");
    }
    if rows != source_height {
        bail!("Converted {rows} rows; expected {source_height}");
    }
    Ok(bgra_image(pixels))
}
