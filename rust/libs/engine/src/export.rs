//! Synchronous timeline export. No editor state, playback clock, or worker is used.

use crate::{export_encoder::ExportEncoder, image::load_image, video_frame::frame_to_rgba};
use ::timeline::TimelineSerialization;
use ::timeline::serialization::{
    Clip, FrameRate, MediaAsset, MediaClipData, MediaKind, TextClipProperties,
    TimelineEditingState, TimelineSettings, TrackKind, VideoClipProperties,
};
use anyhow::{Context as _, Result, bail};
use ffmpeg_next as ffmpeg;
use gpui::{
    AppContext as _, Context, HeadlessAppContext, IntoElement, Render, RenderImage, TextAlign,
    Window, WindowHandle, div, img, prelude::*, px, rgb, rgba, size,
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
    /// Replace an existing output file. Source media is never replaced.
    pub overwrite: bool,
}

/// Export the entire timeline to MP4 (H.264 + stereo AAC), synchronously.
/// Canvas size, frame rate, and audio sample rate come from the document.
/// Requires macOS with Metal and VideoToolbox; existing output is replaced only with
/// `option.overwrite`, and never when it is one of the timeline's source media.
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

    // 每个 clip 一个解码器：导出开始前全部打开，整个导出期间按 clip ID 复用。
    let data = &timeline_serialization.editing_state;
    let mut videos = HashMap::new(); // 按 clip 隔离游标，同一素材可同时出现在不同位置。
    let mut images = HashMap::new(); // 图片不随时间变化，按素材共享。
    let mut audios = HashMap::new();
    for clip in &data.clips {
        let Some(media) = clip_media(clip) else {
            continue;
        };
        let track = data
            .tracks
            .iter()
            .find(|track| track.id == media.track_id)
            .context("Missing clip track")?;
        let asset = data
            .assets
            .iter()
            .find(|asset| asset.id == media.asset_id)
            .context("Missing clip asset")?;
        let path = option.project_root.join(&asset.path);
        if matches!(clip, Clip::Video(_)) && track.visible {
            match asset.kind {
                MediaKind::Video => {
                    let source = source_position(media, asset, media.timeline_start, settings);
                    let video = ClipVideo::open(&path, source)
                        .context(format!("Opening export video {}", path.display()))?;
                    videos.insert(media.id, video);
                }
                MediaKind::Image => {
                    if let Entry::Vacant(entry) = images.entry(asset.id) {
                        let pixels = load_image(&path)
                            .context(format!("Loading export image {}", path.display()))?;
                        entry.insert(prepare_image(pixels));
                    }
                }
                MediaKind::Audio => bail!("Audio asset used as a visual clip"),
            }
        }
        if !track.muted && !media.audio_properties.muted && asset.has_audio {
            let mut backend = AudioBackend::open(&path)
                .context(format!("Opening export audio {}", path.display()))?;
            backend
                .audio
                .configure_output(&PcmFormat::default_layout(settings.audio_sample_rate, 2)?)?;
            if media.source_in > 0 {
                backend
                    .audio
                    .seek(frame_duration(media.source_in, settings.frame_rate))?;
            }
            audios.insert(
                media.id,
                ClipAudio {
                    decoder: backend.audio,
                    pending: None,
                },
            );
        }
    }

    let temporary = output_path.with_file_name(format!(".opencut-export-{}.mp4", Ulid::generate()));
    // 只占用临时文件名：create_new 保证不覆盖已有文件；
    // 句柄立即关闭，FFmpeg 按路径另行打开写入。
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
            // settings.width/height：输出像素，即编码帧尺寸。
            // logical_width/height：GPUI 逻辑像素；
            // 乘以 scale 后等于输出像素，render_to_image 才得到完整输出帧。
            let scale = window.scale_factor();
            let logical_width = settings.width as f32 / scale;
            let logical_height = settings.height as f32 / scale;
            window.resize(size(px(logical_width), px(logical_height)));
            window.bounds_changed(cx);
        })?;
        let mut encoder = ExportEncoder::open(&temporary, settings, option.video_bitrate)?;

        let mut audio_position = 0_i64; // 已提交编码的采样数；不随视频帧率取整累加。
        let mut last_progress = Instant::now();
        for index in 0..frame_count {
            render_frame(
                &mut cx,
                window,
                &mut encoder,
                &mut videos,
                &images,
                &mut audios,
                &mut audio_position,
                &mut last_progress,
                timeline_serialization,
                index,
                frame_count,
                total_samples,
                started,
            )?;
        }
        eprintln!("Export finalizing: flushing encoders and writing MP4");
        encoder.finish(total_samples)?;
        if option.overwrite {
            fs::rename(&temporary, output_path) // 原子替换：输出路径上不会出现写了一半的文件。
        } else {
            fs::hard_link(&temporary, output_path) // 目标已存在时失败，不会覆盖。
        }
        .context(format!("Publishing export {}", output_path.display()))?;
        Ok(())
    })();
    let cleanup = match fs::remove_file(&temporary) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()), // rename 已把临时文件发布为输出。
        cleanup => cleanup,
    };
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error).context("Removing temporary export"),
        (Err(error), Err(cleanup)) => Err(error.context(format!(
            "Also could not remove temporary export: {cleanup:?}"
        ))),
    }
}

#[allow(clippy::too_many_arguments)]
fn render_frame(
    cx: &mut HeadlessAppContext,
    window: WindowHandle<ExportCanvas>,
    encoder: &mut ExportEncoder,
    videos: &mut HashMap<Ulid, ClipVideo>,
    images: &HashMap<Ulid, Arc<RenderImage>>,
    audios: &mut HashMap<Ulid, ClipAudio>,
    audio_position: &mut i64,
    last_progress: &mut Instant,
    timeline_serialization: &TimelineSerialization,
    index: i64,
    frame_count: i64,
    total_samples: i64,
    started: Instant,
) -> Result<()> {
    let settings = timeline_serialization.editing_state.settings;

    let canvas = visual_frame(videos, images, &timeline_serialization.editing_state, index)?;
    window.update(cx, |view, _, _| *view = canvas)?;
    let image = cx.update_window(window.into(), |_, window, cx| {
        window.refresh();
        let arena = window.draw(cx);
        let image = window.render_to_image();
        arena.clear(cx);
        image
    })??;
    encoder.video(&image, index)?;

    let audio_end = frame_units(index + 1, settings.frame_rate, settings.audio_sample_rate) as i64;
    // 按 AAC 块顺序推进，至多提前一个块；最终块止于 timeline 末尾。
    while *audio_position < audio_end {
        let count =
            (encoder.audio_frame_size() as i64).min(total_samples - *audio_position) as usize;
        let samples = mix_audio(
            audios,
            &timeline_serialization.editing_state,
            *audio_position,
            count,
        )?;
        encoder.audio(&samples, *audio_position, total_samples)?;
        *audio_position += count as i64;
    }
    let completed = index + 1;
    if last_progress.elapsed() >= Duration::from_secs(1) || completed == frame_count {
        let elapsed = started.elapsed().as_secs_f64();
        let remaining = elapsed * (frame_count - completed) as f64 / completed as f64;
        eprintln!(
            "Export frames: {:.1}% ({completed}/{frame_count}), remaining ~{remaining:.1}s",
            completed as f64 / frame_count as f64 * 100.0,
        );
        *last_progress = Instant::now();
    }
    Ok(())
}

struct ExportCanvas {
    width: u32,
    height: u32,
    layers: Vec<ExportLayer>,
}

enum ExportLayer {
    VideoFrame {
        frame_image: Arc<RenderImage>,
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
                ExportLayer::VideoFrame {
                    frame_image,
                    properties,
                } => {
                    if properties.scale <= 0.0 {
                        continue;
                    }
                    let dimensions = frame_image.size(0);
                    let source_width = dimensions.width.0 as f32;
                    let source_height = dimensions.height.0 as f32;
                    let fit = (width / source_width).min(height / source_height); // 先完整适配画布，再应用 clip 缩放。
                    let image_width = source_width * fit * properties.scale as f32;
                    let image_height = source_height * fit * properties.scale as f32;
                    canvas = canvas.child(
                        img(Arc::clone(frame_image))
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

struct ClipVideo {
    decoder: VideoDecoder,
    current: VideoFrame,
    next: Option<VideoFrame>, // 保留后一帧，按时间选择最近帧；距离相同时选前一帧。
    scaler: Option<ffmpeg::software::scaling::Context>,
    image: Option<Arc<RenderImage>>, // 同一源帧用于多个输出帧时复用转换结果。
}

struct ClipAudio {
    decoder: AudioDecoder,
    pending: Option<AudioSamples>, // 尚未被当前请求完全消耗的解码块。
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
        Ok(_) if !option.overwrite => {
            bail!("Export output already exists: {}", output.display())
        }
        Ok(_) => {
            let target = fs::canonicalize(output).context("Resolving export output")?;
            for asset in &timeline.editing_state.assets {
                if fs::canonicalize(option.project_root.join(&asset.path))
                    .is_ok_and(|source| source == target)
                {
                    bail!(
                        "Export output would replace source media: {}",
                        output.display()
                    );
                }
            }
        }
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

fn visual_frame(
    videos: &mut HashMap<Ulid, ClipVideo>,
    images: &HashMap<Ulid, Arc<RenderImage>>,
    timeline: &TimelineEditingState,
    position: i64,
) -> Result<ExportCanvas> {
    let mut layers = Vec::new();
    // 文档第一条轨道在最上层；同轨后面的 clip 覆盖前面的 clip。
    for track in timeline.tracks.iter().rev() {
        if !track.visible || track.kind == TrackKind::Audio {
            continue;
        }
        for clip in timeline
            .clips
            .iter()
            .filter(|clip| clip_track(clip) == track.id)
        {
            if position < clip_start(clip)
                || position >= clip.end_frame(timeline.settings.frame_rate)
            {
                continue;
            }
            match clip {
                Clip::Audio(_) => {}
                Clip::Text(text) => layers.push(ExportLayer::Text(text.properties.clone())),
                Clip::Video(media) => {
                    let asset = timeline
                        .assets
                        .iter()
                        .find(|asset| asset.id == media.asset_id)
                        .context("Missing visual asset")?;
                    let frame_image = match asset.kind {
                        MediaKind::Video => {
                            let source = source_position(media, asset, position, timeline.settings);
                            videos
                                .get_mut(&media.id)
                                .context("Missing export video decoder")?
                                .image_at(source)
                                .context(format!("Decoding clip {} at {source:?}", media.id))?
                        }
                        MediaKind::Image => {
                            Arc::clone(images.get(&asset.id).context("Missing export image")?)
                        }
                        MediaKind::Audio => bail!("Audio asset used as a visual clip"),
                    };
                    layers.push(ExportLayer::VideoFrame {
                        frame_image,
                        properties: media.video_properties,
                    });
                }
            }
        }
    }
    Ok(ExportCanvas {
        width: timeline.settings.width,
        height: timeline.settings.height,
        layers,
    })
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

        let image = prepare_image(frame_to_rgba(&self.current, &mut self.scaler)?);
        self.image = Some(Arc::clone(&image));
        Ok(image)
    }
}

fn mix_audio(
    audios: &mut HashMap<Ulid, ClipAudio>,
    data: &TimelineEditingState,
    start: i64,
    count: usize,
) -> Result<Vec<[f32; 2]>> {
    let mut mixed = vec![[0.0_f32; 2]; count];
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
        let source = (frame_units(media.source_in, fps, rate) as i64)
            .checked_add(from - clip_start)
            .context("Export source audio interval overflow")?;
        let source_end = frame_units(media.source_out, fps, rate) as i64;
        // 两端独立舍入可能相差一个采样；不读取 source_out 之后的声音。
        let count = (to - from).min(source_end - source).max(0) as usize;
        if count == 0 {
            continue;
        }
        let samples = audios
            .get_mut(&media.id)
            .context("Missing export audio decoder")?
            .read(source, count, rate)
            .context(format!("Reading audio for clip {}", media.id))?;
        let gain = 10.0_f32.powf(media.audio_properties.gain_db.clamp(-96.0, 24.0) as f32 / 20.0);
        for (index, sample) in samples.iter().enumerate() {
            let target = &mut mixed[(from - start) as usize + index];
            target[0] += sample[0] * gain;
            target[1] += sample[1] * gain;
        }
    }
    for sample in &mut mixed {
        for channel in sample {
            *channel = channel.clamp(-1.0, 1.0);
        }
    }
    Ok(mixed)
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
