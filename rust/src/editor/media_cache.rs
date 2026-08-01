use super::model::{MediaAsset, MediaKind};
use ffmpeg::{
    channel_layout::ChannelLayout,
    codec, format, frame,
    media::Type,
    software::{
        resampling::Context as ResamplingContext,
        scaling::{context::Context as ScalingContext, flag::Flags as ScalingFlags},
    },
    util::format::{Pixel, Sample, sample::Type as SampleType},
};
use ffmpeg_next as ffmpeg;
use image::{RgbImage, Rgba, RgbaImage};
use std::{
    fs,
    mem::size_of,
    path::{Path, PathBuf},
};

const THUMBNAIL_WIDTH: u32 = 480;
const WAVEFORM_WIDTH: u32 = 1200;
const WAVEFORM_HEIGHT: u32 = 96;

pub(super) fn thumbnail_path(project_root: &Path, asset: &MediaAsset) -> PathBuf {
    cache_directory(project_root).join(format!("asset-{}-thumbnail.png", asset.id))
}

pub(super) fn waveform_path(project_root: &Path, asset: &MediaAsset) -> PathBuf {
    cache_directory(project_root).join(format!("asset-{}-waveform.png", asset.id))
}

pub(super) fn cache_is_ready(project_root: &Path, asset: &MediaAsset) -> bool {
    let thumbnail_ready =
        asset.kind != MediaKind::Video || thumbnail_path(project_root, asset).is_file();
    let waveform_ready = !asset.has_audio || waveform_path(project_root, asset).is_file();
    thumbnail_ready && waveform_ready
}

pub(super) fn generate(project_root: &Path, asset: &MediaAsset) -> Result<(), String> {
    ffmpeg::init().map_err(|error| format!("could not initialize FFmpeg: {error}"))?;
    let directory = cache_directory(project_root);
    fs::create_dir_all(&directory)
        .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
    let ignore = directory.join(".gitignore");
    if !ignore.exists() {
        fs::write(&ignore, "*\n!.gitignore\n")
            .map_err(|error| format!("could not write {}: {error}", ignore.display()))?;
    }
    let source = project_root.join(&asset.path);

    if asset.kind == MediaKind::Video {
        generate_thumbnail(&source, &thumbnail_path(project_root, asset))?;
    }
    if asset.has_audio {
        generate_waveform(&source, asset.duration, &waveform_path(project_root, asset))?;
    }
    Ok(())
}

fn generate_thumbnail(source: &Path, output: &Path) -> Result<(), String> {
    let mut input = format::input(source)
        .map_err(|error| format!("could not open {}: {error}", source.display()))?;
    let stream = input
        .streams()
        .best(Type::Video)
        .ok_or_else(|| format!("{} has no video stream", source.display()))?;
    let stream_index = stream.index();
    let mut decoder = codec::context::Context::from_parameters(stream.parameters())
        .and_then(|context| context.decoder().video())
        .map_err(|error| format!("could not create video decoder: {error}"))?;
    let height = ((decoder.height() as f64 * THUMBNAIL_WIDTH as f64 / decoder.width() as f64)
        .round() as u32)
        .max(1);
    let mut scaler = ScalingContext::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        Pixel::RGB24,
        THUMBNAIL_WIDTH,
        height,
        ScalingFlags::BILINEAR,
    )
    .map_err(|error| format!("could not create thumbnail scaler: {error}"))?;

    for (packet_stream, packet) in input.packets() {
        if packet_stream.index() != stream_index {
            continue;
        }
        decoder
            .send_packet(&packet)
            .map_err(|error| format!("could not decode thumbnail packet: {error}"))?;
        if let Some(image) = receive_thumbnail(&mut decoder, &mut scaler)? {
            return image
                .save(output)
                .map_err(|error| format!("could not write {}: {error}", output.display()));
        }
    }
    let _ = decoder.send_eof();
    if let Some(image) = receive_thumbnail(&mut decoder, &mut scaler)? {
        image
            .save(output)
            .map_err(|error| format!("could not write {}: {error}", output.display()))
    } else {
        Err(format!(
            "could not decode a frame from {}",
            source.display()
        ))
    }
}

fn receive_thumbnail(
    decoder: &mut ffmpeg::decoder::Video,
    scaler: &mut ScalingContext,
) -> Result<Option<RgbImage>, String> {
    let mut decoded = frame::Video::empty();
    if decoder.receive_frame(&mut decoded).is_err() {
        return Ok(None);
    }
    let mut rgb = frame::Video::empty();
    scaler
        .run(&decoded, &mut rgb)
        .map_err(|error| format!("could not scale thumbnail: {error}"))?;
    let row_bytes = rgb.width() as usize * 3;
    let mut pixels = Vec::with_capacity(row_bytes * rgb.height() as usize);
    for row in 0..rgb.height() as usize {
        let start = row * rgb.stride(0);
        let end = start + row_bytes;
        pixels.extend_from_slice(
            rgb.data(0)
                .get(start..end)
                .ok_or_else(|| "thumbnail frame has an invalid stride".to_string())?,
        );
    }
    RgbImage::from_raw(rgb.width(), rgb.height(), pixels)
        .map(Some)
        .ok_or_else(|| "could not construct thumbnail image".to_string())
}

fn generate_waveform(source: &Path, duration: f64, output: &Path) -> Result<(), String> {
    let mut input = format::input(source)
        .map_err(|error| format!("could not open {}: {error}", source.display()))?;
    let stream = input
        .streams()
        .best(Type::Audio)
        .ok_or_else(|| format!("{} has no audio stream", source.display()))?;
    let stream_index = stream.index();
    let mut decoder = codec::context::Context::from_parameters(stream.parameters())
        .and_then(|context| context.decoder().audio())
        .map_err(|error| format!("could not create audio decoder: {error}"))?;
    let input_layout = if decoder.channel_layout().is_empty() {
        ChannelLayout::default(i32::from(decoder.channels()))
    } else {
        decoder.channel_layout()
    };
    let sample_rate = decoder.rate().max(1);
    let mut resampler = ResamplingContext::get(
        decoder.format(),
        input_layout,
        sample_rate,
        Sample::F32(SampleType::Packed),
        ChannelLayout::MONO,
        sample_rate,
    )
    .map_err(|error| format!("could not create waveform resampler: {error}"))?;
    let total_samples = (duration.max(0.001) * sample_rate as f64).ceil() as u64;
    let mut peaks = vec![0.0_f32; WAVEFORM_WIDTH as usize];
    let mut sample_offset = 0_u64;

    for (packet_stream, packet) in input.packets() {
        if packet_stream.index() != stream_index {
            continue;
        }
        decoder
            .send_packet(&packet)
            .map_err(|error| format!("could not decode waveform packet: {error}"))?;
        receive_waveform_samples(
            &mut decoder,
            &mut resampler,
            &mut peaks,
            total_samples,
            &mut sample_offset,
        )?;
    }
    let _ = decoder.send_eof();
    receive_waveform_samples(
        &mut decoder,
        &mut resampler,
        &mut peaks,
        total_samples,
        &mut sample_offset,
    )?;

    let mut image = RgbaImage::new(WAVEFORM_WIDTH, WAVEFORM_HEIGHT);
    let center = WAVEFORM_HEIGHT as i32 / 2;
    for (x, peak) in peaks.into_iter().enumerate() {
        let half_height = (peak.clamp(0.0, 1.0) * (center - 1) as f32).round() as i32;
        for y in (center - half_height)..=(center + half_height) {
            image.put_pixel(x as u32, y as u32, Rgba([0x69, 0xc5, 0xcf, 0xff]));
        }
    }
    image
        .save(output)
        .map_err(|error| format!("could not write {}: {error}", output.display()))
}

fn receive_waveform_samples(
    decoder: &mut ffmpeg::decoder::Audio,
    resampler: &mut ResamplingContext,
    peaks: &mut [f32],
    total_samples: u64,
    sample_offset: &mut u64,
) -> Result<(), String> {
    let mut decoded = frame::Audio::empty();
    while decoder.receive_frame(&mut decoded).is_ok() {
        let mut converted = frame::Audio::empty();
        resampler
            .run(&decoded, &mut converted)
            .map_err(|error| format!("could not resample waveform audio: {error}"))?;
        let byte_count = converted.samples() * size_of::<f32>();
        let bytes = converted
            .data(0)
            .get(..byte_count)
            .ok_or_else(|| "waveform audio frame is shorter than expected".to_string())?;
        for sample in bytes.chunks_exact(size_of::<f32>()) {
            let value = f32::from_ne_bytes(sample.try_into().unwrap()).abs();
            let column = ((*sample_offset).saturating_mul(peaks.len() as u64) / total_samples)
                .min(peaks.len().saturating_sub(1) as u64) as usize;
            peaks[column] = peaks[column].max(value);
            *sample_offset = sample_offset.saturating_add(1);
        }
    }
    Ok(())
}

fn cache_directory(project_root: &Path) -> PathBuf {
    project_root.join(".opencut/cache")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn generates_thumbnail_and_waveform_without_a_subprocess() {
        ffmpeg::init().unwrap();
        let assets =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("vendor/gpui-video-player/assets");
        let video_source = assets.join("test1.mp4");
        let audio_source = [assets.join("test1.mp4"), assets.join("test3.mp4")]
            .into_iter()
            .find(|path| {
                format::input(path)
                    .ok()
                    .is_some_and(|input| input.streams().best(Type::Audio).is_some())
            })
            .expect("test video with audio");
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!("opencut-media-cache-{unique}"));
        fs::create_dir_all(&directory).unwrap();
        let thumbnail = directory.join("thumbnail.png");
        let waveform = directory.join("waveform.png");

        generate_thumbnail(&video_source, &thumbnail).unwrap();
        generate_waveform(&audio_source, 5.0, &waveform).unwrap();

        let thumbnail_image = image::open(&thumbnail).unwrap();
        let waveform_image = image::open(&waveform).unwrap();
        assert_eq!(thumbnail_image.width(), THUMBNAIL_WIDTH);
        assert_eq!(
            (waveform_image.width(), waveform_image.height()),
            (WAVEFORM_WIDTH, WAVEFORM_HEIGHT)
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
