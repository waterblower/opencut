use crate::{MediaTime, hardware, time::timestamp_microseconds};
use anyhow::{Context, Result, bail};
use ffmpeg_next::{
    Discard, Error as FfmpegError, Packet, Rational, codec, decoder,
    ffi::av_display_rotation_get,
    format::{Pixel, context::Input},
    frame::{Video, side_data::Type as FrameSideData},
    media::Type as MediaType,
    packet::side_data::Type as PacketSideData,
    util::{color, error::EAGAIN},
};
use std::{
    collections::VecDeque,
    marker::PhantomData,
    rc::Rc,
    time::{Duration, Instant},
};
#[cfg(target_os = "macos")]
use {
    crate::videotoolbox::{DecodedPicture, Received, VideoToolboxDecoder},
    core_foundation::base::TCFType,
    core_video::pixel_buffer::{CVPixelBuffer, CVPixelBufferRef},
};

pub struct VideoFrame {
    /// The decoded picture. Both macOS decoding paths produce VideoToolbox buffers.
    #[cfg(target_os = "macos")]
    pub pixel_buffer: CVPixelBuffer,
    #[cfg(not(target_os = "macos"))]
    pub native: Video,
    pub timestamp: MediaTime,
    pub duration: Option<Duration>,
    pub color_range: color::Range,
    pub color_space: color::Space,
    pub color_primaries: color::Primaries,
    pub color_transfer: color::TransferCharacteristic,
    /// Counterclockwise display rotation, in degrees.
    pub rotation_degrees: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeMode {
    Software,
    VideoToolbox,
}

/// VideoToolbox on macOS and software decoding on other platforms, fixed at compile time.
const DECODE_MODE: DecodeMode = if cfg!(target_os = "macos") {
    DecodeMode::VideoToolbox
} else {
    DecodeMode::Software
};

/// Open, use, and drop on one execution lane. No scheduler or current UI frame.
pub struct VideoDecoder {
    input: Input,
    codec: Codec,
    stream_index: usize,
    time_base: Rational,
    origin_microseconds: i64,
    stream_rotation: f64,
    drain: DrainState,
    /// At most two frames: the next selected frame and any decoded successor.
    lookahead: VecDeque<VideoFrame>,
    // 零大小标记
    // 使解码器无法移到或共享给其他线程
    // FFmpeg 解码上下文和 VideoToolbox 会话
    // 须在同一线程打开、使用和释放
    _lane_local: PhantomData<Rc<()>>,
}

impl VideoDecoder {
    /// Decodes from an already open demuxer, which this decoder then owns. The input must
    /// not be shared with another decoder: each needs its own read position.
    /// Use VideoToolbox on macOS and software decoding on other platforms.
    /// Unsupported codecs and hardware failures are errors; there is no software fallback.
    pub fn from_av_input(
        input: Input,
        stream_index: usize,
        origin_microseconds: i64,
    ) -> Result<Self> {
        let (input, codec, time_base, stream_rotation) = open_decoder(input, stream_index)?;
        let mut decoder = Self {
            input,
            codec,
            stream_index,
            time_base,
            origin_microseconds,
            stream_rotation,
            drain: DrainState::Reading,
            lookahead: VecDeque::new(),
            _lane_local: PhantomData,
        };
        let Some(first) = decoder.decode_next(None)? else {
            bail!("video stream contains no decoded frames");
        };
        decoder.lookahead.push_back(first);
        Ok(decoder)
    }

    /// None is drained EOF; packet pumping and EAGAIN remain internal.
    pub fn next_frame(&mut self) -> Result<Option<VideoFrame>> {
        if let Some(frame) = self.lookahead.pop_front() {
            return Ok(Some(frame));
        }
        self.decode_next(None)
    }

    pub fn is_drained(&self) -> bool {
        self.drain == DrainState::Drained && self.lookahead.is_empty() // seek 留下的帧也已取完；不代表最后一帧已展示完。
    }

    /// Position the next pull at the nearest bracketing frame, earlier on ties.
    /// The selected frame and any decoded successor stay owned by the decoder.
    pub fn seek(&mut self, position: Duration) -> Result<()> {
        let started = Instant::now();
        let (frame, debug) = self.seek_inner(position)?;
        self.lookahead.push_front(frame);
        eprintln!(
            "seek to {} µs: total={:?}, index_seeks={}, sent_packets={}, discarded_packets={}, decoded_frames={}",
            position.as_micros(),
            started.elapsed(),
            debug.index_seeks,
            debug.sent_packets,
            debug.discarded_packets,
            debug.decoded_frames,
        );
        Ok(())
    }
}

impl VideoDecoder {
    /// Nearest bracketing frame, earlier on ties; clamp to first/last frame.
    /// Empty video is an error. Retain lookahead so the next pull follows the
    /// selected frame. Decode dependencies; convert only the selected result.
    fn seek_inner(&mut self, position: Duration) -> Result<(VideoFrame, SeekDebug)> {
        // Even an out-of-range request clamps to the final available frame.
        let target = i64::try_from(position.as_micros()).unwrap_or(i64::MAX);
        let mut scan = SeekDebug {
            target,
            nearest_below: None,
            index_seeks: 0,
            sent_packets: 0,
            discarded_packets: 0,
            decoded_frames: 0,
        };

        // Forward fast path: when the target is only a short distance past the
        // pending frame, continuing the current decode is cheaper than an
        // indexed seek that re-decodes the group of pictures from its keyframe.
        let mut earlier = match self.lookahead.pop_front() {
            Some(pending)
                if pending.timestamp.0 <= target
                    && target - pending.timestamp.0 <= FORWARD_SCAN_LIMIT_MICROSECONDS =>
            {
                pending
            }
            _ => self.indexed_seek(&mut scan)?,
        };
        if earlier.timestamp.0 >= target {
            return Ok((earlier, scan));
        }

        // Consume any remaining lookahead before decoding fresh frames.
        loop {
            let next = match self.lookahead.pop_front() {
                Some(frame) => Some(frame),
                None => self.decode_next(Some(&mut scan))?,
            };
            let Some(later) = next else {
                return Ok((earlier, scan));
            };
            if later.timestamp < earlier.timestamp {
                bail!("video presentation timestamps moved backwards while seeking");
            }
            if later.timestamp.0 < target {
                earlier = later;
                continue;
            }
            let before = i128::from(target) - i128::from(earlier.timestamp.0);
            let after = i128::from(later.timestamp.0) - i128::from(target);
            if before <= after {
                self.lookahead.push_back(later);
                return Ok((earlier, scan));
            }
            return Ok((later, scan));
        }
    }

    /// Land on a decodable frame at or before the scan target through the demuxer
    /// index, clamping to the first frame when the target precedes it.
    fn indexed_seek(&mut self, scan: &mut SeekDebug) -> Result<VideoFrame> {
        let target = scan.target;
        let mut seek_position = if self.input.duration() >= 0 {
            target.min(self.input.duration())
        } else {
            target
        };

        // The index can land on a sync sample the decoder cannot start from: a
        // container may list a non-IDR I-frame as a keyframe, which FFmpeg's
        // parser leaves unflagged, and reordering can put a real keyframe after
        // the target. Reject unflagged landings before decoding, and retry just
        // before the landing packet so each attempt steps back one index keyframe.
        let mut retry_step = FALLBACK_SEEK_STEP_MICROSECONDS;
        loop {
            scan.index_seeks += 1;
            let absolute = self.origin_microseconds.saturating_add(seek_position);
            self.input
                .seek(absolute, ..absolute)
                .context("seeking video demuxer")?;
            self.flush_codec()?;
            self.drain = DrainState::Reading;
            self.lookahead.clear();
            scan.nearest_below = None; // 冲刷后此前送入的包全部作废。
            let Some(landing) = self.read_video_packet()? else {
                if seek_position <= 0 {
                    bail!("video stream contains no decoded frames");
                }
                seek_position = seek_position.saturating_sub(retry_step).max(0);
                retry_step = retry_step.saturating_mul(2);
                continue;
            };
            if landing.is_key() || seek_position <= 0 {
                // The decoder was just flushed, so this send cannot return EAGAIN.
                self.send_video_packet(&landing, Some(&mut *scan))?;
                match self.decode_next(Some(&mut *scan))? {
                    Some(frame) if frame.timestamp.0 <= target || seek_position <= 0 => {
                        return Ok(frame);
                    }
                    None if seek_position <= 0 => {
                        bail!("video stream contains no decoded frames");
                    }
                    // The group of pictures starts after the target or decodes nothing.
                    Some(_) | None => {}
                }
            }
            // The step guarantees progress where the landing gives no earlier anchor.
            let step_position = seek_position.saturating_sub(retry_step);
            let retry_position = match landing.dts().or(landing.pts()) {
                Some(timestamp) => {
                    let landing_position = timestamp_microseconds(timestamp, self.time_base)?
                        .checked_sub(self.origin_microseconds)
                        .context("normalized video timestamp exceeds range")?;
                    step_position.min(landing_position.saturating_sub(1)) // 落点包之前 1 µs：索引回退到上一个关键帧。
                }
                None => step_position,
            };
            seek_position = retry_position.max(0);
            retry_step = retry_step.saturating_mul(2);
        }
    }
}

/// First step back from a failed landing, doubled after each attempt. A retry
/// goes further back when the landing packet itself lies earlier.
const FALLBACK_SEEK_STEP_MICROSECONDS: i64 = 50_000;

/// Furthest a target may lie past the pending frame to scan forward instead
/// of seeking. A few frames of decoding beats re-decoding a whole group of
/// pictures, but a long scan would be slower than the index for intra-only
/// or short-GOP content.
const FORWARD_SCAN_LIMIT_MICROSECONDS: i64 = 250_000;

/// Packets sent while one seek decodes toward its target.
struct SeekDebug {
    target: i64,                // 目标时间（微秒，相对 origin）。
    nearest_below: Option<i64>, // 已完整解码、早于目标的最大 PTS；更早的非参考帧不会被选中。
    index_seeks: u32,           // 临时测量：索引 seek 次数（含回退重试）。
    sent_packets: u32,          // 临时测量：送入解码器的包数。
    discarded_packets: u32,     // 临时测量：以非参考帧方式跳过的包数。
    decoded_frames: u32,        // 临时测量：解码输出的帧数。
}

enum Codec {
    Ffmpeg(decoder::Video),
    #[cfg(target_os = "macos")]
    VideoToolbox {
        decoder: VideoToolboxDecoder,
        colors: StreamColors, // 流参数里的色彩标签；直接解码的帧不带 FFmpeg 帧属性。
    },
}

enum Receive {
    Frame(VideoFrame),
    NeedInput,
    Eof,
}

#[cfg(target_os = "macos")]
struct StreamColors {
    range: color::Range,
    space: color::Space,
    primaries: color::Primaries,
    transfer: color::TransferCharacteristic,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DrainState {
    Reading,
    Draining,
    Drained,
}

impl VideoDecoder {
    /// A seek scan lets packets that cannot affect the selected frame skip
    /// non-reference decoding; None decodes every packet.
    fn decode_next(&mut self, mut scan: Option<&mut SeekDebug>) -> Result<Option<VideoFrame>> {
        if self.drain == DrainState::Drained {
            return Ok(None);
        }
        loop {
            match self.receive_frame()? {
                Receive::Frame(frame) => {
                    if let Some(scan) = scan.as_deref_mut() {
                        scan.decoded_frames += 1;
                    }
                    return Ok(Some(frame));
                }
                Receive::Eof => {
                    self.drain = DrainState::Drained;
                    return Ok(None);
                }
                Receive::NeedInput => {
                    if self.drain == DrainState::Draining {
                        bail!("video decoder requested input after draining began");
                    }
                }
            }

            // Receive-first guarantees send will not return EAGAIN.
            match self.read_video_packet()? {
                Some(packet) => self.send_video_packet(&packet, scan.as_deref_mut())?,
                None => {
                    match &mut self.codec {
                        Codec::Ffmpeg(decoder) => {
                            decoder.send_eof().context("starting video decoder drain")?
                        }
                        #[cfg(target_os = "macos")]
                        Codec::VideoToolbox { decoder, .. } => decoder.send_eof(),
                    }
                    self.drain = DrainState::Draining;
                }
            }
        }
    }

    /// One decoded frame in presentation order, or what the codec needs next.
    fn receive_frame(&mut self) -> Result<Receive> {
        match &mut self.codec {
            Codec::Ffmpeg(decoder) => {
                let mut native = Video::empty();
                match decoder.receive_frame(&mut native) {
                    Ok(()) => {
                        if DECODE_MODE == DecodeMode::VideoToolbox
                            && native.format() != Pixel::VIDEOTOOLBOX
                        {
                            bail!(
                                "VideoToolbox produced unexpected frame format {:?}",
                                native.format()
                            );
                        }
                        Ok(Receive::Frame(describe_frame(
                            native,
                            self.time_base,
                            self.origin_microseconds,
                            self.stream_rotation,
                        )?))
                    }
                    Err(FfmpegError::Eof) => Ok(Receive::Eof),
                    Err(FfmpegError::Other { errno: EAGAIN }) => Ok(Receive::NeedInput),
                    Err(error) => Err(error).context("receiving a decoded video frame"),
                }
            }
            #[cfg(target_os = "macos")]
            Codec::VideoToolbox { decoder, colors } => match decoder.receive()? {
                Received::Picture(picture) => Ok(Receive::Frame(describe_picture(
                    picture,
                    colors,
                    self.time_base,
                    self.origin_microseconds,
                    self.stream_rotation,
                )?)),
                Received::NeedInput => Ok(Receive::NeedInput),
                Received::Eof => Ok(Receive::Eof),
            },
        }
    }

    /// Drops pending frames before decoding from a new position.
    fn flush_codec(&mut self) -> Result<()> {
        match &mut self.codec {
            Codec::Ffmpeg(decoder) => decoder.flush(),
            #[cfg(target_os = "macos")]
            Codec::VideoToolbox { decoder, .. } => decoder.flush()?,
        }
        Ok(())
    }

    /// Next packet of the selected stream; None is demuxer EOF.
    fn read_video_packet(&mut self) -> Result<Option<Packet>> {
        // Read directly: FFmpeg's packet iterator discards demux errors, which is
        // unsuitable for an API where None must mean drained EOF rather than failure.
        let mut packet = Packet::empty();
        loop {
            match packet.read(&mut self.input) {
                Ok(()) => {
                    if packet.stream() == self.stream_index {
                        return Ok(Some(packet));
                    }
                    packet = Packet::empty();
                }
                Err(FfmpegError::Eof) => return Ok(None),
                Err(error) => return Err(error).context("reading a video packet"),
            }
        }
    }

    /// During a seek, a non-reference picture is skipped once a fully decoded
    /// picture lies between it and the target: nothing depends on it, and it
    /// cannot be the selected frame. Outside a seek, every picture decodes.
    fn send_video_packet(&mut self, packet: &Packet, scan: Option<&mut SeekDebug>) -> Result<()> {
        let discard = match scan {
            Some(scan) => {
                scan.sent_packets += 1;
                let discard = match packet.pts() {
                    Some(timestamp) => {
                        let position = timestamp_microseconds(timestamp, self.time_base)?
                            .checked_sub(self.origin_microseconds)
                            .context("normalized video timestamp exceeds range")?;
                        if position >= scan.target {
                            Discard::Default // 目标及之后的帧可能被选中。
                        } else {
                            match scan.nearest_below {
                                Some(nearest) if position < nearest => Discard::NonReference,
                                Some(_) | None => {
                                    scan.nearest_below = Some(position);
                                    Discard::Default
                                }
                            }
                        }
                    }
                    None => Discard::Default, // 无 PTS：无法判断是否会被选中。
                };
                if discard == Discard::NonReference {
                    scan.discarded_packets += 1;
                }
                discard
            }
            None => Discard::Default,
        };
        match &mut self.codec {
            Codec::Ffmpeg(decoder) => {
                // Set per packet: the decoder applies the level current at send time.
                decoder.skip_frame(discard);
                decoder
                    .send_packet(packet)
                    .context("sending a video packet")
            }
            // VideoToolbox still decodes a skipped picture as a reference; it only
            // suppresses the output, which is what keeps it from being selected.
            #[cfg(target_os = "macos")]
            Codec::VideoToolbox { decoder, .. } => {
                decoder.send_packet(packet, discard != Discard::NonReference)
            }
        }
    }
}

fn open_decoder(input: Input, stream_index: usize) -> Result<(Input, Codec, Rational, f64)> {
    let Some(stream) = input.stream(stream_index) else {
        bail!("video stream {stream_index} is no longer present");
    };
    if stream.parameters().medium() != MediaType::Video {
        bail!("stream {stream_index} is not video");
    }
    let time_base = stream.time_base();
    timestamp_microseconds(0, time_base)?;
    let mut rotation = 0.0;
    if let Some(value) = stream.metadata().get("rotate") {
        rotation = value
            .parse::<f64>()
            .context("reading video rotation metadata")?;
        if !rotation.is_finite() {
            bail!("non-finite video rotation metadata");
        }
    }
    for side_data in stream.side_data() {
        if side_data.kind() == PacketSideData::DisplayMatrix {
            rotation = display_rotation(side_data.data())?;
            break;
        }
    }
    // H.264 and HEVC in MP4-style framing decode through a pipelined session;
    // anything else keeps FFmpeg's VideoToolbox hwaccel.
    #[cfg(target_os = "macos")]
    {
        // SAFETY: the parameters belong to the open stream and outlive this block.
        let parameters = unsafe { stream.parameters().as_ptr() };
        if let Some(decoder) = VideoToolboxDecoder::open(parameters, time_base)? {
            // SAFETY: as above.
            let colors = unsafe {
                let parameters = &*parameters;
                StreamColors {
                    range: color::Range::from(parameters.color_range),
                    space: color::Space::from(parameters.color_space),
                    primaries: color::Primaries::from(parameters.color_primaries),
                    transfer: color::TransferCharacteristic::from(parameters.color_trc),
                }
            };
            return Ok((
                input,
                Codec::VideoToolbox { decoder, colors },
                time_base,
                rotation,
            ));
        }
    }
    let mut context = codec::context::Context::from_parameters(stream.parameters())
        .context("copying video codec parameters")?;
    // Frame threads only help software decoding. VideoToolbox serializes its
    // frames anyway, and the thread pipeline would add a refill delay of one
    // frame per thread after every seek flush.
    #[rustfmt::skip]
    let threading = match DECODE_MODE {
        DecodeMode::Software => {
            codec::threading::Config::kind(codec::threading::Type::Frame)
        }
        DecodeMode::VideoToolbox => {
            hardware::use_videotoolbox(&mut context)?;
            codec::threading::Config {
                kind: codec::threading::Type::None,
                count: 1,
            }
        }
    };
    context.set_threading(threading);
    let mut decoder = context.decoder();
    decoder.set_packet_time_base(time_base);
    let decoder = decoder.video().context("opening video decoder")?;
    Ok((input, Codec::Ffmpeg(decoder), time_base, rotation))
}

fn describe_frame(
    native: Video,
    time_base: Rational,
    origin: i64,
    stream_rotation: f64,
) -> Result<VideoFrame> {
    let Some(timestamp) = native.timestamp().or(native.pts()) else {
        bail!("decoded video frame has no presentation timestamp");
    };
    let timestamp = timestamp_microseconds(timestamp, time_base)?
        .checked_sub(origin)
        .context("normalized video timestamp exceeds range")?;
    let duration_ticks = native.packet().duration;
    let duration = if duration_ticks > 0 {
        let micros = timestamp_microseconds(duration_ticks, time_base)?;
        Some(Duration::from_micros(micros as u64))
    } else {
        None
    };
    let rotation_degrees = match native.side_data(FrameSideData::DisplayMatrix) {
        Some(data) => display_rotation(data.data())?,
        None => stream_rotation,
    };
    #[cfg(target_os = "macos")]
    let pixel_buffer = {
        // SAFETY: a VideoToolbox frame keeps its CVPixelBuffer in data[3]. Retain it
        // with the get rule so the picture outlives the FFmpeg frame.
        let source = unsafe { (*native.as_ptr()).data[3] as CVPixelBufferRef };
        if source.is_null() {
            bail!("VideoToolbox frame has no pixel buffer");
        }
        unsafe { CVPixelBuffer::wrap_under_get_rule(source) }
    };
    Ok(VideoFrame {
        timestamp: MediaTime(timestamp),
        duration,
        color_range: native.color_range(),
        color_space: native.color_space(),
        color_primaries: native.color_primaries(),
        color_transfer: native.color_transfer_characteristic(),
        rotation_degrees,
        #[cfg(target_os = "macos")]
        pixel_buffer,
        #[cfg(not(target_os = "macos"))]
        native,
    })
}

/// A picture from the pipelined session. Color tags come from the stream; the
/// rotation is the stream's, since VideoToolbox carries no per-frame side data.
#[cfg(target_os = "macos")]
fn describe_picture(
    picture: DecodedPicture,
    colors: &StreamColors,
    time_base: Rational,
    origin: i64,
    stream_rotation: f64,
) -> Result<VideoFrame> {
    let timestamp = timestamp_microseconds(picture.pts, time_base)?
        .checked_sub(origin)
        .context("normalized video timestamp exceeds range")?;
    let duration = if picture.duration > 0 {
        let micros = timestamp_microseconds(picture.duration, time_base)?;
        Some(Duration::from_micros(micros as u64))
    } else {
        None
    };
    Ok(VideoFrame {
        pixel_buffer: picture.pixel_buffer,
        timestamp: MediaTime(timestamp),
        duration,
        color_range: colors.range,
        color_space: colors.space,
        color_primaries: colors.primaries,
        color_transfer: colors.transfer,
        rotation_degrees: stream_rotation,
    })
}

fn display_rotation(data: &[u8]) -> Result<f64> {
    if data.len() < 36 {
        bail!("video display matrix is truncated");
    }
    // Copy to aligned storage instead of casting an arbitrary byte slice.
    let mut matrix = [0_i32; 9];
    for (index, entry) in matrix.iter_mut().enumerate() {
        let mut bytes = [0_u8; 4];
        bytes.copy_from_slice(&data[index * 4..index * 4 + 4]);
        *entry = i32::from_ne_bytes(bytes);
    }
    // SAFETY: matrix contains exactly nine aligned i32 values, alive for this call.
    let rotation = unsafe { av_display_rotation_get(matrix.as_ptr()) };
    if !rotation.is_finite() {
        bail!("video display matrix has no valid rotation");
    }
    Ok(rotation)
}
