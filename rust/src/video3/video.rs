use crate::video3::{MediaTime, hardware, timestamp_microseconds};
use anyhow::{Context, Result, bail};
use ffmpeg_next::{
    Error as FfmpegError, Packet, Rational, codec, decoder,
    ffi::av_display_rotation_get,
    format::{self, Pixel, context::Input},
    frame::{Video, side_data::Type as FrameSideData},
    media::Type as MediaType,
    packet::side_data::Type as PacketSideData,
    util::{color, error::EAGAIN},
};
use std::{
    collections::VecDeque,
    marker::PhantomData,
    path::Path,
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

#[derive(Clone, Debug)]
pub struct DecodeDiagnostics {
    /// Verified from received frames, not merely decoder creation.
    pub mode: DecodeMode,
    pub demux_time: Duration,
    pub decode_time: Duration,
}

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
    diagnostics: DecodeDiagnostics,
    _lane_local: PhantomData<Rc<()>>,
}

impl VideoDecoder {
    pub fn open(path: &Path, stream_index: usize, origin_microseconds: i64) -> Result<Self> {
        let mode = if cfg!(target_os = "macos") {
            DecodeMode::VideoToolbox
        } else {
            DecodeMode::Software
        };
        Self::open_mode(path, stream_index, origin_microseconds, mode)
    }

    /// None is drained EOF; packet pumping and EAGAIN remain internal.
    pub fn next_frame(&mut self) -> Result<Option<VideoFrame>> {
        if let Some(frame) = self.lookahead.pop_front() {
            return Ok(Some(frame));
        }
        self.decode_next()
    }

    /// Position the next pull at the nearest bracketing frame, earlier on ties.
    /// The selected frame and any decoded successor stay owned by the decoder.
    pub fn seek(&mut self, position: Duration) -> Result<()> {
        let started = Instant::now();
        let demux_before = self.diagnostics.demux_time;
        let decode_before = self.diagnostics.decode_time;
        {
            let frame = self.seek_inner(position)?;
            self.lookahead.push_front(frame);
        }
        eprintln!(
            "Seek to {} µs: seek={:?}, packet_read={:?}, codec_send_receive={:?}",
            position.as_micros(),
            started.elapsed(),
            self.diagnostics.demux_time - demux_before,
            self.diagnostics.decode_time - decode_before,
        );
        Ok(())
    }

    pub fn diagnostics(&self) -> DecodeDiagnostics {
        self.diagnostics.clone()
    }
}

impl VideoDecoder {
    fn open_mode(
        path: &Path,
        stream_index: usize,
        origin_microseconds: i64,
        mode: DecodeMode,
    ) -> Result<Self> {
        let (input, decoder, time_base, stream_rotation) = open_decoder(path, stream_index, mode)?;
        let mut decoder = Self {
            input,
            decoder,
            stream_index,
            time_base,
            origin_microseconds,
            stream_rotation,
            drain: DrainState::Reading,
            lookahead: VecDeque::new(),
            diagnostics: DecodeDiagnostics {
                mode,
                demux_time: Duration::ZERO,
                decode_time: Duration::ZERO,
            },
            _lane_local: PhantomData,
        };
        let Some(first) = decoder.decode_next()? else {
            bail!("video stream contains no decoded frames");
        };
        decoder.lookahead.push_back(first);
        Ok(decoder)
    }

    /// Nearest bracketing frame, earlier on ties; clamp to first/last frame.
    /// Empty video is an error. Retain lookahead so the next pull follows the
    /// selected frame. Decode dependencies; convert only the selected result.
    fn seek_inner(&mut self, position: Duration) -> Result<VideoFrame> {
        // Even an out-of-range request clamps to the final available frame.
        let target = i64::try_from(position.as_micros()).unwrap_or(i64::MAX);

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
            _ => self.indexed_seek(target)?,
        };
        if earlier.timestamp.0 >= target {
            return Ok(earlier);
        }

        // Consume any remaining lookahead before decoding fresh frames.
        loop {
            let Some(later) = self.next_frame()? else {
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

    /// Land on a decodable frame at or before the target through the demuxer
    /// index, clamping to the first frame when the target precedes it.
    fn indexed_seek(&mut self, target: i64) -> Result<VideoFrame> {
        let mut seek_position = if self.input.duration() >= 0 {
            target.min(self.input.duration())
        } else {
            target
        };

        // The indexed seek normally lands on the keyframe at or before the
        // request, but reordering or index granularity can land it after the
        // target or at EOF. Retry slightly earlier through the index instead of
        // reopening and rescanning from the start of the file.
        let mut retry_step = FALLBACK_SEEK_STEP_MICROSECONDS;
        loop {
            let absolute = self.origin_microseconds.saturating_add(seek_position);
            self.input
                .seek(absolute, ..absolute)
                .context("seeking video demuxer")?;
            self.decoder.flush();
            self.drain = DrainState::Reading;
            self.lookahead.clear();
            let landed = self.decode_next()?;
            let step = match landed {
                Some(frame) if frame.timestamp.0 <= target || seek_position <= 0 => {
                    return Ok(frame);
                }
                Some(frame) => frame_step_microseconds(&frame).unwrap_or(retry_step),
                None if seek_position <= 0 => bail!("video stream contains no decoded frames"),
                None => retry_step,
            };
            // Move the request itself earlier: the landing frame is already past
            // the target, so its timestamp is no better anchor than the request.
            seek_position = seek_position.saturating_sub(step).max(0);
            retry_step = retry_step.saturating_mul(2);
        }
    }
}

/// Step used when the landing frame has no usable duration.
const FALLBACK_SEEK_STEP_MICROSECONDS: i64 = 50_000;

/// Furthest a target may lie past the pending frame to scan forward instead
/// of seeking. A few frames of decoding beats re-decoding a whole group of
/// pictures, but a long scan would be slower than the index for intra-only
/// or short-GOP content.
const FORWARD_SCAN_LIMIT_MICROSECONDS: i64 = 250_000;

#[derive(Clone, Copy, PartialEq, Eq)]
enum DrainState {
    Reading,
    Draining,
    Drained,
}

impl VideoDecoder {
    fn decode_next(&mut self) -> Result<Option<VideoFrame>> {
        if self.drain == DrainState::Drained {
            return Ok(None);
        }
        let mut native = Video::empty();
        loop {
            let started = Instant::now();
            let received = self.decoder.receive_frame(&mut native);
            self.diagnostics.decode_time += started.elapsed();
            match received {
                Ok(()) => {
                    if self.diagnostics.mode == DecodeMode::VideoToolbox
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

            // Receive-first guarantees send will not return EAGAIN. Read directly:
            // FFmpeg's packet iterator discards demux errors, which is unsuitable
            // for an API where None must mean drained EOF rather than failure.
            let mut packet = Packet::empty();
            loop {
                let started = Instant::now();
                let read = packet.read(&mut self.input);
                self.diagnostics.demux_time += started.elapsed();
                match read {
                    Ok(()) => {
                        if packet.stream() != self.stream_index {
                            packet = Packet::empty();
                            continue;
                        }
                        let started = Instant::now();
                        let sent = self.decoder.send_packet(&packet);
                        self.diagnostics.decode_time += started.elapsed();
                        sent.context("sending a video packet")?;
                        break;
                    }
                    Err(FfmpegError::Eof) => {
                        self.decoder
                            .send_eof()
                            .context("starting video decoder drain")?;
                        self.drain = DrainState::Draining;
                        break;
                    }
                    Err(error) => return Err(error).context("reading a video packet"),
                }
            }
        }
    }
}

fn open_decoder(
    path: &Path,
    stream_index: usize,
    mode: DecodeMode,
) -> Result<(Input, decoder::Video, Rational, f64)> {
    ffmpeg_next::init().context("initializing FFmpeg")?;
    let input = format::input(path).context("opening video demuxer")?;
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
    let threading = match mode {
        DecodeMode::Software => codec::threading::Config::kind(codec::threading::Type::Frame),
        DecodeMode::VideoToolbox => {
            hardware::configure(&mut context)?;
            codec::threading::Config {
                kind: codec::threading::Type::None,
                count: 1,
            }
        }
    };
    context.set_threading(threading);
    let mut decoder = context.decoder();
    decoder.set_packet_time_base(time_base);
    let decoder = decoder.video().context("opening software video decoder")?;
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

/// Positive frame duration in microseconds, if the container reported one.
fn frame_step_microseconds(frame: &VideoFrame) -> Option<i64> {
    let duration = frame.duration?;
    let micros = i64::try_from(duration.as_micros()).ok()?;
    if micros > 0 { Some(micros) } else { None }
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
