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

pub struct VideoFrame {
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
    decoder: decoder::Video,
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
        let (input, decoder, time_base, stream_rotation) = open_decoder(input, stream_index)?;
        let mut decoder = Self {
            input,
            decoder,
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
        // Even an out-of-range request clamps to the final available frame.
        let mut scan = SeekScan {
            target: i64::try_from(position.as_micros()).unwrap_or(i64::MAX),
            nearest_below: None,
            index_seeks: 0,
            sent_packets: 0,
            discarded_packets: 0,
            decoded_frames: 0,
        };
        {
            let frame = self.seek_inner(&mut scan)?;
            self.lookahead.push_front(frame);
        }
        eprintln!(
            "[clip-switch] seek to {} µs: total={:?}, index_seeks={}, sent_packets={}, discarded_packets={}, decoded_frames={}",
            position.as_micros(),
            started.elapsed(),
            scan.index_seeks,
            scan.sent_packets,
            scan.discarded_packets,
            scan.decoded_frames,
        );
        Ok(())
    }
}

impl VideoDecoder {
    /// Nearest bracketing frame, earlier on ties; clamp to first/last frame.
    /// Empty video is an error. Retain lookahead so the next pull follows the
    /// selected frame. Decode dependencies; convert only the selected result.
    fn seek_inner(&mut self, scan: &mut SeekScan) -> Result<VideoFrame> {
        let target = scan.target;

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
            _ => self.indexed_seek(scan)?,
        };
        if earlier.timestamp.0 >= target {
            return Ok(earlier);
        }

        // Consume any remaining lookahead before decoding fresh frames.
        loop {
            let next = match self.lookahead.pop_front() {
                Some(frame) => Some(frame),
                None => self.decode_next(Some(&mut *scan))?,
            };
            let Some(later) = next else {
                return Ok(earlier);
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
                return Ok(earlier);
            }
            return Ok(later);
        }
    }

    /// Land on a decodable frame at or before the scan target through the demuxer
    /// index, clamping to the first frame when the target precedes it.
    fn indexed_seek(&mut self, scan: &mut SeekScan) -> Result<VideoFrame> {
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
            self.decoder.flush();
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
struct SeekScan {
    target: i64,                // 目标时间（微秒，相对 origin）。
    nearest_below: Option<i64>, // 已完整解码、早于目标的最大 PTS；更早的非参考帧不会被选中。
    index_seeks: u32,           // 临时测量：索引 seek 次数（含回退重试）。
    sent_packets: u32,          // 临时测量：送入解码器的包数。
    discarded_packets: u32,     // 临时测量：以非参考帧方式跳过的包数。
    decoded_frames: u32,        // 临时测量：解码输出的帧数。
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
    fn decode_next(&mut self, mut scan: Option<&mut SeekScan>) -> Result<Option<VideoFrame>> {
        if self.drain == DrainState::Drained {
            return Ok(None);
        }
        let mut native = Video::empty();
        loop {
            match self.decoder.receive_frame(&mut native) {
                Ok(()) => {
                    if let Some(scan) = scan.as_deref_mut() {
                        scan.decoded_frames += 1;
                    }
                    if DECODE_MODE == DecodeMode::VideoToolbox
                        && native.format() != Pixel::VIDEOTOOLBOX
                    {
                        bail!(
                            "VideoToolbox produced unexpected frame format {:?}",
                            native.format()
                        );
                    }
                    return Ok(Some(describe_frame(
                        native,
                        self.time_base,
                        self.origin_microseconds,
                        self.stream_rotation,
                    )?));
                }
                Err(FfmpegError::Eof) => {
                    self.drain = DrainState::Drained;
                    return Ok(None);
                }
                Err(FfmpegError::Other { errno: EAGAIN }) => {
                    if self.drain == DrainState::Draining {
                        bail!("video decoder requested input after draining began");
                    }
                }
                Err(error) => return Err(error).context("receiving a decoded video frame"),
            }

            // Receive-first guarantees send will not return EAGAIN.
            match self.read_video_packet()? {
                Some(packet) => self.send_video_packet(&packet, scan.as_deref_mut())?,
                None => {
                    self.decoder
                        .send_eof()
                        .context("starting video decoder drain")?;
                    self.drain = DrainState::Draining;
                }
            }
        }
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
    fn send_video_packet(&mut self, packet: &Packet, scan: Option<&mut SeekScan>) -> Result<()> {
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
        // Set per packet: the decoder applies the level current at send time.
        self.decoder.skip_frame(discard);
        self.decoder
            .send_packet(packet)
            .context("sending a video packet")
    }
}

fn open_decoder(
    input: Input,
    stream_index: usize,
) -> Result<(Input, decoder::Video, Rational, f64)> {
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
    Ok((input, decoder, time_base, rotation))
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
    Ok(VideoFrame {
        timestamp: MediaTime(timestamp),
        duration,
        color_range: native.color_range(),
        color_space: native.color_space(),
        color_primaries: native.color_primaries(),
        color_transfer: native.color_transfer_characteristic(),
        rotation_degrees,
        native,
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
