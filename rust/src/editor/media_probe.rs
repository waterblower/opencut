use super::{
    explorer::{is_audio_path, is_image_path, is_video_path},
    model::{DEFAULT_IMAGE_CLIP_DURATION, MediaAsset, MediaKind},
    timeline::FrameRate,
};
use ffmpeg::{codec, format, media::Type};
use ffmpeg_next as ffmpeg;
use std::{fs, path::Path};
use ulid::Ulid;

pub(super) fn probe_asset(path: &Path, id: Ulid) -> Result<MediaAsset, String> {
    if is_image_path(path) {
        probe_image(path, id)
    } else if is_audio_path(path) {
        probe_audio(path, id)
    } else if is_video_path(path) {
        probe_video(path, id)
    } else {
        Err(format!("unsupported media file: {}", path.display()))
    }
}

pub(super) fn probe_video(path: &Path, id: Ulid) -> Result<MediaAsset, String> {
    ffmpeg::init().map_err(|error| format!("could not initialize FFmpeg: {error}"))?;
    let input = format::input(path)
        .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
    let stream = input
        .streams()
        .best(Type::Video)
        .ok_or_else(|| format!("{} has no video stream", path.display()))?;
    let context = codec::context::Context::from_parameters(stream.parameters())
        .map_err(|error| format!("could not inspect video parameters: {error}"))?;
    let decoder = context
        .decoder()
        .video()
        .map_err(|error| format!("could not inspect video decoder: {error}"))?;
    let source_frame_rate = frame_rate_from_ffmpeg(stream.avg_frame_rate()).unwrap_or_default();
    let framerate = source_frame_rate.frames_per_second();
    let duration = media_duration(&input, &stream);
    validate_duration(path, duration)?;
    if duration < 1.0 / 30.0 {
        return Err(format!("{} is shorter than one frame", path.display()));
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

pub(super) fn probe_image(path: &Path, id: Ulid) -> Result<MediaAsset, String> {
    let (width, height) = image::image_dimensions(path)
        .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
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

pub(super) fn probe_audio(path: &Path, id: Ulid) -> Result<MediaAsset, String> {
    ffmpeg::init().map_err(|error| format!("could not initialize FFmpeg: {error}"))?;
    let input = format::input(path)
        .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
    let stream = input
        .streams()
        .best(Type::Audio)
        .ok_or_else(|| format!("{} has no audio stream", path.display()))?;
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

fn validate_duration(path: &Path, duration: f64) -> Result<(), String> {
    if duration.is_finite() && duration > 0.0 {
        Ok(())
    } else {
        Err(format!(
            "could not determine duration of {}",
            path.display()
        ))
    }
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
