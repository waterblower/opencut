use crate::video3::{AudioDecoder, AudioInfo, timestamp_microseconds};
use anyhow::{Context, Result, bail};
use ffmpeg_next::{ffi::AV_NOPTS_VALUE, format, media::Type};
use std::{path::Path, time::Duration};

/// Audio-only metadata; opening audio never requires a video stream.
pub struct AudioMediaInfo {
    pub origin_microseconds: i64,
    pub duration: Duration,
    pub audio: AudioInfo,
}

/// Synchronous decoding owner. The caller controls scheduling and output.
pub struct AudioBackend {
    pub metadata: AudioMediaInfo,
    pub audio: AudioDecoder,
}

impl AudioBackend {
    pub fn open(path: &Path) -> Result<Self> {
        let metadata = Self::probe(path)?;
        let audio = AudioDecoder::open_stream(
            path,
            metadata.audio.stream_index,
            metadata.origin_microseconds,
        )?;
        Ok(Self { metadata, audio })
    }

    pub fn probe(path: &Path) -> Result<AudioMediaInfo> {
        let input = format::input(path).context("opening audio metadata")?;
        let stream = input
            .streams()
            .best(Type::Audio)
            .context("media has no audio stream")?;
        // SAFETY: the input and stream own these parameters for this scope.
        let mut origin = unsafe { (*input.as_ptr()).start_time };
        if origin == AV_NOPTS_VALUE {
            origin = if stream.start_time() == AV_NOPTS_VALUE {
                0
            } else {
                timestamp_microseconds(stream.start_time(), stream.time_base())?
            };
        }
        let duration = if input.duration() > 0 {
            input.duration()
        } else if stream.duration() > 0 {
            timestamp_microseconds(stream.duration(), stream.time_base())?
        } else {
            bail!("audio duration is unavailable: {}", path.display());
        };
        let parameters = stream.parameters();
        let (rate, channels) = unsafe {
            (
                (*parameters.as_ptr()).sample_rate,
                (*parameters.as_ptr()).ch_layout.nb_channels,
            )
        };
        Ok(AudioMediaInfo {
            origin_microseconds: origin,
            duration: Duration::from_micros(duration as u64),
            audio: AudioInfo {
                stream_index: stream.index(),
                sample_rate: u32::try_from(rate).context("invalid audio sample rate")?,
                channels: u16::try_from(channels).context("invalid audio channel count")?,
            },
        })
    }
}
