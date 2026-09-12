use anyhow::{Context as _, Error, Result, bail};
use std::{
    path::Path,
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ffmpeg_next as ffmpeg;

use super::{Clock, State, Status, decoder::Control, lock, seconds};

/// Audio has its own demuxer: device backpressure must never delay video frames.
pub struct AudioWorker {
    pub ends: mpsc::Receiver<(u64, Duration)>,
    commands: mpsc::Sender<AudioRequest>,
    worker: Option<JoinHandle<()>>,
}

impl AudioWorker {
    pub fn open(path: &Path, origin: f64, shared: &Arc<Mutex<State>>) -> Result<Self> {
        let (commands, requests) = mpsc::channel();
        let (reporter, ends) = mpsc::channel();
        let (ready, completion) = mpsc::sync_channel(1);
        let path = path.to_owned();
        let shared = Arc::clone(shared);
        let worker = thread::Builder::new()
            .name("video2-audio".into())
            .spawn(move || {
                if let Err(error) = run(&path, origin, &shared, &requests, &reporter, &ready) {
                    lock(&shared).fail(&error);
                    let _ = ready.try_send(Err(error));
                }
            })
            .context(format!("Starting audio decoder at {}:{}", file!(), line!()))?;
        let audio = Self {
            ends,
            commands,
            worker: Some(worker),
        };
        completion.recv().context(format!(
            "Waiting for audio initialization at {}:{}",
            file!(),
            line!()
        ))??;
        Ok(audio)
    }

    pub fn seek(
        &self,
        position: Duration,
        generation: u64,
        control: &mut Control<'_>,
    ) -> Result<bool> {
        let (reply, completion) = mpsc::sync_channel(1);
        self.commands
            .send(AudioRequest::Seek {
                position,
                generation,
                reply,
            })
            .context(format!("Requesting audio seek at {}:{}", file!(), line!()))?;
        loop {
            if control.interrupted() {
                return Ok(false);
            }
            match completion.try_recv() {
                Ok(()) => return Ok(true),
                Err(mpsc::TryRecvError::Empty) => thread::sleep(Duration::from_millis(2)),
                Err(mpsc::TryRecvError::Disconnected) => {
                    bail!("Audio seek did not complete at {}:{}", file!(), line!())
                }
            }
        }
    }
}

impl Drop for AudioWorker {
    fn drop(&mut self) {
        let _ = self.commands.send(AudioRequest::Stop);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

enum AudioRequest {
    Seek {
        position: Duration,
        generation: u64,
        reply: mpsc::SyncSender<()>,
    },
    Stop,
}

struct Audio {
    index: usize,
    decoder: ffmpeg::decoder::Audio,
    resampler: ffmpeg::software::resampling::Context,
    time_base: f64,
    origin: f64,
    position: Option<f64>,
    discard_before: f64,
    channels: u16,
    rate: u32,
    blocks: mpsc::SyncSender<Block>,
    errors: mpsc::Receiver<cpal::StreamError>,
    _stream: cpal::Stream,
}

impl Audio {
    fn open(
        input: &ffmpeg::format::context::Input,
        origin: f64,
        shared: &Arc<Mutex<State>>,
    ) -> Result<Option<Self>> {
        let Some(track) = input.streams().best(ffmpeg::media::Type::Audio) else {
            return Ok(None);
        };
        let decoder = ffmpeg::codec::context::Context::from_parameters(track.parameters())
            .context(format!(
                "Reading audio parameters at {}:{}",
                file!(),
                line!()
            ))?
            .decoder()
            .audio()
            .context(format!("Opening audio decoder at {}:{}", file!(), line!()))?;
        let Some(device) = cpal::default_host().default_output_device() else {
            bail!("No default audio output device at {}:{}", file!(), line!());
        };
        let config = f32_config(&device)?;
        let channels = config.channels;
        let rate = config.sample_rate;
        let resampler = resampler(&decoder, channels, rate)?;
        let (blocks, receiver) = mpsc::sync_channel(64);
        let (reporter, errors) = mpsc::sync_channel(4);
        let stream = output(&device, &config, receiver, reporter, Arc::clone(shared))?;
        stream
            .play()
            .context(format!("Starting audio output at {}:{}", file!(), line!()))?;
        Ok(Some(Self {
            index: track.index(),
            decoder,
            resampler,
            time_base: f64::from(track.time_base()),
            origin,
            position: None,
            discard_before: 0.0,
            channels,
            rate,
            blocks,
            errors,
            _stream: stream,
        }))
    }

    fn check(&self) -> Result<()> {
        if let Ok(error) = self.errors.try_recv() {
            bail!("Audio output failed: {error} at {}:{}", file!(), line!());
        }
        Ok(())
    }

    fn reset(&mut self, position: Duration) -> Result<()> {
        self.decoder.flush();
        self.resampler = resampler(&self.decoder, self.channels, self.rate)?;
        self.position = None;
        self.discard_before = position.as_secs_f64();
        Ok(())
    }

    fn receive(&mut self, generation: u64, control: &mut Control<'_, AudioRequest>) -> Result<()> {
        loop {
            if control.interrupted() || control.superseded(generation) {
                return Ok(());
            }
            let mut decoded = ffmpeg::frame::Audio::empty();
            match self.decoder.receive_frame(&mut decoded) {
                Ok(()) => {}
                Err(ffmpeg::Error::Eof) => return Ok(()),
                Err(ffmpeg::Error::Other { errno }) if errno == ffmpeg::error::EAGAIN => {
                    return Ok(());
                }
                Err(error) => {
                    return Err(Error::new(error).context(format!(
                        "Decoding audio frame at {}:{}",
                        file!(),
                        line!()
                    )));
                }
            }
            if let Some(timestamp) = decoded.timestamp() {
                let time = timestamp as f64 * self.time_base - self.origin;
                // Keep resampler continuity, but retain genuine gaps in the source.
                if self.position.is_none() || (time - self.position.unwrap_or(time)).abs() > 0.05 {
                    self.position = Some(time);
                }
            }
            if self.position.is_none() {
                self.position = Some(self.discard_before);
            }
            if decoded.rate() == 0 {
                bail!(
                    "Audio frame has zero sample rate at {}:{}",
                    file!(),
                    line!()
                );
            }
            decoded.set_channel_layout(layout(decoded.channels(), decoded.channel_layout()));
            let capacity = (decoded.samples() as u64 * u64::from(self.rate))
                .div_ceil(u64::from(decoded.rate())) as usize
                + 256;
            let mut converted = frame(capacity, self.channels);
            self.resampler
                .run(&decoded, &mut converted)
                .context(format!("Resampling audio at {}:{}", file!(), line!()))?;
            self.enqueue(&converted, generation, control)?;
        }
    }

    fn finish(&mut self, generation: u64, control: &mut Control<'_, AudioRequest>) -> Result<()> {
        self.decoder.send_eof().context(format!(
            "Draining audio decoder at {}:{}",
            file!(),
            line!()
        ))?;
        self.receive(generation, control)?;
        loop {
            if control.interrupted() {
                return Ok(());
            }
            let mut tail = frame(4096, self.channels);
            let delay = self.resampler.flush(&mut tail).context(format!(
                "Draining audio resampler at {}:{}",
                file!(),
                line!()
            ))?;
            self.enqueue(&tail, generation, control)?;
            if delay.is_none() {
                return Ok(());
            }
        }
    }

    fn enqueue(
        &mut self,
        frame: &ffmpeg::frame::Audio,
        generation: u64,
        control: &mut Control<'_, AudioRequest>,
    ) -> Result<()> {
        let time = self.position.unwrap_or(self.discard_before);
        self.position = Some(time + frame.samples() as f64 / f64::from(self.rate));
        let skip = ((self.discard_before - time).max(0.0) * f64::from(self.rate)).ceil() as usize;
        let skip = skip.min(frame.samples());
        if skip == frame.samples() {
            return Ok(());
        }
        let start = skip * usize::from(self.channels) * size_of::<f32>();
        let end = frame.samples() * usize::from(self.channels) * size_of::<f32>();
        let source = frame.data(0).get(start..end).context(format!(
            "Truncated resampled audio at {}:{}",
            file!(),
            line!()
        ))?;
        let count = source.len() / size_of::<f32>();
        let mut samples = Vec::<f32>::with_capacity(count);
        // SAFETY: the resampler produces native packed f32. The checked byte
        // slice initializes exactly count elements of the separate allocation;
        // every f32 bit pattern is valid. No source alignment is required.
        unsafe {
            std::ptr::copy_nonoverlapping(
                source.as_ptr(),
                samples.as_mut_ptr().cast::<u8>(),
                source.len(),
            );
            samples.set_len(count);
        }
        let mut block = Block {
            generation,
            time: time + skip as f64 / f64::from(self.rate),
            samples,
        };
        loop {
            if control.interrupted() || control.superseded(generation) {
                return Ok(());
            }
            self.check()?;
            match self.blocks.try_send(block) {
                Ok(()) => return Ok(()),
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    bail!("Audio output disconnected at {}:{}", file!(), line!())
                }
                Err(mpsc::TrySendError::Full(returned)) => {
                    block = returned;
                    thread::sleep(Duration::from_millis(2));
                }
            }
        }
    }
}

const OUTPUT: ffmpeg::format::Sample =
    ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed);

fn run(
    path: &Path,
    origin: f64,
    shared: &Arc<Mutex<State>>,
    commands: &mpsc::Receiver<AudioRequest>,
    ends: &mpsc::Sender<(u64, Duration)>,
    ready: &mpsc::SyncSender<Result<()>>,
) -> Result<()> {
    let mut input = ffmpeg::format::input(path).context(format!(
        "Opening audio input at {}:{}",
        file!(),
        line!()
    ))?;
    let Some(mut audio) = Audio::open(&input, origin, shared)? else {
        bail!(
            "Audio track disappeared while opening playback at {}:{}",
            file!(),
            line!()
        );
    };
    let _ = ready.try_send(Ok(()));
    let mut control = Control::new(shared, commands);
    let mut generation = 0;
    let mut ended = false;
    loop {
        control.interrupted();
        if matches!(lock(shared).status, Status::Stopped | Status::Failed(_)) {
            return Ok(());
        }
        if let Some(request) = control.request.take() {
            match request {
                AudioRequest::Stop => return Ok(()),
                AudioRequest::Seek {
                    position,
                    generation: epoch,
                    reply,
                } => {
                    let timestamp = ((position.as_secs_f64() + origin)
                        * ffmpeg::ffi::AV_TIME_BASE as f64)
                        as i64;
                    if input.seek(timestamp, ..timestamp).is_err() {
                        let beginning = (origin * ffmpeg::ffi::AV_TIME_BASE as f64) as i64;
                        input.seek(beginning, ..beginning).context(format!(
                            "Seeking audio from media origin at {}:{}",
                            file!(),
                            line!()
                        ))?;
                    }
                    audio.reset(position)?;
                    generation = epoch;
                    ended = false;
                    let _ = reply.try_send(());
                }
            }
        }
        audio.check()?;
        if control.superseded(generation) {
            // The video request has advanced the generation, but its audio
            // target has not arrived yet. Do not decode obsolete audio while
            // the device discards it: that competes with seek computation.
            thread::sleep(Duration::from_millis(1));
            continue;
        }
        if ended {
            thread::sleep(Duration::from_millis(2));
            continue;
        }
        let mut packet = ffmpeg::Packet::empty();
        match packet.read(&mut input) {
            Ok(()) => {
                if packet.stream() == audio.index {
                    audio.decoder.send_packet(&packet).context(format!(
                        "Sending audio packet at {}:{}",
                        file!(),
                        line!()
                    ))?;
                    audio.receive(generation, &mut control)?;
                }
            }
            Err(ffmpeg::Error::Eof) => {
                audio.finish(generation, &mut control)?;
                if control.interrupted() {
                    continue;
                }
                let _ = ends.send((generation, seconds(audio.position.unwrap_or(0.0))));
                ended = true;
            }
            Err(error) => {
                return Err(Error::new(error).context(format!(
                    "Reading audio packet at {}:{}",
                    file!(),
                    line!()
                )));
            }
        }
    }
}

struct Block {
    generation: u64,
    time: f64,
    samples: Vec<f32>,
}

#[derive(Clone, Copy)]
struct Snapshot {
    clock: Clock,
    generation: u64,
    gain: f32,
}

struct Output {
    blocks: mpsc::Receiver<Block>,
    pending: Option<Block>,
    cursor: usize,
    channels: usize,
    rate: f64,
}

impl Output {
    fn fill(&mut self, buffer: &mut [f32], snapshot: Snapshot, now: Instant) {
        buffer.fill(0.0);
        let playing = matches!(snapshot.clock, Clock::Playing { .. });
        let head = snapshot.clock.position(now).as_secs_f64();
        let mut written = 0;
        while written < buffer.len() {
            if let Some(block) = &self.pending
                && (block.generation != snapshot.generation || self.cursor >= block.samples.len())
            {
                self.pending = None;
            }
            if self.pending.is_none() {
                let Ok(block) = self.blocks.try_recv() else {
                    return;
                };
                self.pending = Some(block);
                self.cursor = 0;
                continue;
            }
            let Some(block) = &self.pending else {
                return;
            };
            if !playing {
                return;
            }
            let need = head + (written / self.channels) as f64 / self.rate;
            let have = block.time + (self.cursor / self.channels) as f64 / self.rate;
            if need - have > 0.05 {
                let stale = (((need - have) * self.rate) as usize * self.channels)
                    .min(block.samples.len() - self.cursor);
                self.cursor += stale;
                continue;
            }
            if have - need > 0.05 {
                let idle = (((have - need) * self.rate) as usize * self.channels)
                    .min(buffer.len() - written);
                written += idle;
                continue;
            }
            let count = (buffer.len() - written).min(block.samples.len() - self.cursor);
            for index in 0..count {
                buffer[written + index] = block.samples[self.cursor + index] * snapshot.gain;
            }
            written += count;
            self.cursor += count;
        }
    }
}

fn output(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    blocks: mpsc::Receiver<Block>,
    reporter: mpsc::SyncSender<cpal::StreamError>,
    shared: Arc<Mutex<State>>,
) -> Result<cpal::Stream> {
    let mut snapshot = Snapshot {
        clock: Clock::Paused(Duration::ZERO),
        generation: 0,
        gain: 1.0,
    };
    let mut output = Output {
        blocks,
        pending: None,
        cursor: 0,
        channels: usize::from(config.channels),
        rate: f64::from(config.sample_rate),
    };
    device
        .build_output_stream(
            config,
            move |buffer: &mut [f32], info: &cpal::OutputCallbackInfo| {
                // The device callback never waits on the playback mutex or a queue.
                if let Ok(state) = shared.try_lock() {
                    snapshot = Snapshot {
                        clock: state.clock,
                        generation: state.generation,
                        gain: if state.muted {
                            0.0
                        } else {
                            state.volume as f32
                        },
                    };
                }
                let timestamp = info.timestamp();
                let latency = timestamp
                    .playback
                    .duration_since(&timestamp.callback)
                    .unwrap_or_default();
                output.fill(buffer, snapshot, Instant::now() + latency);
            },
            move |error| {
                let _ = reporter.try_send(error);
            },
            None,
        )
        .context(format!(
            "Creating audio output stream at {}:{}",
            file!(),
            line!()
        ))
}

fn resampler(
    decoder: &ffmpeg::decoder::Audio,
    channels: u16,
    rate: u32,
) -> Result<ffmpeg::software::resampling::Context> {
    ffmpeg::software::resampling::Context::get(
        decoder.format(),
        layout(decoder.channels(), decoder.channel_layout()),
        decoder.rate(),
        OUTPUT,
        ffmpeg::ChannelLayout::default(i32::from(channels)),
        rate,
    )
    .context(format!(
        "Creating audio resampler at {}:{}",
        file!(),
        line!()
    ))
}

fn frame(samples: usize, channels: u16) -> ffmpeg::frame::Audio {
    ffmpeg::frame::Audio::new(
        OUTPUT,
        samples,
        ffmpeg::ChannelLayout::default(i32::from(channels)),
    )
}

fn layout(channels: u16, declared: ffmpeg::ChannelLayout) -> ffmpeg::ChannelLayout {
    if declared.is_empty() {
        ffmpeg::ChannelLayout::default(i32::from(channels))
    } else {
        declared
    }
}

fn f32_config(device: &cpal::Device) -> Result<cpal::StreamConfig> {
    let default = device.default_output_config().context(format!(
        "Reading audio configuration at {}:{}",
        file!(),
        line!()
    ))?;
    if default.sample_format() == cpal::SampleFormat::F32 {
        return Ok(default.config());
    }
    for range in device.supported_output_configs().context(format!(
        "Listing audio configurations at {}:{}",
        file!(),
        line!()
    ))? {
        if range.sample_format() == cpal::SampleFormat::F32 {
            return Ok(range.with_max_sample_rate().config());
        }
    }
    bail!(
        "No f32 audio output configuration at {}:{}",
        file!(),
        line!()
    )
}

#[cfg(test)]
#[path = "audio.test.rs"]
mod tests;
