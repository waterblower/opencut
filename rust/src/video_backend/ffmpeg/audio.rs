use super::*;
use ffmpeg::{
    ChannelLayout,
    format::{Sample as AudioSampleFormat, sample::Type as AudioSampleType},
    frame::Audio as DecodedAudioFrame,
    software::resampling::Context as ResamplingContext,
};
use rodio::{
    ChannelCount, DeviceSinkBuilder, MixerDeviceSink, Player as AudioPlayer, SampleRate,
    buffer::SamplesBuffer,
};

const MAX_QUEUED_AUDIO_FRAMES: usize = 24;
const AUDIO_QUEUE_POLL_INTERVAL: Duration = Duration::from_millis(10);

pub(super) struct AudioOutput {
    pub(super) player: AudioPlayer,
    _device: MixerDeviceSink,
}

impl AudioOutput {
    pub(super) fn open(speed: f64) -> Option<Self> {
        let device = DeviceSinkBuilder::open_default_sink().ok()?;
        let player = AudioPlayer::connect_new(device.mixer());
        player.set_speed(speed as f32);
        Some(Self {
            player,
            _device: device,
        })
    }
}

pub(super) fn decode_audio(source: &str, state: &PlaybackState) -> Result<(), String> {
    let mut input =
        format::input(source).map_err(|error| format!("could not open audio: {error}"))?;
    let (stream_index, time_base, start_time, mut decoder) = {
        let Some(stream) = input.streams().best(Type::Audio) else {
            return Ok(());
        };
        let context = codec::context::Context::from_parameters(stream.parameters())
            .map_err(|error| format!("could not read audio parameters: {error}"))?;
        let decoder = context
            .decoder()
            .audio()
            .map_err(|error| format!("could not open audio decoder: {error}"))?;
        (
            stream.index(),
            stream.time_base(),
            stream.start_time(),
            decoder,
        )
    };

    let channel_layout = decoder_channel_layout(&decoder)?;
    let channels = ChannelCount::new(
        channel_layout
            .channels()
            .try_into()
            .map_err(|_| "audio has too many channels".to_string())?,
    )
    .ok_or_else(|| "audio has no channels".to_string())?;
    let sample_rate = SampleRate::new(decoder.rate())
        .ok_or_else(|| "audio has an invalid sample rate".to_string())?;
    let mut resampler = create_resampler(&decoder, channel_layout)?;
    let mut packet = Packet::empty();
    let mut seen_seek = state.audio_seek_serial.load(Ordering::Acquire);
    let mut discard_before = None;
    let mut fallback_position = Duration::ZERO;

    loop {
        if state.exit.load(Ordering::Acquire) {
            return Ok(());
        }

        let requested_seek = state.audio_seek_serial.load(Ordering::Acquire);
        if requested_seek != seen_seek {
            let target =
                Duration::from_nanos(state.audio_seek_target_nanos.load(Ordering::Acquire));
            reset_output(state);
            input
                .seek(duration_to_av_time(target), ..duration_to_av_time(target))
                .map_err(|error| format!("could not seek audio: {error}"))?;
            decoder.flush();
            resampler = create_resampler(&decoder, channel_layout)?;
            packet = Packet::empty();
            fallback_position = target;
            discard_before = Some(target);
            seen_seek = requested_seek;
        }

        match packet.read(&mut input) {
            Ok(()) => {
                if packet.stream() != stream_index {
                    packet = Packet::empty();
                    continue;
                }
                decoder
                    .send_packet(&packet)
                    .map_err(|error| format!("could not send audio packet: {error}"))?;
                packet = Packet::empty();
                if !receive_audio_frames(
                    &mut decoder,
                    &mut resampler,
                    state,
                    time_base,
                    start_time,
                    seen_seek,
                    channels,
                    sample_rate,
                    &mut discard_before,
                    &mut fallback_position,
                )? {
                    continue;
                }
            }
            Err(ffmpeg::Error::Eof) => {
                let _ = decoder.send_eof();
                let _ = receive_audio_frames(
                    &mut decoder,
                    &mut resampler,
                    state,
                    time_base,
                    start_time,
                    seen_seek,
                    channels,
                    sample_rate,
                    &mut discard_before,
                    &mut fallback_position,
                );

                let mut control = state.control.lock().unwrap();
                while !state.exit.load(Ordering::Acquire)
                    && state.audio_seek_serial.load(Ordering::Acquire) == seen_seek
                {
                    control = state.wake.wait(control).unwrap();
                }
            }
            Err(error) => return Err(format!("could not read audio packet: {error}")),
        }
    }
}

fn decoder_channel_layout(decoder: &ffmpeg::decoder::Audio) -> Result<ChannelLayout, String> {
    let layout = decoder.channel_layout();
    if !layout.is_empty() {
        return Ok(layout);
    }

    let channels = decoder.channels() as i32;
    if channels <= 0 {
        Err("audio has no channel layout".to_string())
    } else {
        Ok(ChannelLayout::default(channels))
    }
}

fn create_resampler(
    decoder: &ffmpeg::decoder::Audio,
    channel_layout: ChannelLayout,
) -> Result<ResamplingContext, String> {
    ResamplingContext::get(
        decoder.format(),
        channel_layout,
        decoder.rate(),
        AudioSampleFormat::F32(AudioSampleType::Packed),
        channel_layout,
        decoder.rate(),
    )
    .map_err(|error| format!("could not create audio resampler: {error}"))
}

#[allow(clippy::too_many_arguments)]
fn receive_audio_frames(
    decoder: &mut ffmpeg::decoder::Audio,
    resampler: &mut ResamplingContext,
    state: &PlaybackState,
    time_base: Rational,
    start_time: i64,
    seek_serial: u64,
    channels: ChannelCount,
    sample_rate: SampleRate,
    discard_before: &mut Option<Duration>,
    fallback_position: &mut Duration,
) -> Result<bool, String> {
    let mut decoded = DecodedAudioFrame::empty();
    while decoder.receive_frame(&mut decoded).is_ok() {
        if audio_seek_changed(state, seek_serial) {
            return Ok(false);
        }

        let position = decoded
            .timestamp()
            .or_else(|| decoded.pts())
            .and_then(|timestamp| timestamp_to_duration(timestamp, start_time, time_base))
            .unwrap_or(*fallback_position);
        let frame_duration =
            Duration::from_secs_f64(decoded.samples() as f64 / sample_rate.get() as f64);
        *fallback_position = position.saturating_add(frame_duration);

        let mut converted = DecodedAudioFrame::empty();
        resampler
            .run(&decoded, &mut converted)
            .map_err(|error| format!("could not convert audio frame: {error}"))?;
        let mut samples = packed_f32_samples(&converted, channels)?;

        if let Some(target) = *discard_before {
            let frame_end = position.saturating_add(Duration::from_secs_f64(
                converted.samples() as f64 / sample_rate.get() as f64,
            ));
            if frame_end <= target {
                continue;
            }

            let samples_per_channel = target
                .saturating_sub(position)
                .as_secs_f64()
                .mul_add(sample_rate.get() as f64, 0.0)
                .floor() as usize;
            let interleaved_offset = samples_per_channel
                .saturating_mul(channels.get() as usize)
                .min(samples.len());
            samples.drain(..interleaved_offset);
            *discard_before = None;
        }

        if samples.is_empty() {
            continue;
        }
        if !wait_for_audio_capacity(state, seek_serial) {
            return Ok(false);
        }
        if audio_seek_changed(state, seek_serial) {
            return Ok(false);
        }
        if let Some(output) = state.audio.lock().unwrap().as_ref() {
            output
                .player
                .append(SamplesBuffer::new(channels, sample_rate, samples));
        }
    }

    Ok(true)
}

fn packed_f32_samples(
    frame: &DecodedAudioFrame,
    channels: ChannelCount,
) -> Result<Vec<f32>, String> {
    let sample_count = frame
        .samples()
        .checked_mul(channels.get() as usize)
        .ok_or_else(|| "audio sample count overflowed".to_string())?;
    let byte_count = sample_count
        .checked_mul(size_of::<f32>())
        .ok_or_else(|| "audio sample buffer size overflowed".to_string())?;
    let bytes = frame
        .data(0)
        .get(..byte_count)
        .ok_or_else(|| "converted audio frame is shorter than expected".to_string())?;
    Ok(bytes
        .chunks_exact(size_of::<f32>())
        .map(|bytes| f32::from_ne_bytes(bytes.try_into().unwrap()))
        .collect())
}

fn wait_for_audio_capacity(state: &PlaybackState, seek_serial: u64) -> bool {
    loop {
        if state.exit.load(Ordering::Acquire) || audio_seek_changed(state, seek_serial) {
            return false;
        }
        let queued = state
            .audio
            .lock()
            .unwrap()
            .as_ref()
            .map_or(0, |output| output.player.len());
        if queued < MAX_QUEUED_AUDIO_FRAMES {
            return true;
        }

        let control = state.control.lock().unwrap();
        let _ = state
            .wake
            .wait_timeout(control, AUDIO_QUEUE_POLL_INTERVAL)
            .unwrap();
    }
}

fn reset_output(state: &PlaybackState) {
    let (paused, speed) = {
        let control = state.control.lock().unwrap();
        (control.paused, control.speed)
    };
    let volume = f64::from_bits(state.volume_bits.load(Ordering::Acquire)) as f32;
    let muted = state.muted.load(Ordering::Acquire);

    if let Some(output) = state.audio.lock().unwrap().as_ref() {
        output.player.clear();
        output.player.set_speed(speed as f32);
        output.player.set_volume(if muted { 0.0 } else { volume });
        if paused {
            output.player.pause();
        } else {
            output.player.play();
        }
    }
}

fn audio_seek_changed(state: &PlaybackState, seek_serial: u64) -> bool {
    state.audio_seek_serial.load(Ordering::Acquire) != seek_serial
}
