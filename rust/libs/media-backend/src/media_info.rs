use std::time::Duration;

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
