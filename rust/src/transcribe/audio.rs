use super::error::{Error, Result};
use ffmpeg_next as ffmpeg;
use std::{collections::VecDeque, path::Path, time::Duration};

/// Probe the audio's source-relative end time without decoding its samples.
/// Unknown duration is left to the caller's decoded-audio validation.
pub fn audio_duration(path: &Path) -> Result<Option<Duration>> {
    if let Err(error) = ffmpeg::init() {
        return Err(Error::new("ffmpeg_init", error, file!(), line!()));
    }
    let input = match ffmpeg::format::input(path) {
        Ok(input) => input,
        Err(error) => return Err(Error::new("unreadable_media", error, file!(), line!())),
    };
    let Some(stream) = input.streams().best(ffmpeg::media::Type::Audio) else {
        return Err(Error::new(
            "missing_audio",
            "file contains no audio stream",
            file!(),
            line!(),
        ));
    };
    if stream.duration() <= 0 {
        return Ok(None);
    }
    let time_base = f64::from(stream.time_base());
    let container_start = unsafe { (*input.as_ptr()).start_time };
    let origin = if container_start == ffmpeg::ffi::AV_NOPTS_VALUE {
        0.0
    } else {
        container_start as f64 / ffmpeg::ffi::AV_TIME_BASE as f64
    };
    let start = if stream.start_time() == ffmpeg::ffi::AV_NOPTS_VALUE {
        0.0
    } else {
        (stream.start_time() as f64 * time_base - origin).max(0.0)
    };
    let duration = start + stream.duration() as f64 * time_base;
    if !duration.is_finite() || duration <= 0.0 {
        return Ok(None);
    }
    match Duration::try_from_secs_f64(duration) {
        Ok(duration) => Ok(Some(duration)),
        Err(error) => Err(Error::new("invalid_duration", error, file!(), line!())),
    }
}

/// Decode the whole audio stream into a mono 16 kHz PCM WAV.
/// Timestamps remain relative to the source container, including gaps before speech.
pub fn extract_audio_as_wav(path: &Path) -> Result<Vec<u8>> {
    const RATE: u32 = 16_000;
    match ffmpeg::init() {
        Ok(value) => value,
        Err(error) => return Err(Error::new("ffmpeg_init", error, file!(), line!())),
    };
    let mut reader = AudioReader::open(path, RATE)?;
    let mut wav = vec![0_u8; 44];
    while !reader.drained {
        // ffmpeg-next's delay() rounds to whole seconds before testing for zero.
        // Query in output samples so short resampler delays do not become gaps.
        let delay =
            unsafe { ffmpeg::ffi::swr_get_delay(reader.resampler.as_mut_ptr(), RATE as i64) };
        reader.advance(delay)?;
        let end = reader.queue_start as i128 + reader.queue.len() as i128;
        if end > i128::from((u32::MAX - 36) / 2) {
            return Err(Error::new(
                "audio_too_large",
                "decoded audio exceeds the PCM WAV size limit",
                file!(),
                line!(),
            ));
        }
        for (index, sample) in reader.queue.drain(..).enumerate() {
            let position = reader.queue_start as i128 + index as i128;
            let written = (wav.len() - 44) / 2;
            if position < written as i128 {
                continue;
            }
            // Silence preserves source-relative positions across timestamp gaps.
            wav.resize(44 + position as usize * 2, 0);
            let mono = ((sample[0] + sample[1]) * 0.5).clamp(-1.0, 1.0);
            wav.extend_from_slice(&((mono * i16::MAX as f32).round() as i16).to_le_bytes());
        }
    }
    let size = (wav.len() - 44) as u32;
    if size == 0 {
        return Err(Error::new(
            "missing_audio",
            "no audio samples decoded",
            file!(),
            line!(),
        ));
    }
    wav[0..4].copy_from_slice(b"RIFF");
    wav[4..8].copy_from_slice(&(size + 36).to_le_bytes());
    wav[8..16].copy_from_slice(b"WAVEfmt ");
    wav[16..20].copy_from_slice(&16_u32.to_le_bytes());
    wav[20..22].copy_from_slice(&1_u16.to_le_bytes());
    wav[22..24].copy_from_slice(&1_u16.to_le_bytes());
    wav[24..28].copy_from_slice(&RATE.to_le_bytes());
    wav[28..32].copy_from_slice(&(RATE * 2).to_le_bytes());
    wav[32..34].copy_from_slice(&2_u16.to_le_bytes());
    wav[34..36].copy_from_slice(&16_u16.to_le_bytes());
    wav[36..40].copy_from_slice(b"data");
    wav[40..44].copy_from_slice(&size.to_le_bytes());
    Ok(wav)
}

pub struct AudioReader {
    input: ffmpeg::format::context::Input,
    decoder: ffmpeg::decoder::Audio,
    resampler: ffmpeg::software::resampling::Context,
    stream: usize,
    time_base: f64,
    origin: f64,
    rate: u32,
    queue: VecDeque<[f32; 2]>,
    queue_start: i64,
    next_sample: Option<i64>,
    eof: bool,
    drained: bool,
}

impl AudioReader {
    pub fn open(path: &Path, rate: u32) -> Result<Self> {
        let input = match ffmpeg::format::input(path) {
            Ok(value) => value,
            Err(error) => return Err(Error::new("unreadable_media", error, file!(), line!())),
        };
        let Some(stream) = input.streams().best(ffmpeg::media::Type::Audio) else {
            return Err(Error::new(
                "missing_audio",
                "no audio stream",
                file!(),
                line!(),
            ));
        };
        let context = match ffmpeg::codec::context::Context::from_parameters(stream.parameters()) {
            Ok(value) => value,
            Err(error) => return Err(Error::new("decode_failure", error, file!(), line!())),
        };
        let mut decoder = match context.decoder().audio() {
            Ok(value) => value,
            Err(error) => return Err(Error::new("decode_failure", error, file!(), line!())),
        };
        if decoder.channel_layout().is_empty() {
            decoder.set_channel_layout(ffmpeg::ChannelLayout::default(decoder.channels() as i32));
        }
        let resampler = match ffmpeg::software::resampling::Context::get(
            decoder.format(),
            decoder.channel_layout(),
            decoder.rate(),
            ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
            ffmpeg::ChannelLayout::STEREO,
            rate,
        ) {
            Ok(value) => value,
            Err(error) => return Err(Error::new("resample_failure", error, file!(), line!())),
        };
        Ok(Self {
            stream: stream.index(),
            time_base: f64::from(stream.time_base()),
            origin: {
                let start = unsafe { (*input.as_ptr()).start_time };
                if start == ffmpeg::ffi::AV_NOPTS_VALUE {
                    0.0
                } else {
                    start as f64 / ffmpeg::ffi::AV_TIME_BASE as f64
                }
            },
            input,
            decoder,
            resampler,
            rate,
            queue: VecDeque::new(),
            queue_start: 0,
            next_sample: None,
            eof: false,
            drained: false,
        })
    }

    pub fn read(&mut self, start: i64, count: usize) -> Result<Vec<[f32; 2]>> {
        if self.next_sample.is_none() && start > self.rate as i64 {
            let seek = ((start as f64 / self.rate as f64 - 1.0 + self.origin)
                * ffmpeg::ffi::AV_TIME_BASE as f64) as i64;
            match self.input.seek(seek, ..seek) {
                Ok(value) => value,
                Err(error) => return Err(Error::new("seek_failure", error, file!(), line!())),
            };
            self.decoder.flush();
        }
        let mut result = vec![[0.0; 2]; count];
        for (i, target) in (start..start + count as i64).enumerate() {
            loop {
                while !self.queue.is_empty() && self.queue_start < target {
                    self.queue.pop_front();
                    self.queue_start += 1;
                }
                if self.queue_start == target && !self.queue.is_empty() {
                    result[i] = self.queue.pop_front().unwrap();
                    self.queue_start += 1;
                    break;
                }
                if self.queue_start > target || self.drained {
                    break;
                }
                let delay = match self.resampler.delay() {
                    Some(delay) => delay.output,
                    None => 0,
                };
                self.advance(delay)?;
            }
        }
        Ok(result)
    }

    fn advance(&mut self, delay: i64) -> Result<()> {
        loop {
            let mut decoded = ffmpeg::frame::Audio::empty();
            match self.decoder.receive_frame(&mut decoded) {
                Ok(()) => {
                    let pts = decoded.timestamp();
                    let timestamp = match pts {
                        Some(pts) => Some(
                            ((pts as f64 * self.time_base - self.origin) * self.rate as f64).round()
                                as i64
                                - delay,
                        ),
                        None => None,
                    };
                    let start = match (self.next_sample, timestamp) {
                        (Some(sample), Some(timestamp)) if sample.abs_diff(timestamp) > 2 => {
                            timestamp
                        }
                        (Some(sample), _) => sample,
                        (None, Some(timestamp)) => timestamp,
                        (None, None) => {
                            return Err(Error::new(
                                "missing_pts",
                                "audio frame has no presentation timestamp",
                                file!(),
                                line!(),
                            ));
                        }
                    };
                    let capacity = ((decoded.samples() as u64 * self.rate as u64)
                        .div_ceil(self.decoder.rate() as u64)
                        + 256) as usize;
                    let mut converted = ffmpeg::frame::Audio::new(
                        ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
                        capacity,
                        ffmpeg::ChannelLayout::STEREO,
                    );
                    match self.resampler.run(&decoded, &mut converted) {
                        Ok(value) => value,
                        Err(error) => {
                            return Err(Error::new("resample_failure", error, file!(), line!()));
                        }
                    };
                    self.append(&converted, start);
                    return Ok(());
                }
                Err(ffmpeg::Error::Eof) => {
                    let mut converted = ffmpeg::frame::Audio::new(
                        ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
                        4096,
                        ffmpeg::ChannelLayout::STEREO,
                    );
                    let delay = match self.resampler.flush(&mut converted) {
                        Ok(value) => value,
                        Err(error) => {
                            return Err(Error::new("resample_failure", error, file!(), line!()));
                        }
                    };
                    self.append(&converted, self.next_sample.unwrap_or(0));
                    self.drained = delay.is_none() || converted.samples() == 0;
                    return Ok(());
                }
                Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => {}
                Err(error) => {
                    return Err(Error::new("decode_failure", error, file!(), line!()));
                }
            }
            if self.eof {
                self.drained = true;
                return Ok(());
            }
            let mut packet = ffmpeg::Packet::empty();
            match packet.read(&mut self.input) {
                Ok(()) => {
                    if packet.stream() == self.stream {
                        match self.decoder.send_packet(&packet) {
                            Ok(value) => value,
                            Err(error) => {
                                return Err(Error::new("decode_failure", error, file!(), line!()));
                            }
                        };
                    }
                }
                Err(ffmpeg::Error::Eof) => {
                    match self.decoder.send_eof() {
                        Ok(value) => value,
                        Err(error) => {
                            return Err(Error::new("decode_failure", error, file!(), line!()));
                        }
                    };
                    self.eof = true;
                }
                Err(error) => {
                    return Err(Error::new("decode_failure", error, file!(), line!()));
                }
            }
        }
    }

    fn append(&mut self, frame: &ffmpeg::frame::Audio, start: i64) {
        if self.queue.is_empty() {
            self.queue_start = start;
        }
        for pair in frame.data(0).chunks_exact(8).take(frame.samples()) {
            self.queue.push_back([
                f32::from_ne_bytes([pair[0], pair[1], pair[2], pair[3]]),
                f32::from_ne_bytes([pair[4], pair[5], pair[6], pair[7]]),
            ]);
        }
        self.next_sample = Some(start + frame.samples() as i64);
    }
}
