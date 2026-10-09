use crate::time::timestamp_microseconds;
use anyhow::{Context, Error, Result, bail};
use ffmpeg_next::{ffi::AV_NOPTS_VALUE, format::context::Input, media::Type};
use std::{path::Path, time::Duration};

/// Immutable description shared by independently opened decoder lanes.
#[derive(Clone, Debug)]
pub struct MediaInfo {
    /// Common origin in FFmpeg's microsecond time base, before normalization.
    pub origin_microseconds: i64,
    pub duration: Duration,
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

impl MediaInfo {
    /// Reads metadata from an open demuxer without consuming it, so the same input can then
    /// feed a decoder. Requires a video stream and known duration. `path` only labels errors.
    pub fn from_av_input(input: &Input, path: &Path) -> Result<Self> {
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

        let duration_microseconds = input.duration();
        let duration = if duration_microseconds > 0 {
            Duration::from_micros(duration_microseconds as u64)
        } else {
            bail!("media duration is unavailable: {}", path.display());
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
        Ok(Self {
            origin_microseconds: origin,
            duration,
            video,
            audio,
        })
    }
}
