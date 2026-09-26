use crate::video3::{MediaInfo, MediaTime, timestamp_microseconds};
use anyhow::{Context, Result, bail};
use ffmpeg_next::{
    ChannelLayout, Error as FfmpegError, Packet, Rational, codec, decoder,
    ffi::{self, AVChannel},
    format,
    frame::Audio,
    util::error::EAGAIN,
};
use std::{marker::PhantomData, path::Path, ptr, rc::Rc, time::Duration};

/// Owned positions in interleaving order. The player selects the device's native
/// channel count and uses FFmpeg's default speaker order for that count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PcmFormat {
    pub sample_rate: u32,
    pub channel_layout: Vec<AVChannel>,
}

impl PcmFormat {
    pub fn default_layout(sample_rate: u32, channels: u16) -> Result<Self> {
        if sample_rate == 0 || channels == 0 || channels > 8 {
            bail!("unsupported device audio configuration");
        }
        let layout = ChannelLayout::default(i32::from(channels));
        let mut positions = Vec::with_capacity(channels as usize);
        for index in 0..channels {
            // SAFETY: default layout is valid and index is below its channel count.
            positions.push(unsafe {
                ffi::av_channel_layout_channel_from_index(&layout.0, index as u32)
            });
        }
        Ok(Self {
            sample_rate,
            channel_layout: positions,
        })
    }
}

pub struct AudioSamples {
    pub samples: Vec<f32>,
    pub timestamp: MediaTime,
    pub format: PcmFormat,
    pub frame_count: usize,
}

pub struct AudioDecoder {
    input: format::context::Input,
    decoder: decoder::Audio,
    stream_index: usize,
    time_base: Rational,
    origin: i64,
    drain: Drain,
    output: Option<PcmFormat>,
    resampler: Option<Resampler>,
    pending: Option<Audio>,
    trim_before: i64,
    _lane_local: PhantomData<Rc<()>>,
}

impl AudioDecoder {
    pub fn open(path: &Path, metadata: &MediaInfo) -> Result<Self> {
        let info = metadata
            .audio
            .as_ref()
            .context("media has no audio stream")?;
        Self::open_stream(path, info.stream_index, metadata.origin_microseconds)
    }

    pub fn open_stream(path: &Path, stream_index: usize, origin_microseconds: i64) -> Result<Self> {
        let input = format::input(path).context("opening independent audio demuxer")?;
        let stream = input
            .stream(stream_index)
            .context("audio stream is missing")?;
        let time_base = stream.time_base();
        let mut decoder = codec::context::Context::from_parameters(stream.parameters())?.decoder();
        decoder.set_packet_time_base(time_base);
        let decoder = decoder.audio().context("opening audio decoder")?;
        Ok(Self {
            input,
            decoder,
            stream_index,
            time_base,
            origin: origin_microseconds,
            drain: Drain::Reading,
            output: None,
            resampler: None,
            pending: None,
            trim_before: 0,
            _lane_local: PhantomData,
        })
    }

    pub fn configure_output(&mut self, format: &PcmFormat) -> Result<()> {
        let standard =
            PcmFormat::default_layout(format.sample_rate, format.channel_layout.len() as u16)?;
        if standard != *format {
            bail!("output requires the device's default channel layout");
        }
        self.output = Some(format.clone());
        self.resampler = None;
        Ok(())
    }

    pub fn next_samples(&mut self) -> Result<Option<AudioSamples>> {
        let output = self
            .output
            .clone()
            .context("audio output format is not configured")?;
        loop {
            let frame = match self.pending.take() {
                Some(frame) => Some(frame),
                None => self.decode_next()?,
            };
            let Some(frame) = frame else {
                if let Some(resampler) = &mut self.resampler
                    && let Some(samples) = resampler.convert(None, &output, self.trim_before)?
                {
                    return Ok(Some(samples));
                }
                self.resampler = None;
                self.drain = Drain::Drained; // 解码器和重采样尾部均已耗尽，才记录最终 EOF。
                return Ok(None);
            };
            let timestamp = match frame.timestamp().or(frame.pts()) {
                Some(timestamp) => timestamp_microseconds(timestamp, self.time_base)?
                    .checked_sub(self.origin)
                    .context("audio timestamp overflow")?,
                None => match &self.resampler {
                    Some(resampler) => resampler.expected_input,
                    None => bail!("first audio frame has no timestamp"),
                },
            };
            if frame.rate() == 0 {
                bail!("decoded audio has zero sample rate");
            }
            if let Some(resampler) = &mut self.resampler {
                // Preserve gaps instead of stretching contiguous PCM across them.
                let discontinuity = (i128::from(timestamp) - i128::from(resampler.expected_input)).abs() > 1000
                        || resampler.rate != frame.rate()
                        || resampler.sample_format != frame.format()
                        // SAFETY: resampler owns its layout; frame is borrowed
                        // for this call. Compare speaker positions, not just count.
                        || unsafe { ffi::av_channel_layout_compare(&resampler.layout, &(*frame.as_ptr()).ch_layout) != 0 };
                if discontinuity {
                    self.pending = Some(frame);
                    if let Some(samples) = resampler.convert(None, &output, self.trim_before)? {
                        return Ok(Some(samples));
                    }
                    self.resampler = None;
                    continue;
                }
            }
            if self.resampler.is_none() {
                self.resampler = Some(Resampler::new(&frame, timestamp, &output)?);
            }
            let resampler = self.resampler.as_mut().context("missing audio resampler")?;
            resampler.expected_input = timestamp
                + ((frame.samples() as i128 * 1_000_000) / i128::from(frame.rate())) as i64;
            if let Some(samples) = resampler.convert(Some(&frame), &output, self.trim_before)? {
                return Ok(Some(samples));
            }
        }
    }

    pub fn is_drained(&self) -> bool {
        self.drain == Drain::Drained // 不再产生 PCM；不代表输出设备已经播完。
    }

    /// 最近一次 seek 的目标；尚未 seek 时为零，不代表当前解码或播放位置。
    pub fn seek_position(&self) -> Duration {
        Duration::from_micros(self.trim_before.max(0) as u64)
    }

    pub fn seek(&mut self, position: Duration) -> Result<()> {
        let target = i64::try_from(position.as_micros()).unwrap_or(i64::MAX);
        let absolute = self.origin.saturating_add(target);
        self.input
            .seek(absolute, ..absolute)
            .context("seeking audio demuxer")?;
        self.decoder.flush();
        self.drain = Drain::Reading;
        self.pending = None;
        self.resampler = None;
        self.trim_before = target;
        Ok(())
    }
}

#[rustfmt::skip]
#[derive(PartialEq, Eq)]
enum Drain {
    Reading,           // 读取压缩包并解码。
    DrainingDecoder,   // 输入 EOF 已送入 FFmpeg，继续取出缓存的音频帧。
    DrainingResampler, // FFmpeg 已 EOF，继续取出重采样器缓存的 PCM。
    Drained,           // 所有 PCM 均已取出；seek 后回到 Reading。
}

impl AudioDecoder {
    fn decode_next(&mut self) -> Result<Option<Audio>> {
        if matches!(self.drain, Drain::DrainingResampler | Drain::Drained) {
            return Ok(None);
        }
        let mut frame = Audio::empty();
        loop {
            match self.decoder.receive_frame(&mut frame) {
                Ok(()) => return Ok(Some(frame)),
                Err(FfmpegError::Eof) => {
                    self.drain = Drain::DrainingResampler;
                    return Ok(None);
                }
                Err(FfmpegError::Other { errno: EAGAIN }) if self.drain == Drain::Reading => {}
                Err(error) => return Err(error).context("receiving decoded audio"),
            }
            let mut packet = Packet::empty();
            loop {
                match packet.read(&mut self.input) {
                    Ok(()) => {
                        if packet.stream() != self.stream_index {
                            packet = Packet::empty();
                            continue;
                        }
                        self.decoder
                            .send_packet(&packet)
                            .context("sending audio packet")?;
                        break;
                    }
                    Err(FfmpegError::Eof) => {
                        self.decoder.send_eof()?;
                        self.drain = Drain::DrainingDecoder;
                        break;
                    }
                    Err(error) => return Err(error).context("reading audio packet"),
                }
            }
        }
    }
}

struct Resampler {
    context: *mut ffi::SwrContext,
    origin: i64,
    emitted: u64,
    expected_input: i64,
    rate: u32,
    sample_format: ffmpeg_next::format::Sample,
    layout: ffi::AVChannelLayout,
}

impl Resampler {
    fn new(frame: &Audio, timestamp: i64, output: &PcmFormat) -> Result<Self> {
        let mut context = ptr::null_mut();
        let layout = ChannelLayout::default(output.channel_layout.len() as i32);
        // SAFETY: input frame/layout are alive during configuration. swr copies
        // layouts; the wrapper owns and frees the resulting context, even on error.
        let result = unsafe {
            ffi::swr_alloc_set_opts2(
                &mut context,
                &layout.0,
                ffi::AVSampleFormat::AV_SAMPLE_FMT_FLT,
                output.sample_rate as i32,
                &(*frame.as_ptr()).ch_layout,
                frame.format().into(),
                frame.rate() as i32,
                0,
                ptr::null_mut(),
            )
        };
        let mut owner = Self {
            context,
            origin: timestamp,
            emitted: 0,
            expected_input: timestamp,
            rate: frame.rate(),
            sample_format: frame.format(),
            layout: unsafe { std::mem::zeroed() },
        };
        if result < 0 {
            return Err(FfmpegError::from(result)).context("configuring audio resampler");
        }
        let result =
            unsafe { ffi::av_channel_layout_copy(&mut owner.layout, &(*frame.as_ptr()).ch_layout) };
        if result < 0 {
            return Err(FfmpegError::from(result)).context("copying decoded audio layout");
        }
        let result = unsafe { ffi::swr_init(owner.context) };
        if result < 0 {
            return Err(FfmpegError::from(result)).context("initializing audio resampler");
        }
        Ok(owner)
    }

    fn convert(
        &mut self,
        input: Option<&Audio>,
        output: &PcmFormat,
        trim_before: i64,
    ) -> Result<Option<AudioSamples>> {
        let (samples, data) = match input {
            Some(frame) => (frame.samples() as i32, unsafe {
                (*frame.as_ptr()).extended_data as *mut *const u8
            }),
            None => (0, ptr::null_mut()),
        };
        let capacity = unsafe { ffi::swr_get_out_samples(self.context, samples) };
        if capacity < 0 {
            return Err(FfmpegError::from(capacity)).context("sizing resampled audio");
        }
        let capacity = capacity.max(32);
        let channels = output.channel_layout.len();
        let mut pcm = vec![0_f32; capacity as usize * channels];
        let mut pointer = pcm.as_mut_ptr().cast::<u8>();
        // SAFETY: interleaved f32 output capacity covers capacity sample frames;
        // input planes belong to the borrowed native frame and are never mutated.
        let written =
            unsafe { ffi::swr_convert(self.context, &mut pointer, capacity, data, samples) };
        if written < 0 {
            return Err(FfmpegError::from(written)).context("resampling audio");
        }
        let start = self.origin
            + (u128::from(self.emitted) * 1_000_000 / u128::from(output.sample_rate)) as i64;
        self.emitted += written as u64;
        if written == 0 {
            return Ok(None);
        }
        let skip = if start < trim_before {
            (((i128::from(trim_before) - i128::from(start)) * i128::from(output.sample_rate)
                + 999_999)
                / 1_000_000)
                .min(i128::from(written)) as usize
        } else {
            0
        };
        if skip == written as usize {
            return Ok(None);
        }
        pcm.truncate(written as usize * channels);
        if skip > 0 {
            pcm.drain(..skip * channels);
        }
        Ok(Some(AudioSamples {
            samples: pcm,
            timestamp: MediaTime(
                start + (skip as u64 * 1_000_000 / u64::from(output.sample_rate)) as i64,
            ),
            format: output.clone(),
            frame_count: written as usize - skip,
        }))
    }
}

impl Drop for Resampler {
    fn drop(&mut self) {
        unsafe {
            ffi::swr_free(&mut self.context);
            ffi::av_channel_layout_uninit(&mut self.layout);
        }
    }
}
