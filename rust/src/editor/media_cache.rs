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
use image::RgbImage;
use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    mem::size_of,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

const THUMBNAIL_WIDTH: u32 = 480;
const WAVEFORM_MAGIC: [u8; 4] = *b"OCWF";
const WAVEFORM_VERSION: u32 = 1;
const WAVEFORM_HEADER_SIZE: u64 = 40;
const WAVEFORM_LEVEL_HEADER_SIZE: u64 = 20;
const WAVEFORM_FINE_SAMPLES_PER_PEAK: u32 = 64;
const WAVEFORM_LEVEL_REDUCTION: usize = 4;
const MAX_WAVEFORM_LEVELS: usize = 32;
const MAX_RENDER_COLUMNS: usize = 4096;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct WaveformPeak {
    pub(super) min: f32,
    pub(super) max: f32,
}

#[derive(Debug)]
pub(super) struct WaveformLevel {
    samples_per_peak: u32,
    peaks: Vec<WaveformPeak>,
}

#[derive(Debug)]
pub(super) struct WaveformData {
    sample_rate: u32,
    total_samples: u64,
    levels: Vec<WaveformLevel>,
}

impl WaveformData {
    pub(super) fn columns(
        &self,
        source_start_seconds: f64,
        source_end_seconds: f64,
        requested_columns: usize,
    ) -> Vec<WaveformPeak> {
        if self.sample_rate == 0 || self.total_samples == 0 || self.levels.is_empty() {
            return Vec::new();
        }
        let column_count = requested_columns.clamp(1, MAX_RENDER_COLUMNS);
        let start = seconds_to_sample(source_start_seconds, self.sample_rate, self.total_samples);
        let end = seconds_to_sample(source_end_seconds, self.sample_rate, self.total_samples)
            .max(start.saturating_add(1))
            .min(self.total_samples);
        if start >= end {
            return Vec::new();
        }

        let samples_per_column = (end - start) as f64 / column_count as f64;
        let level = self
            .levels
            .iter()
            .rev()
            .find(|level| level.samples_per_peak as f64 <= samples_per_column)
            .unwrap_or(&self.levels[0]);
        let samples_per_peak = u64::from(level.samples_per_peak);
        let mut columns = Vec::with_capacity(column_count);
        for column in 0..column_count {
            let column_start =
                start + ((end - start) as u128 * column as u128 / column_count as u128) as u64;
            let column_end = start
                + ((end - start) as u128 * (column + 1) as u128 / column_count as u128) as u64;
            let first_peak = (column_start / samples_per_peak) as usize;
            let last_peak = (column_end.saturating_sub(1) / samples_per_peak) as usize;
            let mut peak = WaveformPeak {
                min: 1.0,
                max: -1.0,
            };
            for source_peak in level.peaks.get(first_peak..=last_peak).unwrap_or_default() {
                peak.min = peak.min.min(source_peak.min);
                peak.max = peak.max.max(source_peak.max);
            }
            if peak.min > peak.max {
                peak = WaveformPeak::default();
            }
            columns.push(peak);
        }
        columns
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceFingerprint {
    size: u64,
    modified_nanos: u64,
}

#[derive(Clone, Copy, Debug)]
struct WaveformHeader {
    fingerprint: SourceFingerprint,
    sample_rate: u32,
    level_count: u32,
    total_samples: u64,
}

pub(super) fn thumbnail_path(project_root: &Path, asset: &MediaAsset) -> PathBuf {
    cache_directory(project_root).join(format!(
        "media-{:016x}-thumbnail.png",
        media_key(&asset.path)
    ))
}

pub(super) fn waveform_path(project_root: &Path, asset: &MediaAsset) -> PathBuf {
    cache_directory(project_root).join(format!(
        "media-{:016x}-waveform.ocwf",
        media_key(&asset.path)
    ))
}

fn media_key(path: &Path) -> u64 {
    // FNV-1a is fixed across Rust/toolchain releases and makes the cache key
    // independent of timeline-local asset IDs.
    path.to_string_lossy()
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
}

pub(super) fn cache_is_ready(project_root: &Path, asset: &MediaAsset) -> bool {
    let thumbnail_ready =
        asset.kind != MediaKind::Video || thumbnail_path(project_root, asset).is_file();
    let source = project_root.join(&asset.path);
    let waveform_ready =
        !asset.has_audio || waveform_cache_is_valid(&source, &waveform_path(project_root, asset));
    thumbnail_ready && waveform_ready
}

pub(super) fn prepare(
    project_root: &Path,
    asset: &MediaAsset,
) -> Result<Option<WaveformData>, String> {
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

    if asset.kind == MediaKind::Video && !thumbnail_path(project_root, asset).is_file() {
        generate_thumbnail(&source, &thumbnail_path(project_root, asset))?;
    }
    if !asset.has_audio {
        return Ok(None);
    }

    let waveform = waveform_path(project_root, asset);
    if !waveform_cache_is_valid(&source, &waveform) {
        generate_waveform(&source, &waveform)?;
    }
    match load_waveform_file(&source, &waveform) {
        Ok(waveform) => Ok(Some(waveform)),
        Err(_) => {
            generate_waveform(&source, &waveform)?;
            load_waveform_file(&source, &waveform).map(Some)
        }
    }
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

fn generate_waveform(source: &Path, output: &Path) -> Result<(), String> {
    let fingerprint = source_fingerprint(source)?;
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
    let mut builder = WaveformBuilder::new(WAVEFORM_FINE_SAMPLES_PER_PEAK);

    for (packet_stream, packet) in input.packets() {
        if packet_stream.index() != stream_index {
            continue;
        }
        decoder
            .send_packet(&packet)
            .map_err(|error| format!("could not decode waveform packet: {error}"))?;
        receive_waveform_samples(&mut decoder, &mut resampler, &mut builder)?;
    }
    let _ = decoder.send_eof();
    receive_waveform_samples(&mut decoder, &mut resampler, &mut builder)?;
    let total_samples = builder.total_samples;
    let finest = builder.finish();
    if finest.is_empty() || total_samples == 0 {
        return Err(format!(
            "could not decode audio samples from {}",
            source.display()
        ));
    }
    let levels = build_waveform_levels(finest);
    let waveform = WaveformData {
        sample_rate,
        total_samples,
        levels,
    };
    if source_fingerprint(source)? != fingerprint {
        return Err(format!(
            "{} changed while its waveform was being generated",
            source.display()
        ));
    }
    write_waveform_file(output, fingerprint, &waveform)
}

fn receive_waveform_samples(
    decoder: &mut ffmpeg::decoder::Audio,
    resampler: &mut ResamplingContext,
    builder: &mut WaveformBuilder,
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
            builder.push(f32::from_ne_bytes(sample.try_into().unwrap()));
        }
    }
    Ok(())
}

struct WaveformBuilder {
    samples_per_peak: u32,
    samples_in_peak: u32,
    current: WaveformPeak,
    peaks: Vec<WaveformPeak>,
    total_samples: u64,
}

impl WaveformBuilder {
    fn new(samples_per_peak: u32) -> Self {
        Self {
            samples_per_peak,
            samples_in_peak: 0,
            current: empty_peak(),
            peaks: Vec::new(),
            total_samples: 0,
        }
    }

    fn push(&mut self, sample: f32) {
        let sample = if sample.is_finite() {
            sample.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        self.current.min = self.current.min.min(sample);
        self.current.max = self.current.max.max(sample);
        self.samples_in_peak += 1;
        self.total_samples = self.total_samples.saturating_add(1);
        if self.samples_in_peak == self.samples_per_peak {
            self.peaks.push(self.current);
            self.current = empty_peak();
            self.samples_in_peak = 0;
        }
    }

    fn finish(mut self) -> Vec<WaveformPeak> {
        if self.samples_in_peak > 0 {
            self.peaks.push(self.current);
        }
        self.peaks
    }
}

fn build_waveform_levels(finest: Vec<WaveformPeak>) -> Vec<WaveformLevel> {
    let mut levels = vec![WaveformLevel {
        samples_per_peak: WAVEFORM_FINE_SAMPLES_PER_PEAK,
        peaks: finest,
    }];
    while levels.len() < MAX_WAVEFORM_LEVELS
        && levels.last().is_some_and(|level| level.peaks.len() > 1)
    {
        let previous = levels.last().expect("waveform has a finest level");
        let peaks = previous
            .peaks
            .chunks(WAVEFORM_LEVEL_REDUCTION)
            .map(|chunk| {
                chunk.iter().fold(empty_peak(), |mut peak, source| {
                    peak.min = peak.min.min(source.min);
                    peak.max = peak.max.max(source.max);
                    peak
                })
            })
            .collect();
        levels.push(WaveformLevel {
            samples_per_peak: previous
                .samples_per_peak
                .saturating_mul(WAVEFORM_LEVEL_REDUCTION as u32),
            peaks,
        });
    }
    levels
}

fn empty_peak() -> WaveformPeak {
    WaveformPeak {
        min: 1.0,
        max: -1.0,
    }
}

fn seconds_to_sample(seconds: f64, sample_rate: u32, total_samples: u64) -> u64 {
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0;
    }
    ((seconds * f64::from(sample_rate)).round().max(0.0) as u64).min(total_samples)
}

fn source_fingerprint(source: &Path) -> Result<SourceFingerprint, String> {
    let metadata = fs::metadata(source)
        .map_err(|error| format!("could not inspect {}: {error}", source.display()))?;
    let modified_nanos = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0);
    Ok(SourceFingerprint {
        size: metadata.len(),
        modified_nanos,
    })
}

fn waveform_cache_is_valid(source: &Path, cache: &Path) -> bool {
    let Ok(fingerprint) = source_fingerprint(source) else {
        return false;
    };
    let Ok(file) = File::open(cache) else {
        return false;
    };
    let mut reader = BufReader::new(file);
    read_waveform_header(&mut reader).is_ok_and(|header| {
        header.fingerprint == fingerprint
            && header.sample_rate > 0
            && header.total_samples > 0
            && header.level_count > 0
            && header.level_count as usize <= MAX_WAVEFORM_LEVELS
    })
}

fn write_waveform_file(
    output: &Path,
    fingerprint: SourceFingerprint,
    waveform: &WaveformData,
) -> Result<(), String> {
    let temporary = output.with_extension("ocwf.part");
    let file = File::create(&temporary)
        .map_err(|error| format!("could not create {}: {error}", temporary.display()))?;
    let mut writer = BufWriter::new(file);
    writer
        .write_all(&WAVEFORM_MAGIC)
        .and_then(|_| write_u32(&mut writer, WAVEFORM_VERSION))
        .and_then(|_| write_u64(&mut writer, fingerprint.size))
        .and_then(|_| write_u64(&mut writer, fingerprint.modified_nanos))
        .and_then(|_| write_u32(&mut writer, waveform.sample_rate))
        .and_then(|_| write_u32(&mut writer, waveform.levels.len() as u32))
        .and_then(|_| write_u64(&mut writer, waveform.total_samples))
        .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;

    let mut data_offset =
        WAVEFORM_HEADER_SIZE + WAVEFORM_LEVEL_HEADER_SIZE * waveform.levels.len() as u64;
    for level in &waveform.levels {
        write_u32(&mut writer, level.samples_per_peak)
            .and_then(|_| write_u64(&mut writer, level.peaks.len() as u64))
            .and_then(|_| write_u64(&mut writer, data_offset))
            .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
        data_offset = data_offset.saturating_add(level.peaks.len() as u64 * 4);
    }
    for level in &waveform.levels {
        for peak in &level.peaks {
            write_i16(&mut writer, normalized_to_i16(peak.min))
                .and_then(|_| write_i16(&mut writer, normalized_to_i16(peak.max)))
                .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
        }
    }
    writer
        .flush()
        .map_err(|error| format!("could not finish {}: {error}", temporary.display()))?;
    drop(writer);

    if let Err(error) = fs::rename(&temporary, output) {
        if output.is_file() {
            fs::remove_file(output)
                .map_err(|remove| format!("could not replace {}: {remove}", output.display()))?;
            fs::rename(&temporary, output)
                .map_err(|rename| format!("could not finish {}: {rename}", output.display()))?;
        } else {
            return Err(format!("could not finish {}: {error}", output.display()));
        }
    }
    Ok(())
}

fn load_waveform_file(source: &Path, cache: &Path) -> Result<WaveformData, String> {
    let fingerprint = source_fingerprint(source)?;
    let file = File::open(cache)
        .map_err(|error| format!("could not open {}: {error}", cache.display()))?;
    let file_length = file
        .metadata()
        .map_err(|error| format!("could not inspect {}: {error}", cache.display()))?
        .len();
    let mut reader = BufReader::new(file);
    let header = read_waveform_header(&mut reader)?;
    if header.fingerprint != fingerprint {
        return Err(format!("{} is stale", cache.display()));
    }
    if header.sample_rate == 0
        || header.total_samples == 0
        || header.level_count == 0
        || header.level_count as usize > MAX_WAVEFORM_LEVELS
    {
        return Err(format!("{} has an invalid header", cache.display()));
    }

    let mut level_headers = Vec::with_capacity(header.level_count as usize);
    for _ in 0..header.level_count {
        let samples_per_peak = read_u32(&mut reader)?;
        let peak_count = read_u64(&mut reader)?;
        let data_offset = read_u64(&mut reader)?;
        let data_length = peak_count
            .checked_mul(4)
            .ok_or_else(|| format!("{} has an invalid peak count", cache.display()))?;
        if samples_per_peak == 0
            || peak_count == 0
            || data_offset
                .checked_add(data_length)
                .is_none_or(|end| end > file_length)
        {
            return Err(format!("{} has an invalid level table", cache.display()));
        }
        level_headers.push((samples_per_peak, peak_count, data_offset));
    }

    let mut levels = Vec::with_capacity(level_headers.len());
    for (samples_per_peak, peak_count, data_offset) in level_headers {
        reader
            .seek(SeekFrom::Start(data_offset))
            .map_err(|error| format!("could not read {}: {error}", cache.display()))?;
        let mut peaks = Vec::with_capacity(peak_count as usize);
        for _ in 0..peak_count {
            peaks.push(WaveformPeak {
                min: i16_to_normalized(read_i16(&mut reader)?),
                max: i16_to_normalized(read_i16(&mut reader)?),
            });
        }
        levels.push(WaveformLevel {
            samples_per_peak,
            peaks,
        });
    }
    Ok(WaveformData {
        sample_rate: header.sample_rate,
        total_samples: header.total_samples,
        levels,
    })
}

fn read_waveform_header(reader: &mut impl Read) -> Result<WaveformHeader, String> {
    let mut magic = [0_u8; 4];
    reader
        .read_exact(&mut magic)
        .map_err(|error| format!("could not read waveform header: {error}"))?;
    if magic != WAVEFORM_MAGIC {
        return Err("waveform cache has an invalid magic number".to_string());
    }
    let version = read_u32(reader)?;
    if version != WAVEFORM_VERSION {
        return Err(format!("unsupported waveform cache version {version}"));
    }
    Ok(WaveformHeader {
        fingerprint: SourceFingerprint {
            size: read_u64(reader)?,
            modified_nanos: read_u64(reader)?,
        },
        sample_rate: read_u32(reader)?,
        level_count: read_u32(reader)?,
        total_samples: read_u64(reader)?,
    })
}

fn normalized_to_i16(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16
}

fn i16_to_normalized(value: i16) -> f32 {
    f32::from(value) / f32::from(i16::MAX)
}

fn write_u32(writer: &mut impl Write, value: u32) -> std::io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

fn write_u64(writer: &mut impl Write, value: u64) -> std::io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

fn write_i16(writer: &mut impl Write, value: i16) -> std::io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

fn read_u32(reader: &mut impl Read) -> Result<u32, String> {
    let mut bytes = [0_u8; 4];
    reader
        .read_exact(&mut bytes)
        .map_err(|error| format!("could not read waveform cache: {error}"))?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_u64(reader: &mut impl Read) -> Result<u64, String> {
    let mut bytes = [0_u8; 8];
    reader
        .read_exact(&mut bytes)
        .map_err(|error| format!("could not read waveform cache: {error}"))?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_i16(reader: &mut impl Read) -> Result<i16, String> {
    let mut bytes = [0_u8; 2];
    reader
        .read_exact(&mut bytes)
        .map_err(|error| format!("could not read waveform cache: {error}"))?;
    Ok(i16::from_le_bytes(bytes))
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
        let waveform = directory.join("waveform.ocwf");

        generate_thumbnail(&video_source, &thumbnail).unwrap();
        generate_waveform(&audio_source, &waveform).unwrap();

        let thumbnail_image = image::open(&thumbnail).unwrap();
        let waveform_data = load_waveform_file(&audio_source, &waveform).unwrap();
        assert_eq!(thumbnail_image.width(), THUMBNAIL_WIDTH);
        assert!(waveform_data.sample_rate > 0);
        assert!(waveform_data.total_samples > 0);
        assert!(waveform_data.levels.len() > 1);
        assert_eq!(waveform_data.levels[0].samples_per_peak, 64);
        assert_eq!(
            fs::read(&waveform).unwrap().get(..4),
            Some(WAVEFORM_MAGIC.as_slice())
        );
        assert_eq!(waveform_data.columns(0.0, 1.0, 320).len(), 320);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn cache_keys_are_stable_and_source_path_based() {
        let first = media_key(Path::new("media/first.mp4"));
        assert_eq!(first, media_key(Path::new("media/first.mp4")));
        assert_ne!(first, media_key(Path::new("media/second.mp4")));
    }
}
