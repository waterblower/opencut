//! Synchronous native decoding. Callers own execution, scheduling, and presentation.
//!
//! Returns native frames and PCM; the application prepares images for rendering.

mod audio;
mod hardware;
mod video;

pub use audio::{AudioDecoder, AudioSamples, PcmFormat};
pub use video::{DecodeDiagnostics, DecodeMode, VideoDecoder, VideoFrame};

use anyhow::{Context, Error, Result, bail};
use ffmpeg_next::{Rational, ffi::AV_NOPTS_VALUE, format, media::Type};
use std::{path::Path, time::Duration};

/// Signed normalized microseconds: preserve stream offsets and preroll.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MediaTime(pub i64);

/// Immutable description shared by independently opened decoder lanes.
#[derive(Clone, Debug)]
pub struct MediaInfo {
    /// Common origin in FFmpeg's microsecond time base, before normalization.
    pub origin_microseconds: i64,
    pub duration: Option<Duration>,
    pub video: VideoInfo,
    pub audio: Option<AudioInfo>,
}

#[derive(Clone, Debug)]
pub struct VideoInfo {
    pub stream_index: usize,
    pub width: u32,
    pub height: u32,
    pub average_frame_interval: Option<Duration>,
}

#[derive(Clone, Debug)]
pub struct AudioInfo {
    pub stream_index: usize,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Convenience owner for synchronous callers. Each decoder has its own demuxer.
/// The blocking player owns this directly and opens only the video decoder.
pub struct VideoBackend {
    pub metadata: MediaInfo,
    pub video: VideoDecoder,
    pub audio: Option<AudioDecoder>,
}

impl VideoBackend {
    pub fn open(path: &Path) -> Result<Self> {
        let mut backend = Self::open_video(path)?;
        if backend.metadata.audio.is_some() {
            backend.audio = Some(AudioDecoder::open(path, &backend.metadata)?);
        }
        Ok(backend)
    }

    /// Open video only, without initializing an audio decoder.
    pub fn open_video(path: &Path) -> Result<Self> {
        let metadata = Self::probe(path)?;
        let video = VideoDecoder::open(
            path,
            metadata.video.stream_index,
            metadata.origin_microseconds,
        )?;
        Ok(Self {
            metadata,
            video,
            audio: None,
        })
    }

    /// Requires a video stream. Probe resources are dropped before returning.
    pub fn probe(path: &Path) -> Result<MediaInfo> {
        ffmpeg_next::init().context("initializing FFmpeg")?;
        let input = format::input(path).context("opening media for metadata")?;
        let Some(video) = input.streams().best(Type::Video) else {
            bail!("media has no video stream");
        };
        let audio = input.streams().best(Type::Audio);

        // SAFETY: input owns the format context for the duration of this read.
        let mut origin = unsafe { (*input.as_ptr()).start_time };
        if origin == AV_NOPTS_VALUE {
            for stream in [Some(&video), audio.as_ref()].into_iter().flatten() {
                if stream.start_time() == AV_NOPTS_VALUE {
                    continue;
                }
                let start = timestamp_microseconds(stream.start_time(), stream.time_base())?;
                if origin == AV_NOPTS_VALUE || start < origin {
                    origin = start;
                }
            }
            if origin == AV_NOPTS_VALUE {
                origin = 0;
            }
        }

        let duration = if input.duration() >= 0 {
            Some(Duration::from_micros(input.duration() as u64))
        } else {
            None
        };
        let rate = video.avg_frame_rate();
        let average_frame_interval = if rate.numerator() > 0 && rate.denominator() > 0 {
            Some(Duration::from_secs_f64(
                f64::from(rate.denominator()) / f64::from(rate.numerator()),
            ))
        } else {
            None
        };
        let video_parameters = video.parameters();
        // SAFETY: the parameters and input remain alive; only scalar fields escape.
        let (width, height) = unsafe {
            let parameters = &*video_parameters.as_ptr();
            (parameters.width, parameters.height)
        };
        let video = VideoInfo {
            stream_index: video.index(),
            width: u32::try_from(width).context("invalid video width")?,
            height: u32::try_from(height).context("invalid video height")?,
            average_frame_interval,
        };
        let audio = (|| {
            let Some(stream) = audio else {
                return Ok(None);
            };
            let parameters = stream.parameters();
            // SAFETY: stream parameters are borrowed only while input is alive.
            let (sample_rate, channels) = unsafe {
                let parameters = &*parameters.as_ptr();
                (parameters.sample_rate, parameters.ch_layout.nb_channels)
            };
            Ok::<_, Error>(Some(AudioInfo {
                stream_index: stream.index(),
                sample_rate: u32::try_from(sample_rate).context("invalid audio sample rate")?,
                channels: u16::try_from(channels).context("invalid audio channel count")?,
            }))
        })()?;
        Ok(MediaInfo {
            origin_microseconds: origin,
            duration,
            video,
            audio,
        })
    }
}

// Round to the closest microsecond, with ties away from zero. i128 arithmetic
// avoids intermediate overflow and keeps large/nonzero stream origins exact.
fn timestamp_microseconds(value: i64, time_base: Rational) -> Result<i64> {
    if time_base.numerator() <= 0 || time_base.denominator() <= 0 {
        bail!("invalid stream time base: {time_base:?}");
    }
    let numerator = i128::from(value) * i128::from(time_base.numerator()) * 1_000_000;
    let denominator = i128::from(time_base.denominator());
    let rounded = if numerator < 0 {
        (numerator - denominator / 2) / denominator
    } else {
        (numerator + denominator / 2) / denominator
    };
    i64::try_from(rounded).context("timestamp exceeds microsecond range")
}
