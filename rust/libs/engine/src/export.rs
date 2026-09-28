//! Synchronous timeline export. No editor state, playback clock, or worker is used.

use crate::image::load_image;
use ::timeline::TimelineSerialization;
use ::timeline::serialization::{
    Clip, FrameRate, MediaAsset, MediaClipData, MediaKind, TextClipProperties, TimelineSettings,
    TrackKind, VideoClipProperties,
};
use anyhow::{Context as _, Result, bail};
use ffmpeg_next as ffmpeg;
use gpui::{
    AppContext as _, Context, HeadlessAppContext, IntoElement, Render, RenderImage, TextAlign,
    Window, div, img, prelude::*, px, rgb, rgba, size,
};
use image::{Frame, RgbaImage};
use media_backend::{
    AudioBackend, AudioDecoder, AudioSamples, PcmFormat, VideoBackend, VideoDecoder, VideoFrame,
};
use smallvec::smallvec;
use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use ulid::Ulid;

pub struct ExportOption {
    /// Relative asset paths are resolved against this project directory.
    pub project_root: PathBuf,
    /// H.264 target bitrate in bits per second.
    pub video_bitrate: u64,
}

/// Export the entire timeline to MP4 (H.264 + stereo AAC), synchronously.
/// Canvas size, frame rate, and audio sample rate come from the document.
/// Requires macOS with Metal and VideoToolbox; existing output is never replaced.
/// Persisted playhead and other view preferences do not affect the export.
pub fn export(
    timeline_serialization: &TimelineSerialization,
    output_path: &Path,
    option: &ExportOption,
) -> Result<()> {
    validate_export(timeline_serialization, output_path, option)?;
    ffmpeg::init().context("Initializing export codecs")?;
    let settings = timeline_serialization.editing_state.settings;
    let frame_count = timeline_serialization.frame_count();
    let total_samples = frame_units(frame_count, settings.frame_rate, settings.audio_sample_rate);
    let total_samples =
        i64::try_from(total_samples).context("Export audio duration is too large")?;

    let temporary = output_path.with_file_name(format!(".opencut-export-{}.mp4", Ulid::generate()));
    drop(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .context("Creating temporary export")?,
    );
    let started = Instant::now();
    eprintln!(
        "Export starting: {frame_count} frames, {}x{} -> {}",
        settings.width,
        settings.height,
        output_path.display(),
    );
    let result = (|| -> Result<()> {
        let platform = gpui_platform::current_platform(true);
        let mut cx = HeadlessAppContext::with_platform(
            platform.text_system(),
            Arc::new(()),
            gpui_platform::current_headless_renderer,
        );
        let window = cx
            .open_window(
                size(px(settings.width as f32), px(settings.height as f32)),
                |_, cx| {
                    cx.new(|_| ExportCanvas {
                        width: settings.width,
                        height: settings.height,
                        layers: Vec::new(),
                    })
                },
            )
            .context("Creating export canvas")?;
        window.update(&mut cx, |_, window, cx| {
            let scale = window.scale_factor(); // timeline 使用输出像素；窗口使用逻辑像素。
            window.resize(size(
                px(settings.width as f32 / scale),
                px(settings.height as f32 / scale),
            ));
            window.bounds_changed(cx);
        })?;
        let mut encoder = ExportEncoder::open(&temporary, settings, option.video_bitrate)?;
        let mut visuals = VisualSources::default();
        let mut audio = AudioSources::default();
        let mut audio_position = 0_i64; // 已提交编码的采样数；不随视频帧率取整累加。
        let mut last_progress = Instant::now();
        for index in 0..frame_count {
            let canvas = visuals.frame(timeline_serialization, &option.project_root, index)?;
            window.update(&mut cx, |view, _, _| *view = canvas)?;
            let image = cx.update_window(window.into(), |_, window, cx| {
                window.refresh();
                let arena = window.draw(cx);
                let image = window.render_to_image();
                arena.clear(cx);
                image
            })??;
            encoder.video(&image, index)?;

            let audio_end =
                frame_units(index + 1, settings.frame_rate, settings.audio_sample_rate) as i64;
            // 按 AAC 块顺序推进，至多提前一个块；最终块止于 timeline 末尾。
            while audio_position < audio_end {
                let count = (encoder.audio.frame_size() as i64).min(total_samples - audio_position)
                    as usize;
                let samples = audio.mix(
                    timeline_serialization,
                    &option.project_root,
                    audio_position,
                    count,
                )?;
                encoder.audio(&samples, audio_position, total_samples)?;
                audio_position += count as i64;
            }
            let completed = index + 1;
            if last_progress.elapsed() >= Duration::from_secs(1) || completed == frame_count {
                let elapsed = started.elapsed().as_secs_f64();
                let remaining = elapsed * (frame_count - completed) as f64 / completed as f64;
                eprintln!(
                    "Export frames: {:.1}% ({completed}/{frame_count}), elapsed {elapsed:.1}s, remaining ~{remaining:.1}s",
                    completed as f64 / frame_count as f64 * 100.0,
                );
                last_progress = Instant::now();
            }
        }
        eprintln!("Export finalizing: flushing encoders and writing MP4");
        encoder.finish(total_samples)?;
        fs::hard_link(&temporary, output_path)
            .context(format!("Publishing export {}", output_path.display()))?;
        Ok(())
    })();
    let cleanup = fs::remove_file(&temporary);
    match (result, cleanup) {
        (Ok(()), Ok(())) => {
            eprintln!("Export complete: {:.1}s", started.elapsed().as_secs_f64());
            Ok(())
        }
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error).context("Removing temporary export"),
        (Err(error), Err(cleanup)) => Err(error.context(format!(
            "Also could not remove temporary export: {cleanup:?}"
        ))),
    }
}

struct ExportCanvas {
    width: u32,
    height: u32,
    layers: Vec<ExportLayer>,
}

enum ExportLayer {
    Image {
        image: Arc<RenderImage>,
        properties: VideoClipProperties,
    },
    Text(TextClipProperties),
}

impl Render for ExportCanvas {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let scale = window.scale_factor();
        let width = self.width as f32 / scale;
        let height = self.height as f32 / scale;
        let mut canvas = div()
            .relative()
            .overflow_hidden()
            .w(px(width))
            .h(px(height))
            .bg(rgb(0));
        for layer in &self.layers {
            match layer {
                ExportLayer::Image { image, properties } => {
                    if properties.scale <= 0.0 {
                        continue;
                    }
                    let dimensions = image.size(0);
                    let source_width = dimensions.width.0 as f32;
                    let source_height = dimensions.height.0 as f32;
                    let fit = (width / source_width).min(height / source_height); // 先完整适配画布，再应用 clip 缩放。
                    let image_width = source_width * fit * properties.scale as f32;
                    let image_height = source_height * fit * properties.scale as f32;
                    canvas = canvas.child(
                        img(Arc::clone(image))
                            .absolute()
                            .left(px(
                                (width - image_width) / 2.0 + properties.position_x as f32 / scale
                            ))
                            .top(px((height - image_height) / 2.0
                                + properties.position_y as f32 / scale))
                            .w(px(image_width))
                            .h(px(image_height)),
                    );
                }
                ExportLayer::Text(properties) => {
                    canvas = canvas.child(
                        div()
                            .absolute()
                            .left(px(properties.position_x as f32 * width))
                            .top(px(properties.position_y as f32 * height))
                            .w(px(0.0))
                            .h(px(0.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .whitespace_nowrap()
                                    .font_family(properties.font.clone())
                                    .text_size(px(properties.font_size as f32 / scale))
                                    .line_height(px(properties.font_size as f32 * 1.2 / scale))
                                    .text_align(TextAlign::Center)
                                    .text_color(rgba(properties.color.rotate_left(8)))
                                    .child(properties.text.clone()),
                            ),
                    );
                }
            }
        }
        canvas
    }
}

#[derive(Default)]
struct VisualSources {
    videos: HashMap<Ulid, ClipVideo>, // 按 clip 隔离游标，同一素材可同时出现在不同位置。
    images: HashMap<Ulid, Arc<RenderImage>>,
}

struct ClipVideo {
    decoder: VideoDecoder,
    current: VideoFrame,
    next: Option<VideoFrame>, // 保留后一帧，按时间选择最近帧；距离相同时选前一帧。
    scaler: Option<ffmpeg::software::scaling::Context>,
    image: Option<Arc<RenderImage>>, // 同一源帧用于多个输出帧时复用转换结果。
}

#[derive(Default)]
struct AudioSources {
    clips: HashMap<Ulid, ClipAudio>,
}

struct ClipAudio {
    decoder: AudioDecoder,
    pending: Option<AudioSamples>, // 尚未被当前请求完全消耗的解码块。
}

struct ExportEncoder {
    output: ffmpeg::format::context::Output,
    video: ffmpeg::encoder::Video,
    audio: ffmpeg::encoder::Audio,
    scaler: ffmpeg::software::scaling::Context,
}

fn validate_export(
    timeline: &TimelineSerialization,
    output: &Path,
    option: &ExportOption,
) -> Result<()> {
    if !cfg!(target_os = "macos") {
        bail!("Timeline export currently requires macOS");
    }
    let data = &timeline.editing_state;
    let settings = data.settings;
    if settings.width == 0
        || settings.height == 0
        || !settings.width.is_multiple_of(2)
        || !settings.height.is_multiple_of(2)
    {
        bail!("H.264 export requires positive, even canvas dimensions");
    }
    if settings.frame_rate.numerator == 0
        || settings.frame_rate.denominator == 0
        || settings.audio_sample_rate == 0
    {
        bail!("Export frame rate and audio sample rate must be positive");
    }
    i32::try_from(settings.width).context("Export width is too large")?;
    i32::try_from(settings.height).context("Export height is too large")?;
    i32::try_from(settings.frame_rate.numerator).context("Export frame rate is too large")?;
    i32::try_from(settings.frame_rate.denominator).context("Export frame rate is too large")?;
    i32::try_from(settings.audio_sample_rate).context("Export audio sample rate is too large")?;
    if option.video_bitrate == 0 || option.video_bitrate > i64::MAX as u64 {
        bail!("Export video bitrate must be positive and fit in a signed 64-bit integer");
    }
    if data.clips.is_empty() {
        bail!("Cannot export an empty timeline");
    }
    if !option.project_root.is_dir() {
        bail!(
            "Export project root is not a directory: {}",
            option.project_root.display()
        );
    }
    if output.extension().and_then(|value| value.to_str()) != Some("mp4") {
        bail!("Export output must have an .mp4 extension");
    }
    match fs::symlink_metadata(output) {
        Ok(_) => bail!("Export output already exists: {}", output.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("Inspecting export output"),
    }
    let mut ids = HashSet::new();
    for track in &data.tracks {
        if !ids.insert(track.id) {
            bail!("Duplicate export track {}", track.id);
        }
    }
    ids.clear();
    for asset in &data.assets {
        if !ids.insert(asset.id) {
            bail!("Duplicate export asset {}", asset.id);
        }
    }
    ids.clear();
    for clip in &data.clips {
        if !ids.insert(clip_id(clip)) {
            bail!("Duplicate export clip {}", clip_id(clip));
        }
        let track = data
            .tracks
            .iter()
            .find(|track| track.id == clip_track(clip))
            .context("Export clip references a missing track")?;
        if clip_start(clip) < 0 || clip.end_frame(settings.frame_rate) <= clip_start(clip) {
            bail!("Clip {} has an invalid timeline interval", clip_id(clip));
        }
        match clip {
            Clip::Video(media) | Clip::Audio(media) => {
                let asset = data
                    .assets
                    .iter()
                    .find(|asset| asset.id == media.asset_id)
                    .context("Export clip references a missing asset")?;
                match clip {
                    Clip::Video(_) => {
                        if track.kind != TrackKind::Video || asset.kind == MediaKind::Audio {
                            bail!(
                                "Video clip {} requires a video track and visual asset",
                                media.id
                            );
                        }
                        let properties = media.video_properties;
                        if !properties.position_x.is_finite()
                            || !properties.position_y.is_finite()
                            || !properties.scale.is_finite()
                            || properties.scale < 0.0
                        {
                            bail!("Clip {} has invalid visual properties", media.id);
                        }
                    }
                    Clip::Audio(_) => {
                        if track.kind != TrackKind::Audio || asset.kind == MediaKind::Image {
                            bail!(
                                "Audio clip {} requires an audio track and audio or video asset",
                                media.id
                            );
                        }
                    }
                    Clip::Text(_) => unreachable!(),
                }
                if media.source_in < 0
                    || media.source_out <= media.source_in
                    || !media.audio_properties.gain_db.is_finite()
                {
                    bail!("Clip {} has an invalid trim or audio gain", media.id);
                }
                media
                    .timeline_start
                    .checked_add(media.source_out - media.source_in)
                    .context("Export clip end exceeds the timeline range")?;
                i64::try_from(frame_units(
                    media.source_out,
                    settings.frame_rate,
                    settings.audio_sample_rate,
                ))
                .context("Export source audio position is too large")?;
            }
            Clip::Text(text) => {
                let properties = &text.properties;
                if track.kind != TrackKind::Text
                    || !properties.position_x.is_finite()
                    || !properties.position_y.is_finite()
                    || !properties.font_size.is_finite()
                    || properties.font_size <= 0.0
                {
                    bail!("Text clip {} has an invalid track or layout", text.id);
                }
            }
        }
    }
    Ok(())
}

impl VisualSources {
    fn frame(
        &mut self,
        timeline: &TimelineSerialization,
        project_root: &Path,
        position: i64,
    ) -> Result<ExportCanvas> {
        let data = &timeline.editing_state;
        let mut layers = Vec::new();
        // 文档第一条轨道在最上层；同轨后面的 clip 覆盖前面的 clip。
        for track in data.tracks.iter().rev() {
            if !track.visible || track.kind == TrackKind::Audio {
                continue;
            }
            for clip in data
                .clips
                .iter()
                .filter(|clip| clip_track(clip) == track.id)
            {
                if position < clip_start(clip)
                    || position >= clip.end_frame(data.settings.frame_rate)
                {
                    continue;
                }
                match clip {
                    Clip::Audio(_) => {}
                    Clip::Text(text) => layers.push(ExportLayer::Text(text.properties.clone())),
                    Clip::Video(media) => {
                        let asset = data
                            .assets
                            .iter()
                            .find(|asset| asset.id == media.asset_id)
                            .context("Missing visual asset")?;
                        let path = project_root.join(&asset.path);
                        let image = match asset.kind {
                            MediaKind::Video => {
                                let source = source_position(media, asset, position, data.settings);
                                if let Entry::Vacant(entry) = self.videos.entry(media.id) {
                                    entry.insert(ClipVideo::open(&path, source).context(
                                        format!("Opening export video {}", path.display()),
                                    )?);
                                }
                                self.videos
                                    .get_mut(&media.id)
                                    .unwrap()
                                    .image_at(source)
                                    .context(format!("Decoding clip {} at {source:?}", media.id))?
                            }
                            MediaKind::Image => {
                                if let Entry::Vacant(entry) = self.images.entry(asset.id) {
                                    let pixels = load_image(&path).context(format!(
                                        "Loading export image {}",
                                        path.display()
                                    ))?;
                                    entry.insert(prepare_image(pixels));
                                }
                                Arc::clone(&self.images[&asset.id])
                            }
                            MediaKind::Audio => bail!("Audio asset used as a visual clip"),
                        };
                        layers.push(ExportLayer::Image {
                            image,
                            properties: media.video_properties,
                        });
                    }
                }
            }
        }
        self.videos.retain(|id, _| {
            data.clips
                .iter()
                .find(|clip| clip_id(clip) == *id)
                .is_some_and(|clip| clip.end_frame(data.settings.frame_rate) > position)
        });
        Ok(ExportCanvas {
            width: data.settings.width,
            height: data.settings.height,
            layers,
        })
    }
}

impl ClipVideo {
    fn open(path: &Path, position: Duration) -> Result<Self> {
        let metadata = VideoBackend::probe(path)?;
        let mut decoder = VideoDecoder::open(
            path,
            metadata.video.stream_index,
            metadata.origin_microseconds,
        )?;
        if !position.is_zero() {
            decoder.seek(position)?;
        }
        let current = decoder
            .next_frame()?
            .context("Video has no decoded frames")?;
        let next = decoder.next_frame()?;
        Ok(Self {
            decoder,
            current,
            next,
            scaler: None,
            image: None,
        })
    }

    fn image_at(&mut self, position: Duration) -> Result<Arc<RenderImage>> {
        let target = i128::try_from(position.as_micros()).context("Video position is too large")?;
        while let Some(next) = &self.next {
            if next.timestamp < self.current.timestamp {
                bail!("Video presentation timestamps moved backwards");
            }
            let current_time = i128::from(self.current.timestamp.0);
            let next_time = i128::from(next.timestamp.0);
            if next_time > target && (next_time - target).abs() >= (current_time - target).abs() {
                break;
            }
            self.current = self.next.take().unwrap();
            self.image = None;
            self.next = self.decoder.next_frame()?;
        }
        if let Some(image) = &self.image {
            return Ok(Arc::clone(image));
        }

        let frame = &self.current;
        let mut transferred = ffmpeg::frame::Video::empty();
        // SAFETY: frame owns its AVFrame; the transfer destination is exclusively owned.
        let hardware = unsafe { !(*frame.native.as_ptr()).hw_frames_ctx.is_null() };
        let source = if hardware {
            let result = unsafe {
                ffmpeg::ffi::av_hwframe_transfer_data(
                    transferred.as_mut_ptr(),
                    frame.native.as_ptr(),
                    0,
                )
            };
            if result < 0 {
                return Err(ffmpeg::Error::from(result)).context("Transferring export video frame");
            }
            &transferred
        } else {
            &frame.native
        };
        let definition = ffmpeg::software::scaling::context::Definition {
            format: source.format(),
            width: source.width(),
            height: source.height(),
        };
        let reconfigure = match &self.scaler {
            Some(scaler) => *scaler.input() != definition,
            None => true,
        };
        if reconfigure {
            self.scaler = Some(ffmpeg::software::scaling::Context::get(
                source.format(),
                source.width(),
                source.height(),
                ffmpeg::format::Pixel::BGRA,
                source.width(),
                source.height(),
                ffmpeg::software::scaling::Flags::BILINEAR,
            )?);
        }
        let scaler = self
            .scaler
            .as_mut()
            .context("Missing export video scaler")?;
        let matrix = match frame.color_space {
            ffmpeg::color::Space::BT709 => ffmpeg::ffi::SWS_CS_ITU709,
            ffmpeg::color::Space::BT2020NCL | ffmpeg::color::Space::BT2020CL => {
                ffmpeg::ffi::SWS_CS_BT2020
            }
            ffmpeg::color::Space::FCC => ffmpeg::ffi::SWS_CS_FCC,
            ffmpeg::color::Space::SMPTE240M => ffmpeg::ffi::SWS_CS_SMPTE240M,
            ffmpeg::color::Space::BT470BG | ffmpeg::color::Space::SMPTE170M => {
                ffmpeg::ffi::SWS_CS_ITU601
            }
            _ if source.height() >= 720 => ffmpeg::ffi::SWS_CS_ITU709,
            _ => ffmpeg::ffi::SWS_CS_ITU601,
        };
        // SAFETY: coefficients have static lifetime; scaler is exclusively owned.
        let result = unsafe {
            let coefficients = ffmpeg::ffi::sws_getCoefficients(matrix);
            ffmpeg::ffi::sws_setColorspaceDetails(
                scaler.as_mut_ptr(),
                coefficients,
                i32::from(frame.color_range == ffmpeg::color::Range::JPEG),
                coefficients,
                1,
                0,
                1 << 16,
                1 << 16,
            )
        };
        if result < 0 {
            return Err(ffmpeg::Error::from(result)).context("Configuring export video colors");
        }
        let mut bgra = ffmpeg::frame::Video::empty();
        scaler
            .run(source, &mut bgra)
            .context("Converting export video frame")?;
        let mut pixels = RgbaImage::new(bgra.width(), bgra.height()); // GPUI 使用 BGRA 字节布局。
        let row_bytes = bgra.width() as usize * 4;
        for (row, output) in pixels.as_mut().chunks_exact_mut(row_bytes).enumerate() {
            let offset = row * bgra.stride(0);
            output.copy_from_slice(&bgra.data(0)[offset..offset + row_bytes]);
        }
        let quarter = frame.rotation_degrees / 90.0;
        if !quarter.is_finite() || (quarter - quarter.round()).abs() > 0.1 / 90.0 {
            bail!(
                "Unsupported display rotation: {} degrees",
                frame.rotation_degrees
            );
        }
        pixels = match quarter.round().rem_euclid(4.0) as u32 {
            1 => image::imageops::rotate270(&pixels),
            2 => image::imageops::rotate180(&pixels),
            3 => image::imageops::rotate90(&pixels),
            _ => pixels,
        };
        let image = Arc::new(RenderImage::new(smallvec![Frame::new(pixels)]));
        self.image = Some(Arc::clone(&image));
        Ok(image)
    }
}

impl AudioSources {
    fn mix(
        &mut self,
        timeline: &TimelineSerialization,
        project_root: &Path,
        start: i64,
        count: usize,
    ) -> Result<Vec<[f32; 2]>> {
        let mut mixed = vec![[0.0_f32; 2]; count];
        let data = &timeline.editing_state;
        let settings = data.settings;
        let rate = settings.audio_sample_rate;
        let fps = settings.frame_rate;
        let end = start + count as i64;
        for clip in &data.clips {
            let Some(media) = clip_media(clip) else {
                continue;
            };
            let track = data
                .tracks
                .iter()
                .find(|track| track.id == media.track_id)
                .context("Missing audio track")?;
            let asset = data
                .assets
                .iter()
                .find(|asset| asset.id == media.asset_id)
                .context("Missing audio asset")?;
            if track.muted || media.audio_properties.muted || !asset.has_audio {
                continue;
            }
            let clip_start = frame_units(media.timeline_start, fps, rate) as i64;
            let clip_end = frame_units(clip.end_frame(fps), fps, rate) as i64;
            let from = start.max(clip_start);
            let to = end.min(clip_end);
            if from >= to {
                continue;
            }
            if let Entry::Vacant(entry) = self.clips.entry(media.id) {
                let path = project_root.join(&asset.path);
                let mut backend = AudioBackend::open(&path)
                    .context(format!("Opening export audio {}", path.display()))?;
                backend
                    .audio
                    .configure_output(&PcmFormat::default_layout(rate, 2)?)?;
                if media.source_in > 0 {
                    backend.audio.seek(frame_duration(media.source_in, fps))?;
                }
                entry.insert(ClipAudio {
                    decoder: backend.audio,
                    pending: None,
                });
            }
            let source = (frame_units(media.source_in, fps, rate) as i64)
                .checked_add(from - clip_start)
                .context("Export source audio interval overflow")?;
            let source_end = frame_units(media.source_out, fps, rate) as i64;
            // 两端独立舍入可能相差一个采样；不读取 source_out 之后的声音。
            let count = (to - from).min(source_end - source).max(0) as usize;
            if count == 0 {
                continue;
            }
            let samples = self
                .clips
                .get_mut(&media.id)
                .unwrap()
                .read(source, count, rate)
                .context(format!("Reading audio for clip {}", media.id))?;
            let gain =
                10.0_f32.powf(media.audio_properties.gain_db.clamp(-96.0, 24.0) as f32 / 20.0);
            for (index, sample) in samples.iter().enumerate() {
                let target = &mut mixed[(from - start) as usize + index];
                target[0] += sample[0] * gain;
                target[1] += sample[1] * gain;
            }
        }
        self.clips.retain(|id, _| {
            data.clips
                .iter()
                .find(|clip| clip_id(clip) == *id)
                .is_some_and(|clip| frame_units(clip.end_frame(fps), fps, rate) > end as u64)
        });
        for sample in &mut mixed {
            for channel in sample {
                *channel = channel.clamp(-1.0, 1.0);
            }
        }
        Ok(mixed)
    }
}

impl ClipAudio {
    fn read(&mut self, start: i64, count: usize, rate: u32) -> Result<Vec<[f32; 2]>> {
        let end = start
            .checked_add(count as i64)
            .context("Audio source interval overflow")?;
        let mut result = vec![[0.0; 2]; count];
        loop {
            if self.pending.is_none() {
                self.pending = self.decoder.next_samples()?;
            }
            let Some(block) = &self.pending else {
                break;
            };
            let numerator = i128::from(block.timestamp.0) * i128::from(rate);
            let rounded = if numerator < 0 {
                numerator - 500_000
            } else {
                numerator + 500_000
            };
            let block_start =
                i64::try_from(rounded / 1_000_000).context("Decoded audio timestamp overflow")?;
            let block_end = block_start
                .checked_add(block.frame_count as i64)
                .context("Decoded audio interval overflow")?;
            if block_start >= end {
                break;
            }
            for sample in start.max(block_start)..end.min(block_end) {
                let source = (sample - block_start) as usize * 2;
                result[(sample - start) as usize] =
                    [block.samples[source], block.samples[source + 1]];
            }
            if block_end > end {
                break;
            }
            self.pending = None;
        }
        Ok(result)
    }
}

impl ExportEncoder {
    fn open(path: &Path, settings: TimelineSettings, bitrate: u64) -> Result<Self> {
        let mut output = ffmpeg::format::output(path).context("Opening export container")?;
        let global = output
            .format()
            .flags()
            .contains(ffmpeg::format::Flags::GLOBAL_HEADER);
        let codec = ffmpeg::encoder::find_by_name("h264_videotoolbox")
            .context("H.264 VideoToolbox encoder is unavailable")?;
        let mut video = ffmpeg::codec::context::Context::new_with_codec(codec)
            .encoder()
            .video()?;
        video.set_width(settings.width);
        video.set_height(settings.height);
        video.set_format(ffmpeg::format::Pixel::YUV420P);
        let fps = settings.frame_rate;
        video.set_time_base((fps.denominator as i32, fps.numerator as i32));
        video.set_frame_rate(Some((fps.numerator as i32, fps.denominator as i32)));
        video.set_bit_rate(usize::try_from(bitrate).context("Export bitrate is too large")?);
        video.set_max_b_frames(0);
        video.set_gop(
            ((f64::from(fps.numerator) / f64::from(fps.denominator)) * 2.0)
                .round()
                .clamp(1.0, i32::MAX as f64) as u32,
        );
        video.set_colorspace(ffmpeg::color::Space::BT709);
        video.set_color_range(ffmpeg::color::Range::MPEG);
        // SAFETY: this encoder exclusively owns its live codec context.
        unsafe {
            (*video.as_mut_ptr()).color_primaries = ffmpeg::ffi::AVColorPrimaries::AVCOL_PRI_BT709;
            (*video.as_mut_ptr()).color_trc =
                ffmpeg::ffi::AVColorTransferCharacteristic::AVCOL_TRC_BT709;
        }
        if global {
            video.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
        }
        let mut options = ffmpeg::Dictionary::new();
        options.set("allow_sw", "1");
        let video = video
            .open_with(options)
            .context("Opening H.264 export encoder")?;
        {
            let mut stream = output.add_stream(codec)?;
            stream.set_time_base(video.time_base());
            stream.set_parameters(&video);
        }

        let aac =
            ffmpeg::encoder::find(ffmpeg::codec::Id::AAC).context("AAC encoder is unavailable")?;
        let mut audio = ffmpeg::codec::context::Context::new_with_codec(aac)
            .encoder()
            .audio()?;
        audio.set_rate(settings.audio_sample_rate as i32);
        audio.set_time_base((1, settings.audio_sample_rate as i32));
        audio.set_channel_layout(ffmpeg::ChannelLayout::STEREO);
        audio.set_format(ffmpeg::format::Sample::F32(
            ffmpeg::format::sample::Type::Planar,
        ));
        audio.set_bit_rate(192_000);
        if global {
            audio.set_flags(ffmpeg::codec::Flags::GLOBAL_HEADER);
        }
        let audio = audio.open_as(aac).context("Opening AAC export encoder")?;
        if audio.frame_size() == 0 {
            bail!("AAC encoder has no fixed frame size");
        }
        {
            let mut stream = output.add_stream(aac)?;
            stream.set_time_base(audio.time_base());
            stream.set_parameters(&audio);
        }
        output.write_header().context("Writing export header")?;
        let mut scaler = ffmpeg::software::scaling::Context::get(
            ffmpeg::format::Pixel::RGBA,
            settings.width,
            settings.height,
            ffmpeg::format::Pixel::YUV420P,
            settings.width,
            settings.height,
            ffmpeg::software::scaling::Flags::BILINEAR,
        )?;
        // SAFETY: scaler owns the context; FFmpeg returns a static coefficient table.
        unsafe {
            let matrix = ffmpeg::ffi::sws_getCoefficients(ffmpeg::ffi::SWS_CS_ITU709);
            if ffmpeg::ffi::sws_setColorspaceDetails(
                scaler.as_mut_ptr(),
                matrix,
                1,
                matrix,
                0,
                0,
                1 << 16,
                1 << 16,
            ) < 0
            {
                bail!("Could not configure export BT.709 conversion");
            }
        }
        Ok(Self {
            output,
            video,
            audio,
            scaler,
        })
    }

    fn video(&mut self, image: &RgbaImage, index: i64) -> Result<()> {
        if image.dimensions() != (self.video.width(), self.video.height()) {
            bail!(
                "Export renderer returned {}x{} pixels; expected {}x{}",
                image.width(),
                image.height(),
                self.video.width(),
                self.video.height(),
            );
        }
        let mut rgba =
            ffmpeg::frame::Video::new(ffmpeg::format::Pixel::RGBA, image.width(), image.height());
        let row = image.width() as usize * 4;
        let stride = rgba.stride(0);
        for y in 0..image.height() as usize {
            rgba.data_mut(0)[y * stride..y * stride + row]
                .copy_from_slice(&image.as_raw()[y * row..(y + 1) * row]);
        }
        let mut frame = ffmpeg::frame::Video::empty();
        self.scaler
            .run(&rgba, &mut frame)
            .context("Converting export pixels")?;
        frame.set_pts(Some(index));
        frame.set_color_space(ffmpeg::color::Space::BT709);
        frame.set_color_range(ffmpeg::color::Range::MPEG);
        frame.set_color_primaries(ffmpeg::color::Primaries::BT709);
        frame.set_color_transfer_characteristic(ffmpeg::color::TransferCharacteristic::BT709);
        self.video
            .send_frame(&frame)
            .context("Encoding export video")?;
        self.drain_video(false)
    }

    fn audio(&mut self, samples: &[[f32; 2]], start: i64, total_samples: i64) -> Result<()> {
        let mut frame = ffmpeg::frame::Audio::new(
            self.audio.format(),
            samples.len(),
            ffmpeg::ChannelLayout::STEREO,
        );
        frame.set_rate(self.audio.rate());
        frame.set_pts(Some(start));
        for channel in 0..2 {
            for (index, sample) in samples.iter().enumerate() {
                frame.plane_mut::<f32>(channel)[index] = sample[channel];
            }
        }
        self.audio
            .send_frame(&frame)
            .context("Encoding export audio")?;
        self.drain_audio(false, total_samples)
    }

    fn finish(mut self, total_samples: i64) -> Result<()> {
        self.video.send_eof().context("Finishing video encoder")?;
        self.drain_video(true)?;
        self.audio.send_eof().context("Finishing audio encoder")?;
        self.drain_audio(true, total_samples)?;
        self.output
            .write_trailer()
            .context("Finishing export container")?;
        Ok(())
    }

    fn drain_video(&mut self, finishing: bool) -> Result<()> {
        loop {
            let mut packet = ffmpeg::Packet::empty();
            match self.video.receive_packet(&mut packet) {
                Ok(()) => {
                    packet.set_stream(0);
                    if packet.duration() <= 0 {
                        packet.set_duration(1);
                    }
                    packet.rescale_ts(
                        self.video.time_base(),
                        self.output.stream(0).unwrap().time_base(),
                    );
                    packet
                        .write_interleaved(&mut self.output)
                        .context("Writing export video")?;
                }
                Err(ffmpeg::Error::Eof) => return Ok(()),
                Err(ffmpeg::Error::Other { errno })
                    if errno == ffmpeg::error::EAGAIN && !finishing =>
                {
                    return Ok(());
                }
                Err(error) => return Err(error).context("Draining export video encoder"),
            }
        }
    }

    fn drain_audio(&mut self, finishing: bool, total_samples: i64) -> Result<()> {
        loop {
            let mut packet = ffmpeg::Packet::empty();
            match self.audio.receive_packet(&mut packet) {
                Ok(()) => {
                    // 编码填充不延长容器时长；负 PTS 保留 AAC priming 信息。
                    if let Some(pts) = packet.pts() {
                        if pts >= total_samples {
                            continue;
                        }
                        if pts >= 0 {
                            packet.set_duration(packet.duration().min(total_samples - pts));
                        }
                    }
                    packet.set_stream(1);
                    packet.rescale_ts(
                        self.audio.time_base(),
                        self.output.stream(1).unwrap().time_base(),
                    );
                    packet
                        .write_interleaved(&mut self.output)
                        .context("Writing export audio")?;
                }
                Err(ffmpeg::Error::Eof) => return Ok(()),
                Err(ffmpeg::Error::Other { errno })
                    if errno == ffmpeg::error::EAGAIN && !finishing =>
                {
                    return Ok(());
                }
                Err(error) => return Err(error).context("Draining export audio encoder"),
            }
        }
    }
}

fn clip_id(clip: &Clip) -> Ulid {
    match clip {
        Clip::Video(media) | Clip::Audio(media) => media.id,
        Clip::Text(text) => text.id,
    }
}

fn clip_track(clip: &Clip) -> Ulid {
    match clip {
        Clip::Video(media) | Clip::Audio(media) => media.track_id,
        Clip::Text(text) => text.track_id,
    }
}

fn clip_start(clip: &Clip) -> i64 {
    match clip {
        Clip::Video(media) | Clip::Audio(media) => media.timeline_start,
        Clip::Text(text) => text.timeline_start,
    }
}

fn clip_media(clip: &Clip) -> Option<&MediaClipData> {
    match clip {
        Clip::Video(media) | Clip::Audio(media) => Some(media),
        Clip::Text(_) => None,
    }
}

// 用绝对帧边界换算采样数或纳秒，四舍五入；不逐帧累加误差。
fn frame_units(frame: i64, fps: FrameRate, units_per_second: u32) -> u64 {
    let numerator =
        frame.max(0) as u128 * u128::from(fps.denominator) * u128::from(units_per_second);
    let denominator = u128::from(fps.numerator);
    ((numerator + denominator / 2) / denominator).min(u64::MAX as u128) as u64
}

fn frame_duration(frame: i64, fps: FrameRate) -> Duration {
    Duration::from_nanos(frame_units(frame, fps, 1_000_000_000))
}

fn source_position(
    media: &MediaClipData,
    asset: &MediaAsset,
    position: i64,
    settings: TimelineSettings,
) -> Duration {
    let local = (position - media.timeline_start).clamp(0, media.source_out - media.source_in);
    let source = media.source_in + local;
    let Some(rate) = source_frame_rate(asset) else {
        let samples = frame_units(source, settings.frame_rate, settings.audio_sample_rate);
        return Duration::from_secs_f64(samples as f64 / f64::from(settings.audio_sample_rate));
    };
    let numerator =
        source as u128 * u128::from(settings.frame_rate.denominator) * u128::from(rate.numerator);
    let denominator = u128::from(settings.frame_rate.numerator) * u128::from(rate.denominator);
    let frame = (numerator / denominator).min(i64::MAX as u128) as i64;
    frame_duration(frame, rate)
}

fn source_frame_rate(asset: &MediaAsset) -> Option<FrameRate> {
    if asset.frame_rate_numerator > 0 && asset.frame_rate_denominator > 0 {
        return Some(FrameRate {
            numerator: asset.frame_rate_numerator,
            denominator: asset.frame_rate_denominator,
        });
    }
    if !asset.framerate.is_finite() || asset.framerate <= 0.0 {
        return None;
    }
    for numerator in [24_000, 30_000, 60_000] {
        if (asset.framerate - f64::from(numerator) / 1001.0).abs() < 0.01 {
            return Some(FrameRate {
                numerator,
                denominator: 1001,
            });
        }
    }
    Some(FrameRate {
        numerator: asset.framerate.round().clamp(1.0, u32::MAX as f64) as u32,
        denominator: 1,
    })
}

fn prepare_image(mut pixels: RgbaImage) -> Arc<RenderImage> {
    for pixel in pixels.pixels_mut() {
        pixel.0.swap(0, 2);
    } // GPUI 使用 BGRA。
    Arc::new(RenderImage::new(smallvec![Frame::new(pixels)]))
}
