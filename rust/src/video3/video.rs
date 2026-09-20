use crate::video3::{MediaInfo, MediaTime};
use anyhow::Result;
use ffmpeg_next::{frame::Video, util::color};
use std::{marker::PhantomData, path::Path, rc::Rc, time::Duration};

pub struct VideoFrame {
    pub native: Video,
    pub timestamp: MediaTime,
    pub duration: Option<Duration>,
    pub color_range: color::Range,
    pub color_space: color::Space,
    pub color_primaries: color::Primaries,
    pub color_transfer: color::TransferCharacteristic,
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
    pub fallback_reason: Option<String>,
}

/// Open, use, and drop on one execution lane. No scheduler or current UI frame.
pub struct VideoDecoder {
    _lane_local: PhantomData<Rc<()>>,
}

impl VideoDecoder {
    pub fn open(_path: &Path, _metadata: &MediaInfo) -> Result<Self> {
        todo!("V2/V3: open an independent demuxer and native video decoder")
    }

    /// None is drained EOF; packet pumping and EAGAIN remain internal.
    pub fn next_frame(&mut self) -> Result<Option<VideoFrame>> {
        todo!("V2: pull reordered native frames and drain at EOF")
    }

    /// Nearest bracketing frame, earlier on ties; clamp to first/last frame.
    /// Empty video is an error. Retain lookahead so the next pull follows the
    /// selected frame. Decode dependencies; convert only the selected result.
    pub fn seek(&mut self, _position: Duration) -> Result<VideoFrame> {
        todo!("V2: seek with retained native lookahead")
    }

    pub fn diagnostics(&self) -> DecodeDiagnostics {
        todo!("V3: report actual frame format and hardware fallback")
    }
}
