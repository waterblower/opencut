//! Synchronous native decoding. Callers own execution, scheduling, and presentation.
//!
//! V1 establishes contracts only; opening/decoding/conversion are filled in V2–V5.

mod audio;
mod convert;
mod video;

pub use audio::{AudioDecoder, AudioSamples, PcmFormat};
pub use convert::{FrameConverter, RgbaImage};
pub use video::{DecodeDiagnostics, DecodeMode, VideoDecoder, VideoFrame};

use anyhow::Result;
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
/// The player instead probes and opens each decoder on its owning lane; it never
/// moves this aggregate across threads or shares it behind a mutex.
pub struct VideoBackend {
    pub metadata: MediaInfo,
    pub video: VideoDecoder,
    pub audio: Option<AudioDecoder>,
}

impl VideoBackend {
    pub fn open(path: &Path) -> Result<Self> {
        let metadata = Self::probe(path)?;
        let video = VideoDecoder::open(path, &metadata)?;
        let audio = if metadata.audio.is_some() {
            Some(AudioDecoder::open(path, &metadata)?)
        } else {
            None
        };
        Ok(Self {
            metadata,
            video,
            audio,
        })
    }

    /// Requires a video stream. Probe resources are dropped before returning.
    pub fn probe(_path: &Path) -> Result<MediaInfo> {
        todo!("V2: probe media and select a shared stream origin")
    }
}
