//! Synchronous timeline export. No editor state, playback clock, or worker is used.

use crate::editor::preview_timeline::TimelinePreviewFrame;
use crate::editor::timeline_backend::{TimelineFrame, TimelineLayer};
use ::timeline::{
    Clip, MediaKind, TimelineEditingState, TimelineSerialization, TimelineSettings, TimelineTime,
    TrackKind,
};
use anyhow::{Context as _, Result, bail};
use engine::{decode::VideoReader, raster::load_image};
use ffmpeg_next as ffmpeg;
use gpui::{
    AppContext as _, Context, HeadlessAppContext, IntoElement, Render, Window, div, prelude::*, px,
    rgb, size,
};
use image::RgbaImage;
use media_backend::{AudioBackend, AudioDecoder, AudioSamples, PcmFormat};
use std::{
    collections::{HashMap, HashSet, hash_map::Entry},
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    sync::Arc,
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
    let timeline = timeline_serialization.to_editing_state();
    validate_export(&timeline, output_path, option)?;
    ffmpeg::init().context("Initializing export codecs")?;
    let settings = timeline.settings;
    let frame_count = timeline.content_duration().frames();
    let total_samples = settings
        .frame_rate
        .audio_samples(timeline.content_duration(), settings.audio_sample_rate);
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
                |_, cx| cx.new(|_| ExportCanvas { frame: None }),
            )
            .context("Creating export canvas")?;
        let mut encoder = ExportEncoder::open(&temporary, settings, option.video_bitrate)?;
        let mut visuals = VisualSources::default();
        let mut audio = AudioSources::default();
        let mut audio_position = 0_i64; // 已提交编码的采样数；不随视频帧率取整累加。
        for index in 0..frame_count {
            let position = TimelineTime::from_frames(index);
            let frame = visuals.frame(&timeline, &option.project_root, position)?;
            let prepared = TimelinePreviewFrame::new(Arc::new(frame));
            window.update(&mut cx, |canvas, _, _| canvas.frame = Some(prepared))?;
            let image = cx.update_window(window.into(), |_, window, cx| {
                window.refresh();
                let arena = window.draw(cx);
                let image = window.render_to_image();
                arena.clear(cx);
                image
            })??;
            encoder.video(&image, index)?;

            let audio_end = settings.frame_rate.audio_samples(
                TimelineTime::from_frames(index + 1),
                settings.audio_sample_rate,
            ) as i64;
            // 按 AAC 块顺序推进，至多提前一个块；最终块止于 timeline 末尾。
            while audio_position < audio_end {
                let count = (encoder.audio.frame_size() as i64).min(total_samples - audio_position)
                    as usize;
                let samples = audio.mix(&timeline, &option.project_root, audio_position, count)?;
                encoder.audio(&samples, audio_position, total_samples)?;
                audio_position += count as i64;
            }
        }
        encoder.finish(total_samples)?;
        fs::hard_link(&temporary, output_path)
            .context(format!("Publishing export {}", output_path.display()))?;
        Ok(())
    })();
    let cleanup = fs::remove_file(&temporary);
    match (result, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(error).context("Removing temporary export"),
        (Err(error), Err(cleanup)) => Err(error.context(format!(
            "Also could not remove temporary export: {cleanup:?}"
        ))),
    }
}

struct ExportCanvas {
    frame: Option<TimelinePreviewFrame>,
}

impl Render for ExportCanvas {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        match &self.frame {
            Some(frame) => frame.render(frame.frame.width as f32, frame.frame.height as f32),
            None => div().size_full().bg(rgb(0)).into_any_element(),
        }
    }
}

#[derive(Default)]
struct VisualSources {
    videos: HashMap<Ulid, VideoReader>, // 按 clip 隔离游标，同一素材可同时出现在不同位置。
    images: HashMap<Ulid, Arc<RgbaImage>>,
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
    timeline: &TimelineEditingState,
    output: &Path,
    option: &ExportOption,
) -> Result<()> {
    if !cfg!(target_os = "macos") {
        bail!("Timeline export currently requires macOS");
    }
    timeline.validate()?;
    let settings = timeline.settings;
    if !settings.width.is_multiple_of(2) || !settings.height.is_multiple_of(2) {
        bail!("H.264 export requires even canvas dimensions");
    }
    i32::try_from(settings.width).context("Export width is too large")?;
    i32::try_from(settings.height).context("Export height is too large")?;
    i32::try_from(settings.frame_rate.numerator).context("Export frame rate is too large")?;
    i32::try_from(settings.frame_rate.denominator).context("Export frame rate is too large")?;
    i32::try_from(settings.audio_sample_rate).context("Export audio sample rate is too large")?;
    if option.video_bitrate == 0 || option.video_bitrate > i64::MAX as u64 {
        bail!("Export video bitrate must be positive and fit in a signed 64-bit integer");
    }
    if timeline.content_duration() <= TimelineTime::ZERO {
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
    for clip in &timeline.clips {
        if !ids.insert(clip.id()) {
            bail!("Duplicate export clip {}", clip.id());
        }
        let Some(media) = clip.media() else {
            continue;
        };
        let track = timeline
            .track(media.track_id)
            .context("Export clip references a missing track")?;
        let asset = timeline
            .asset(media.asset_id)
            .context("Export clip references a missing asset")?;
        if matches!(clip, Clip::Audio(_))
            && (track.kind != TrackKind::Audio || asset.kind == MediaKind::Image)
        {
            bail!(
                "Audio clip {} requires an audio track and an audio or video asset",
                media.id
            );
        }
        if media.timeline_start < TimelineTime::ZERO
            || media.source_in < TimelineTime::ZERO
            || media.source_out <= media.source_in
            || !media.audio_properties.gain_db.is_finite()
        {
            bail!(
                "Clip {} has an invalid trim, start position, or audio gain",
                media.id
            );
        }
        media
            .timeline_start
            .frames()
            .checked_add(media.source_out.frames() - media.source_in.frames())
            .context("Export clip end exceeds the timeline range")?;
        i64::try_from(
            settings
                .frame_rate
                .audio_samples(media.source_out, settings.audio_sample_rate),
        )
        .context("Export source audio position is too large")?;
    }
    Ok(())
}

impl VisualSources {
    fn frame(
        &mut self,
        timeline: &TimelineEditingState,
        project_root: &Path,
        position: TimelineTime,
    ) -> Result<TimelineFrame> {
        let mut layers = Vec::new();
        // 文档第一条轨道在最上层；同轨后面的 clip 覆盖前面的 clip。
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
                    Clip::Text(text) => layers.push(TimelineLayer::Text {
                        clip_id: text.id,
                        properties: text.properties.clone(),
                    }),
                    Clip::Video(media) => {
                        let asset = timeline
                            .asset(media.asset_id)
                            .context("Missing visual asset")?;
                        let path = project_root.join(&asset.path);
                        match asset.kind {
                            MediaKind::Video => {
                                if let Entry::Vacant(entry) = self.videos.entry(media.id) {
                                    entry.insert(VideoReader::open(&path).context(format!(
                                        "Opening export video {}",
                                        path.display()
                                    ))?);
                                }
                                let source = timeline.source_position_at(clip, position);
                                let pixels = self
                                    .videos
                                    .get_mut(&media.id)
                                    .unwrap()
                                    .at(source.as_secs_f64())
                                    .context(format!("Decoding clip {} at {source:?}", media.id))?;
                                layers.push(TimelineLayer::Video {
                                    clip_id: media.id,
                                    pixels: Arc::new(pixels),
                                    properties: media.video_properties,
                                });
                            }
                            MediaKind::Image => {
                                if let Entry::Vacant(entry) = self.images.entry(asset.id) {
                                    entry.insert(Arc::new(load_image(&path).context(format!(
                                        "Loading export image {}",
                                        path.display()
                                    ))?));
                                }
                                layers.push(TimelineLayer::Image {
                                    clip_id: media.id,
                                    pixels: Arc::clone(&self.images[&asset.id]),
                                    properties: media.video_properties,
                                });
                            }
                            MediaKind::Audio => bail!("Audio asset used as a visual clip"),
                        }
                    }
                }
            }
        }
        self.videos.retain(|id, _| {
            timeline
                .clip(*id)
                .is_some_and(|clip| clip.timeline_end(timeline.settings.frame_rate) > position)
        });
        Ok(TimelineFrame {
            timestamp: timeline.duration(position),
            width: timeline.settings.width,
            height: timeline.settings.height,
            layers,
        })
    }
}

impl AudioSources {
    fn mix(
        &mut self,
        timeline: &TimelineEditingState,
        project_root: &Path,
        start: i64,
        count: usize,
    ) -> Result<Vec<[f32; 2]>> {
        let mut mixed = vec![[0.0_f32; 2]; count];
        let settings = timeline.settings;
        let rate = settings.audio_sample_rate;
        let fps = settings.frame_rate;
        let end = start + count as i64;
        for clip in &timeline.clips {
            let Some(media) = clip.media() else {
                continue;
            };
            let track = timeline
                .track(media.track_id)
                .context("Missing audio track")?;
            let asset = timeline
                .asset(media.asset_id)
                .context("Missing audio asset")?;
            if track.muted || media.audio_properties.muted || !asset.has_audio {
                continue;
            }
            let clip_start = fps.audio_samples(media.timeline_start, rate) as i64;
            let clip_end = fps.audio_samples(clip.timeline_end(fps), rate) as i64;
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
                if media.source_in > TimelineTime::ZERO {
                    backend.audio.seek(fps.duration(media.source_in))?;
                }
                entry.insert(ClipAudio {
                    decoder: backend.audio,
                    pending: None,
                });
            }
            let source = (fps.audio_samples(media.source_in, rate) as i64)
                .checked_add(from - clip_start)
                .context("Export source audio interval overflow")?;
            let source_end = fps.audio_samples(media.source_out, rate) as i64;
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
            timeline
                .clip(*id)
                .is_some_and(|clip| fps.audio_samples(clip.timeline_end(fps), rate) > end as u64)
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
            (fps.frames_per_second() * 2.0)
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
            bail!("Export renderer returned unexpected pixel dimensions");
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
