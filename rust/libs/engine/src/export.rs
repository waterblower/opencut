use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{
    AnyElement, AppContext as _, Context, HeadlessAppContext, IntoElement, Render, Window, div, px,
    size,
};
use media_backend::{AudioBackend, AudioDecoder, AudioSamples, PcmFormat};
use timeline::serialization::{self, MediaKind, TrackKind};
use timeline::{Clip, TimelineEditingState, TimelineSerialization};
use ulid::Ulid;

use crate::{export_encoder::ExportEncoder, timeline_decoder::TimelineDecoder};
use anyhow::{Context as _, Result, bail};

pub struct ExportOption {
    /// Relative asset paths are resolved against this project directory.
    pub project_root: PathBuf,
    /// H.264 target bitrate in bits per second.
    pub video_bitrate: u64,
    /// Replace an existing output file. Source media is never replaced.
    pub overwrite: bool,
}

pub fn export_timeline(
    timeline_serialization: &TimelineSerialization,
    output_path: &Path,
    option: &ExportOption,
) -> Result<()> {
    validate_export(timeline_serialization, output_path, option)?;

    // Convenient Variables
    let editing_state = timeline_serialization.to_editing_state();
    let timeline_settings = editing_state.settings;
    let frame_count = timeline_serialization.frame_count();

    // Init the decoder and encoder
    let mut decoder = TimelineDecoder::new(&option.project_root);
    let mut encoder = ExportEncoder::open(
        output_path,
        timeline_serialization.editing_state.settings,
        option.video_bitrate,
    )?;

    // ----------------------------------|
    // creating the headless GPUI window |
    // ----------------------------------|
    let platform = gpui_platform::current_platform(true);
    let mut cx = HeadlessAppContext::with_platform(
        platform.text_system(),
        Arc::new(()),
        gpui_platform::current_headless_renderer,
    );
    let window = cx
        .open_window(
            size(
                px(timeline_settings.width as f32),
                px(timeline_settings.height as f32),
            ),
            |_, cx| cx.new(|_| ExportCanvas { element: None }),
        )
        .context("Creating export canvas")?;

    let (logical_width, logical_height) = window.update(&mut cx, |_, window, cx| {
        let scale = window.scale_factor();
        let width = timeline_settings.width as f32 / scale;
        let height = timeline_settings.height as f32 / scale;

        window.resize(size(px(width), px(height)));
        window.bounds_changed(cx);

        (width, height)
    })?;

    // variables needed by audio handling
    let rate = timeline_settings.frame_rate;
    let sample_rate = timeline_settings.audio_sample_rate;
    let total_samples = i64::try_from(rate.audio_samples(frame_count.into(), sample_rate))
        .context("Timeline audio sample count exceeds i64")?;
    let mut audio_position = 0_i64;
    let mut audio_readers = HashMap::new();

    // --------------- //
    // The Render Loop //
    // --------------- //
    for i in 0..frame_count {
        let started = Instant::now();
        // Video Handling
        {
            // get frame compositions from the decoder
            let frame = decoder.frame_at(&editing_state, i.into())?;
            let decoded = Instant::now();

            // convert the composition to GPUI element
            let element = frame.render_frame(logical_width, logical_height);
            let composed = Instant::now();

            // convert the GPUI element to image buffer, aka raw frame data
            let image = cx.update_window(window.into(), |root, window, cx| {
                let view = root.downcast::<ExportCanvas>().unwrap();
                view.update(cx, |view, _| {
                    view.element = Some(element);
                });

                window.refresh();
                let arena = window.draw(cx);
                let capture_started = Instant::now();
                let image = window.render_to_image();
                let capture_elapsed = capture_started.elapsed();
                arena.clear(cx);
                // eprintln!("Export frame {i}: window.render_to_image={capture_elapsed:?}");
                image
            })??;
            let rendered = Instant::now();

            // send to encoder
            encoder.video(&image, i)?;
            let encoded = Instant::now();
            eprintln!(
                "Export frame {i}: decode={:?}, compose={:?}, render_to_image={:?}, encode={:?}, total={:?}",
                decoded.duration_since(started),
                composed.duration_since(decoded),
                rendered.duration_since(composed),
                encoded.duration_since(rendered),
                encoded.duration_since(started),
            );
        }
        // Audio Handling
        {
            // 使用绝对帧边界计算采样位置，避免逐帧取整产生累计误差。
            let audio_end = i64::try_from(rate.audio_samples((i + 1).into(), sample_rate))
                .context("Audio frame boundary exceeds i64")?
                .min(total_samples);

            // 按编码器块大小提交；允许领先视频边界一个块，最后一块止于时间线末尾。
            while audio_position < audio_end {
                let count = i64::from(encoder.audio_frame_size())
                    .max(1)
                    .min(total_samples - audio_position) as usize;
                let samples = mix_timeline_audio(
                    &editing_state,
                    &option.project_root,
                    &mut audio_readers,
                    audio_position,
                    count,
                    sample_rate,
                )?;
                encoder.audio(&samples, audio_position, total_samples)?;
                audio_position += count as i64;
            }
        }
    }
    encoder.finish(total_samples)?;
    return Ok(());
}

struct ExportCanvas {
    // question: why not element: AnyElement?
    element: Option<AnyElement>,
}

impl Render for ExportCanvas {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.element
            .take()
            .unwrap_or_else(|| div().into_any_element())
    }
}

pub struct ClipAudio {
    decoder: AudioDecoder,
    pending: Option<AudioSamples>, // 已解码但未完全消费的 PCM 块。
}

/// Mixes a sequential interval of timeline audio into interleaved stereo samples at `rate`.
/// `start` and `count` are sample indices at `rate`; clear `readers` before a non-sequential read.
pub fn mix_timeline_audio(
    timeline: &TimelineEditingState,
    project_root: &Path,
    readers: &mut HashMap<Ulid, ClipAudio>,
    start: i64,
    count: usize,
    rate: u32,
) -> Result<Vec<[f32; 2]>> {
    let mut mixed = vec![[0.0_f32; 2]; count];
    let fps = timeline.settings.frame_rate;
    let end = start
        .checked_add(i64::try_from(count)?)
        .context("Audio interval overflow")?;
    for clip in &timeline.clips {
        let media = match clip {
            Clip::Video(media) | Clip::Audio(media) => media,
            Clip::Text(_) => continue,
        };
        let track = timeline
            .tracks
            .iter()
            .find(|track| track.id == media.track_id)
            .context("Missing audio track")?;
        let asset = timeline
            .asset(media.asset_id)
            .context("Missing audio asset")?;
        if track.muted || media.audio_properties.muted || !asset.has_audio {
            continue;
        }
        let clip_start = i64::try_from(fps.audio_samples(media.timeline_start, rate))?;
        let clip_end = i64::try_from(fps.audio_samples(clip.timeline_end(fps), rate))?;
        let from = start.max(clip_start);
        let to = end.min(clip_end);
        if from >= to {
            continue;
        }
        let source_start = i64::try_from(fps.audio_samples(media.source_in, rate))?
            .checked_add(from - clip_start)
            .context("Audio source interval overflow")?;
        let source_end = i64::try_from(fps.audio_samples(media.source_out, rate))?;
        // 两端独立取整可能相差一个采样，不读取 source_out 之后的声音。
        let sample_count = (to - from).min(source_end - source_start).max(0) as usize;
        if sample_count == 0 {
            continue;
        }
        let reader = match readers.entry(media.id) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                let path = project_root.join(&asset.path);
                let mut backend = AudioBackend::open(&path)
                    .context(format!("Opening timeline audio {}", path.display()))?;
                backend
                    .audio
                    .configure_output(&PcmFormat::default_layout(rate, 2)?)?;
                // 从首次读取的位置打开；预览从片段中间开始时无需从 source_in 解码。
                let seek_nanos = source_start as u128 * 1_000_000_000 / u128::from(rate);
                backend
                    .audio
                    .seek(Duration::from_nanos(seek_nanos as u64))?;
                entry.insert(ClipAudio {
                    decoder: backend.audio,
                    pending: None,
                })
            }
        };
        let samples = reader
            .read(source_start, sample_count, rate)
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
            serialization::Clip::Video(media) | serialization::Clip::Audio(media) => {
                let asset = data
                    .assets
                    .iter()
                    .find(|asset| asset.id == media.asset_id)
                    .context("Export clip references a missing asset")?;
                match clip {
                    serialization::Clip::Video(_) => {
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
                    serialization::Clip::Audio(_) => {
                        if track.kind != TrackKind::Audio || asset.kind == MediaKind::Image {
                            bail!(
                                "Audio clip {} requires an audio track and audio or video asset",
                                media.id
                            );
                        }
                    }
                    serialization::Clip::Text(_) => unreachable!(),
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
            serialization::Clip::Text(text) => {
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

fn clip_id(clip: &serialization::Clip) -> Ulid {
    match clip {
        serialization::Clip::Video(media) | serialization::Clip::Audio(media) => media.id,
        serialization::Clip::Text(text) => text.id,
    }
}

fn clip_track(clip: &serialization::Clip) -> Ulid {
    match clip {
        serialization::Clip::Video(media) | serialization::Clip::Audio(media) => media.track_id,
        serialization::Clip::Text(text) => text.track_id,
    }
}

fn clip_start(clip: &serialization::Clip) -> i64 {
    match clip {
        serialization::Clip::Video(media) | serialization::Clip::Audio(media) => {
            media.timeline_start
        }
        serialization::Clip::Text(text) => text.timeline_start,
    }
}

// 用绝对帧边界换算采样数或纳秒，四舍五入；不逐帧累加误差。
fn frame_units(frame: i64, fps: serialization::FrameRate, units_per_second: u32) -> u64 {
    let numerator =
        frame.max(0) as u128 * u128::from(fps.denominator) * u128::from(units_per_second);
    let denominator = u128::from(fps.numerator);
    ((numerator + denominator / 2) / denominator).min(u64::MAX as u128) as u64
}
