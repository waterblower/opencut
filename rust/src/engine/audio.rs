use super::decode::{again, origin};
use crate::{
    cli_error, cli_try,
    core::{
        document::{Clip, Document},
        error::Result,
        validate::MediaInfo,
    },
};
use ffmpeg_next as ffmpeg;
use std::{
    collections::{HashMap, VecDeque},
    path::Path,
};

#[derive(Default)]
pub struct Mixer {
    readers: HashMap<String, AudioReader>,
}

impl Mixer {
    pub fn block(
        &mut self,
        doc: &Document,
        base: &Path,
        media: &HashMap<String, MediaInfo>,
        start: i64,
        count: usize,
    ) -> Result<Vec<[f32; 2]>> {
        let fps = doc.settings.frame_rate;
        let rate = doc.settings.sample_rate;
        let mut mixed = vec![[0.0_f32; 2]; count];
        let mut active = Vec::new();
        for clip in &doc.clips {
            let Clip::Media {
                common,
                asset_id,
                source_in,
                audio_properties,
                ..
            } = clip
            else {
                continue;
            };
            let Some(track) = doc.tracks.iter().find(|t| t.id == common.track_id) else {
                continue;
            };
            if track.muted || audio_properties.muted || !media[asset_id].audio {
                continue;
            }
            let mut clip_start = fps.samples(common.timeline_start, rate);
            let mut clip_end = fps.samples(clip.end(fps), rate);
            let mut incoming = None;
            let mut outgoing = None;
            for transition in &doc.transitions {
                if transition.from_clip != common.id && transition.to_clip != common.id {
                    continue;
                }
                let cut = doc
                    .clip(&transition.to_clip)
                    .unwrap()
                    .common()
                    .timeline_start;
                let begin = fps.samples(cut - transition.duration / 2, rate);
                let end = fps.samples(cut + transition.duration - transition.duration / 2, rate);
                if transition.to_clip == common.id {
                    clip_start = begin;
                    incoming = Some((begin, end));
                } else {
                    clip_end = end;
                    outgoing = Some((begin, end));
                }
            }
            let from = start.max(clip_start);
            let end = (start + count as i64).min(clip_end);
            if from >= end {
                continue;
            }
            active.push(common.id.clone());
            if !self.readers.contains_key(&common.id) {
                self.readers.insert(
                    common.id.clone(),
                    AudioReader::open(&doc.asset_path(asset_id, base)?, rate)?,
                );
            }
            let source =
                fps.samples(*source_in, rate) + from - fps.samples(common.timeline_start, rate);
            let samples = self
                .readers
                .get_mut(&common.id)
                .unwrap()
                .read(source, (end - from) as usize)?;
            let gain = 10_f64.powf(audio_properties.gain_db / 20.0) as f32;
            for (i, sample) in samples.iter().enumerate() {
                let time = from + i as i64;
                let mut envelope = 1.0_f64;
                if let Some((begin, end)) = incoming
                    && time < end
                {
                    envelope *= (((time - begin) as f64 / (end - begin) as f64)
                        * std::f64::consts::FRAC_PI_2)
                        .sin();
                }
                if let Some((begin, end)) = outgoing
                    && time >= begin
                {
                    envelope *= (((time - begin) as f64 / (end - begin) as f64)
                        * std::f64::consts::FRAC_PI_2)
                        .cos();
                }
                let index = (time - start) as usize;
                for channel in 0..2 {
                    mixed[index][channel] += sample[channel] * gain * envelope as f32;
                }
            }
        }
        self.readers.retain(|id, _| active.contains(id));
        for sample in &mut mixed {
            for channel in sample {
                *channel = channel.clamp(-1.0, 1.0);
            }
        }
        Ok(mixed)
    }
}

struct AudioReader {
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
    fn open(path: &Path, rate: u32) -> Result<Self> {
        let input = cli_try!(ffmpeg::format::input(path), "unreadable_media", "", 4);
        let Some(stream) = input.streams().best(ffmpeg::media::Type::Audio) else {
            return Err(cli_error!("missing_audio", "", 4, "no audio stream"));
        };
        let context = cli_try!(
            ffmpeg::codec::context::Context::from_parameters(stream.parameters()),
            "decode_failure",
            "",
            5
        );
        let mut decoder = cli_try!(context.decoder().audio(), "decode_failure", "", 5);
        if decoder.channel_layout().is_empty() {
            decoder.set_channel_layout(ffmpeg::ChannelLayout::default(decoder.channels() as i32));
        }
        let resampler = cli_try!(
            ffmpeg::software::resampling::Context::get(
                decoder.format(),
                decoder.channel_layout(),
                decoder.rate(),
                ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
                ffmpeg::ChannelLayout::STEREO,
                rate
            ),
            "resample_failure",
            "",
            5
        );
        Ok(Self {
            stream: stream.index(),
            time_base: f64::from(stream.time_base()),
            origin: origin(&input),
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

    fn read(&mut self, start: i64, count: usize) -> Result<Vec<[f32; 2]>> {
        if self.next_sample.is_none() && start > self.rate as i64 {
            let seek = ((start as f64 / self.rate as f64 - 1.0 + self.origin)
                * ffmpeg::ffi::AV_TIME_BASE as f64) as i64;
            cli_try!(self.input.seek(seek, ..seek), "seek_failure", "", 5);
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
                self.advance()?;
            }
        }
        Ok(result)
    }

    fn advance(&mut self) -> Result<()> {
        loop {
            let mut decoded = ffmpeg::frame::Audio::empty();
            match self.decoder.receive_frame(&mut decoded) {
                Ok(()) => {
                    let pts = decoded.timestamp();
                    let delay = match self.resampler.delay() {
                        Some(delay) => delay.output,
                        None => 0,
                    };
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
                            return Err(cli_error!(
                                "missing_pts",
                                "",
                                5,
                                "audio frame has no presentation timestamp"
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
                    cli_try!(
                        self.resampler.run(&decoded, &mut converted),
                        "resample_failure",
                        "",
                        5
                    );
                    self.append(&converted, start);
                    return Ok(());
                }
                Err(ffmpeg::Error::Eof) => {
                    let mut converted = ffmpeg::frame::Audio::new(
                        ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed),
                        4096,
                        ffmpeg::ChannelLayout::STEREO,
                    );
                    let delay = cli_try!(
                        self.resampler.flush(&mut converted),
                        "resample_failure",
                        "",
                        5
                    );
                    self.append(&converted, self.next_sample.unwrap_or(0));
                    self.drained = delay.is_none() || converted.samples() == 0;
                    return Ok(());
                }
                Err(error) if again(error) => {}
                Err(error) => return Err(cli_error!("decode_failure", "", 5, "{error}")),
            }
            if self.eof {
                self.drained = true;
                return Ok(());
            }
            let mut packet = ffmpeg::Packet::empty();
            match packet.read(&mut self.input) {
                Ok(()) => {
                    if packet.stream() == self.stream {
                        cli_try!(self.decoder.send_packet(&packet), "decode_failure", "", 5);
                    }
                }
                Err(ffmpeg::Error::Eof) => {
                    cli_try!(self.decoder.send_eof(), "decode_failure", "", 5);
                    self.eof = true;
                }
                Err(error) => return Err(cli_error!("decode_failure", "", 5, "{error}")),
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
