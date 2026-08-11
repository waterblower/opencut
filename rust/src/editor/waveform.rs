use ffmpeg::{
    channel_layout::ChannelLayout,
    codec, format, frame,
    media::Type,
    software::resampling::Context as ResamplingContext,
    util::format::{Sample, sample::Type as SampleType},
};
use ffmpeg_next as ffmpeg;
use std::{mem::size_of, path::Path};

const WAVEFORM_FINE_SAMPLES_PER_PEAK: u32 = 64;
const WAVEFORM_LEVEL_REDUCTION: usize = 4;
const MAX_WAVEFORM_LEVELS: usize = 32;
const MAX_RENDER_COLUMNS: usize = 2 * 4096;

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

pub(super) fn generate_waveform(source: &Path) -> Result<WaveformData, String> {
    ffmpeg::init().map_err(|error| format!("could not initialize FFmpeg: {error}"))?;
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

    Ok(WaveformData {
        sample_rate,
        total_samples,
        levels: build_waveform_levels(finest),
    })
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

#[cfg(test)]
#[path = "waveform.test.rs"]
mod tests;
