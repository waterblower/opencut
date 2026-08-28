use crate::editor::explorer::{is_audio_path, is_image_path, is_video_path};

use super::{
    model::{DEFAULT_IMAGE_CLIP_DURATION, MediaAsset, MediaKind},
    timeline::FrameRate,
};
use anyhow::{Context as _, Result, bail};
use ffmpeg::{codec, format, media::Type};
use ffmpeg_next as ffmpeg;
use std::{fs, path::Path};
use ulid::Ulid;

pub(super) fn probe_asset(path: &Path) -> Result<MediaAsset> {
    let asset = if is_image_path(path) {
        probe_image(path, Ulid::from(0))
    } else if is_audio_path(path) {
        probe_audio(path, Ulid::from(0))
    } else if is_video_path(path) {
        probe_video(path, Ulid::from(0))
    } else {
        bail!("unsupported media file: {}", path.display());
    }?;
    Ok(asset)
}

pub(super) fn probe_video(path: &Path, id: Ulid) -> Result<MediaAsset> {
    ffmpeg::init().context("could not initialize FFmpeg")?;
    let input =
        format::input(path).with_context(|| format!("could not inspect {}", path.display()))?;
    let stream = input
        .streams()
        .best(Type::Video)
        .with_context(|| format!("{} has no video stream", path.display()))?;
    let context = codec::context::Context::from_parameters(stream.parameters())
        .context("could not inspect video parameters")?;
    let decoder = context
        .decoder()
        .video()
        .context("could not inspect video decoder")?;
    let source_frame_rate = frame_rate_from_ffmpeg(stream.avg_frame_rate()).unwrap_or_default();
    let framerate = source_frame_rate.frames_per_second();
    let duration = media_duration(&input, &stream);
    validate_duration(path, duration)?;
    if duration < 1.0 / 30.0 {
        bail!("{} is shorter than one frame", path.display());
    }

    Ok(MediaAsset {
        id,
        kind: MediaKind::Video,
        path: canonical_path(path),
        name: media_name(path),
        duration,
        width: decoder.width(),
        height: decoder.height(),
        framerate,
        frame_rate_numerator: source_frame_rate.numerator,
        frame_rate_denominator: source_frame_rate.denominator,
        codec: stream.parameters().id().name().to_string(),
        has_audio: input.streams().best(Type::Audio).is_some(),
    })
}

pub(super) fn probe_image(path: &Path, id: Ulid) -> Result<MediaAsset> {
    let (width, height) = image::image_dimensions(path)
        .with_context(|| format!("could not inspect {}", path.display()))?;
    let codec = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_uppercase())
        .unwrap_or_else(|| "IMAGE".to_string());

    Ok(MediaAsset {
        id,
        kind: MediaKind::Image,
        path: canonical_path(path),
        name: media_name(path),
        duration: DEFAULT_IMAGE_CLIP_DURATION,
        width,
        height,
        framerate: 0.0,
        frame_rate_numerator: 0,
        frame_rate_denominator: 0,
        codec,
        has_audio: false,
    })
}

pub(super) fn probe_audio(path: &Path, id: Ulid) -> Result<MediaAsset> {
    ffmpeg::init().context("could not initialize FFmpeg")?;
    let input =
        format::input(path).with_context(|| format!("could not inspect {}", path.display()))?;
    let stream = input
        .streams()
        .best(Type::Audio)
        .with_context(|| format!("{} has no audio stream", path.display()))?;
    let duration = media_duration(&input, &stream);
    validate_duration(path, duration)?;

    Ok(MediaAsset {
        id,
        kind: MediaKind::Audio,
        path: canonical_path(path),
        name: media_name(path),
        duration,
        width: 0,
        height: 0,
        framerate: 0.0,
        frame_rate_numerator: 0,
        frame_rate_denominator: 0,
        codec: stream.parameters().id().name().to_string(),
        has_audio: true,
    })
}

fn media_duration(input: &format::context::Input, stream: &format::stream::Stream<'_>) -> f64 {
    if input.duration() > 0 {
        input.duration() as f64 / ffmpeg::ffi::AV_TIME_BASE as f64
    } else if stream.duration() > 0 {
        stream.duration() as f64 * rational_to_f64(stream.time_base()).unwrap_or(0.0)
    } else {
        0.0
    }
}

fn validate_duration(path: &Path, duration: f64) -> Result<()> {
    if !duration.is_finite() || duration <= 0.0 {
        bail!("could not determine duration of {}", path.display());
    }
    Ok(())
}

fn canonical_path(path: &Path) -> std::path::PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn media_name(path: &Path) -> String {
    path.file_stem()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn rational_to_f64(value: ffmpeg::Rational) -> Option<f64> {
    let denominator = value.denominator();
    (denominator != 0).then(|| value.numerator() as f64 / denominator as f64)
}

fn frame_rate_from_ffmpeg(value: ffmpeg::Rational) -> Option<FrameRate> {
    let numerator = u32::try_from(value.numerator()).ok()?;
    let denominator = u32::try_from(value.denominator()).ok()?;
    (numerator > 0 && denominator > 0).then(|| FrameRate::new(numerator, denominator))
}
