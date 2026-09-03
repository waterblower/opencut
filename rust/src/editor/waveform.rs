use anyhow::{Context as _, Result, anyhow};
use ffmpeg::{
    channel_layout::ChannelLayout,
    codec, format, frame,
    media::Type,
    software::resampling::Context as ResamplingContext,
    util::format::{Sample, sample::Type as SampleType},
};
use ffmpeg_next as ffmpeg;
use gst::prelude::*;
use gstreamer as gst;
use gstreamer_app as gst_app;
use gstreamer_audio as gst_audio;
use std::{path::Path, time::Instant};
use url::Url;

const WAVEFORM_FINE_SAMPLES_PER_PEAK: u32 = 64;
const WAVEFORM_LEVEL_REDUCTION: usize = 4;
const MAX_WAVEFORM_LEVELS: usize = 32;
const MAX_RENDER_COLUMNS: usize = 30 * 4096;

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

pub(super) fn generate_waveform_gstreamer(source: &Path) -> Result<WaveformData> {
    gst::init().with_context(|| {
        format!(
            "could not initialize GStreamer for waveform generation at {}:{}",
            file!(),
            line!()
        )
    })?;
    let uri = Url::from_file_path(source).map_err(|_| {
        anyhow!(
            "could not convert {} to a file URL at {}:{}",
            source.display(),
            file!(),
            line!()
        )
    })?;
    let pipeline = gst::parse::launch(&format!(
        "uridecodebin uri=\"{}\" caps=audio/x-raw expose-all-streams=false ! audioconvert ! audio/x-raw,format=F32LE,layout=interleaved,channels=1 ! appsink name=waveform_sink sync=false",
        uri.as_str()
    ))
    .with_context(|| {
        format!(
            "could not create the waveform pipeline at {}:{}",
            file!(),
            line!()
        )
    })?
    .downcast::<gst::Pipeline>()
    .map_err(|_| {
        anyhow!(
            "waveform pipeline had an unexpected type at {}:{}",
            file!(),
            line!()
        )
    })?;
    let sink = pipeline
        .by_name("waveform_sink")
        .ok_or_else(|| {
            anyhow!(
                "waveform pipeline did not create its sink at {}:{}",
                file!(),
                line!()
            )
        })?
        .downcast::<gst_app::AppSink>()
        .map_err(|_| {
            anyhow!(
                "waveform sink had an unexpected type at {}:{}",
                file!(),
                line!()
            )
        })?;
    let bus = pipeline.bus().ok_or_else(|| {
        anyhow!(
            "waveform pipeline did not create a message bus at {}:{}",
            file!(),
            line!()
        )
    })?;
    let result = (|| -> Result<WaveformData> {
        pipeline.set_state(gst::State::Playing).with_context(|| {
            format!(
                "could not start waveform decoding at {}:{}",
                file!(),
                line!()
            )
        })?;
        let mut sample_rate = None;
        let mut builder = WaveformBuilder::new(WAVEFORM_FINE_SAMPLES_PER_PEAK);
        loop {
            if let Some(sample) = sink.try_pull_sample(gst::ClockTime::from_mseconds(100)) {
                let caps = sample.caps().ok_or_else(|| {
                    anyhow!("waveform sample had no caps at {}:{}", file!(), line!())
                })?;
                let info = gst_audio::AudioInfo::from_caps(caps).with_context(|| {
                    format!(
                        "waveform sample had invalid audio caps at {}:{}",
                        file!(),
                        line!()
                    )
                })?;
                if let Some(sample_rate) = sample_rate {
                    if sample_rate != info.rate() {
                        return Err(anyhow!(
                            "waveform sample rate changed from {sample_rate} to {} at {}:{}",
                            info.rate(),
                            file!(),
                            line!()
                        ));
                    }
                } else {
                    sample_rate = Some(info.rate());
                }
                let buffer = sample.buffer().ok_or_else(|| {
                    anyhow!("waveform sample had no buffer at {}:{}", file!(), line!())
                })?;
                let map = buffer.map_readable().with_context(|| {
                    format!("could not map waveform samples at {}:{}", file!(), line!())
                })?;
                let bytes = map.as_slice();
                if !bytes.len().is_multiple_of(size_of::<f32>()) {
                    return Err(anyhow!(
                        "waveform sample buffer had an invalid size at {}:{}",
                        file!(),
                        line!()
                    ));
                }
                builder.push_f32le_bytes(bytes);
                continue;
            }

            let Some(message) = bus.pop_filtered(&[gst::MessageType::Error, gst::MessageType::Eos])
            else {
                if sink.is_eos() {
                    break;
                }
                continue;
            };
            match message.view() {
                gst::MessageView::Eos(..) => break,
                gst::MessageView::Error(error) => {
                    return Err(anyhow!(
                        "GStreamer waveform decoding failed: {}{} at {}:{}",
                        error.error(),
                        error
                            .debug()
                            .map(|debug| format!(" ({debug})"))
                            .unwrap_or_default(),
                        file!(),
                        line!()
                    ));
                }
                _ => unreachable!("bus messages were filtered to error and EOS"),
            }
        }

        let sample_rate = sample_rate.ok_or_else(|| {
            anyhow!(
                "{} has no decodable audio stream at {}:{}",
                source.display(),
                file!(),
                line!()
            )
        })?;
        let total_samples = builder.total_samples;
        let finest = builder.finish();
        if finest.is_empty() || total_samples == 0 {
            return Err(anyhow!(
                "could not decode audio samples from {} at {}:{}",
                source.display(),
                file!(),
                line!()
            ));
        }
        Ok(WaveformData {
            sample_rate,
            total_samples,
            levels: build_waveform_levels(finest),
        })
    })();

    let stop_result = pipeline.set_state(gst::State::Null).with_context(|| {
        format!(
            "could not stop waveform decoding at {}:{}",
            file!(),
            line!()
        )
    });
    match result {
        Ok(waveform) => {
            stop_result?;
            Ok(waveform)
        }
        Err(error) => Err(error),
    }
}

pub(super) fn generate_waveform(source: &Path) -> Result<WaveformData> {
    let t = Instant::now();
    ffmpeg::init().map_err(|error| anyhow!("could not initialize FFmpeg: {error}"))?;
    let mut input = format::input(source)
        .map_err(|error| anyhow!("could not open {}: {error}", source.display()))?;
    let stream = input
        .streams()
        .best(Type::Audio)
        .ok_or_else(|| anyhow!("{} has no audio stream", source.display()))?;
    let stream_index = stream.index();
    let mut decoder = codec::context::Context::from_parameters(stream.parameters())
        .and_then(|context| context.decoder().audio())
        .map_err(|error| anyhow!("could not create audio decoder: {error}"))?;
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
    .map_err(|error| anyhow!("could not create waveform resampler: {error}"))?;
    let mut builder = WaveformBuilder::new(WAVEFORM_FINE_SAMPLES_PER_PEAK);

    for (packet_stream, packet) in input.packets() {
        if packet_stream.index() != stream_index {
            continue;
        }
        decoder
            .send_packet(&packet)
            .map_err(|error| anyhow!("could not decode waveform packet: {error}"))?;
        receive_waveform_samples(&mut decoder, &mut resampler, &mut builder)?;
    }
    let _ = decoder.send_eof();
    receive_waveform_samples(&mut decoder, &mut resampler, &mut builder)?;
    let total_samples = builder.total_samples;
    let finest = builder.finish();
    if finest.is_empty() || total_samples == 0 {
        return Err(anyhow!(
            "could not decode audio samples from {}",
            source.display()
        ));
    }
    eprintln!("time {}", t.elapsed().as_secs());

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
) -> anyhow::Result<()> {
    let mut decoded = frame::Audio::empty();
    while decoder.receive_frame(&mut decoded).is_ok() {
        let mut converted = frame::Audio::empty();
        resampler
            .run(&decoded, &mut converted)
            .map_err(|error| anyhow::anyhow!("could not resample waveform audio: {error}"))?;
        builder.push_samples(converted.plane::<f32>(0));
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

    fn push_samples(&mut self, mut samples: &[f32]) {
        self.total_samples = self.total_samples.saturating_add(samples.len() as u64);
        let samples_per_peak = self.samples_per_peak as usize;

        if self.samples_in_peak > 0 {
            let remaining = samples_per_peak - self.samples_in_peak as usize;
            let split = remaining.min(samples.len());
            include_samples(&mut self.current, &samples[..split]);
            self.samples_in_peak += split as u32;
            samples = &samples[split..];
            if self.samples_in_peak == self.samples_per_peak {
                self.peaks.push(self.current);
                self.current = empty_peak();
                self.samples_in_peak = 0;
            }
        }

        let mut chunks = samples.chunks_exact(samples_per_peak);
        for chunk in &mut chunks {
            let mut peak = empty_peak();
            include_samples(&mut peak, chunk);
            self.peaks.push(peak);
        }

        let remainder = chunks.remainder();
        if !remainder.is_empty() {
            include_samples(&mut self.current, remainder);
            self.samples_in_peak = remainder.len() as u32;
        }
    }

    fn push_f32le_bytes(&mut self, mut bytes: &[u8]) {
        if cfg!(target_endian = "little") {
            // GStreamer audio buffers are normally aligned for their negotiated sample type.
            // Every bit pattern is valid for f32, so an exactly aligned buffer can be consumed
            // directly without allocating or converting each sample.
            let (prefix, samples, suffix) = unsafe { bytes.align_to::<f32>() };
            if prefix.is_empty() && suffix.is_empty() {
                self.push_samples(samples);
                return;
            }
        }

        let bytes_per_sample = size_of::<f32>();
        self.total_samples = self
            .total_samples
            .saturating_add((bytes.len() / bytes_per_sample) as u64);
        let samples_per_peak = self.samples_per_peak as usize;

        if self.samples_in_peak > 0 {
            let remaining = samples_per_peak - self.samples_in_peak as usize;
            let split = remaining.min(bytes.len() / bytes_per_sample) * bytes_per_sample;
            include_f32le_bytes(&mut self.current, &bytes[..split]);
            self.samples_in_peak += (split / bytes_per_sample) as u32;
            bytes = &bytes[split..];
            if self.samples_in_peak == self.samples_per_peak {
                self.peaks.push(self.current);
                self.current = empty_peak();
                self.samples_in_peak = 0;
            }
        }

        let bytes_per_peak = samples_per_peak * bytes_per_sample;
        let mut chunks = bytes.chunks_exact(bytes_per_peak);
        for chunk in &mut chunks {
            let mut peak = empty_peak();
            include_f32le_bytes(&mut peak, chunk);
            self.peaks.push(peak);
        }

        let remainder = chunks.remainder();
        if !remainder.is_empty() {
            include_f32le_bytes(&mut self.current, remainder);
            self.samples_in_peak = (remainder.len() / bytes_per_sample) as u32;
        }
    }

    fn finish(mut self) -> Vec<WaveformPeak> {
        if self.samples_in_peak > 0 {
            self.peaks.push(self.current);
        }
        self.peaks
    }
}

fn include_samples(peak: &mut WaveformPeak, samples: &[f32]) {
    for &sample in samples {
        let sample = if sample.is_finite() {
            sample.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        peak.min = peak.min.min(sample);
        peak.max = peak.max.max(sample);
    }
}

fn include_f32le_bytes(peak: &mut WaveformPeak, bytes: &[u8]) {
    for bytes in bytes.chunks_exact(size_of::<f32>()) {
        let sample = f32::from_le_bytes(bytes.try_into().expect("chunk size is four"));
        let sample = if sample.is_finite() {
            sample.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        peak.min = peak.min.min(sample);
        peak.max = peak.max.max(sample);
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
