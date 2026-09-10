use super::{FrameRate, TimelineTime, deserialize_ulid};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use ulid::Ulid;

/// Static visual adjustments for one timeline clip.
///
/// Position is an offset in timeline pixels from the clip's centered placement. Scale is a
/// normalized multiplier, so `1.0` means 100%.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct VideoClipProperties {
    pub position_x: f64,
    pub position_y: f64,
    pub scale: f64,
}

impl Default for VideoClipProperties {
    fn default() -> Self {
        Self {
            position_x: 0.0,
            position_y: 0.0,
            scale: 1.0,
        }
    }
}

/// Static audio adjustments for one timeline clip.
///
/// `0 dB` is unity gain.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct AudioClipProperties {
    pub gain_db: f64,
    pub muted: bool,
}

impl Default for AudioClipProperties {
    fn default() -> Self {
        Self {
            gain_db: 0.0,
            muted: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct TextClipProperties {
    pub text: String,
    pub font: String,
    pub font_size: f64,
    /// Text color as big-endian ARGB, matching GStreamer.
    pub color: u32,
    pub position_x: f64,
    pub position_y: f64,
}

impl Default for TextClipProperties {
    fn default() -> Self {
        Self {
            text: "Text".to_string(),
            font: "Sans".to_string(),
            font_size: 64.0,
            color: 0xffffffff,
            position_x: 0.5,
            position_y: 0.5,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
// https://serde.rs/enum-representations.html#adjacently-tagged
#[serde(tag = "kind", content = "data")]
pub enum Clip {
    #[serde(alias = "Media")]
    Video(VideoClip),
    Audio(AudioClip),
    Text(TextClip),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
pub struct MediaClipData {
    #[serde(deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub id: Ulid,
    #[serde(alias = "layer_id", deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub track_id: Ulid,
    #[serde(default = "Ulid::nil", deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub asset_id: Ulid,
    pub timeline_start: TimelineTime,
    pub source_in: TimelineTime,
    pub source_out: TimelineTime,
    #[serde(default)]
    pub video_properties: VideoClipProperties,
    #[serde(default)]
    pub audio_properties: AudioClipProperties,
}

pub type VideoClip = MediaClipData;
pub type AudioClip = MediaClipData;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
pub struct TextClip {
    #[serde(deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub id: Ulid,
    #[serde(deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub track_id: Ulid,
    pub timeline_start: TimelineTime,
    pub length: Duration,
    pub properties: TextClipProperties,
}

impl TextClip {
    pub fn frame_length(&self, frame_rate: FrameRate) -> TimelineTime {
        frame_rate.frames_from_duration_nearest(self.length)
    }
}

impl Clip {
    pub fn id(&self) -> Ulid {
        match self {
            Self::Video(clip) | Self::Audio(clip) => clip.id,
            Self::Text(clip) => clip.id,
        }
    }

    pub fn set_id(&mut self, id: Ulid) {
        match self {
            Self::Video(clip) | Self::Audio(clip) => clip.id = id,
            Self::Text(clip) => clip.id = id,
        }
    }

    pub fn track_id(&self) -> Ulid {
        match self {
            Self::Video(clip) | Self::Audio(clip) => clip.track_id,
            Self::Text(clip) => clip.track_id,
        }
    }

    pub fn set_track_id(&mut self, track_id: Ulid) {
        match self {
            Self::Video(clip) | Self::Audio(clip) => clip.track_id = track_id,
            Self::Text(clip) => clip.track_id = track_id,
        }
    }

    pub fn timeline_start(&self) -> TimelineTime {
        match self {
            Self::Video(clip) | Self::Audio(clip) => clip.timeline_start,
            Self::Text(clip) => clip.timeline_start,
        }
    }

    pub fn set_timeline_start(&mut self, timeline_start: TimelineTime) {
        match self {
            Self::Video(clip) | Self::Audio(clip) => clip.timeline_start = timeline_start,
            Self::Text(clip) => clip.timeline_start = timeline_start,
        }
    }

    pub fn media(&self) -> Option<&MediaClipData> {
        match self {
            Self::Video(clip) | Self::Audio(clip) => Some(clip),
            Self::Text(_) => None,
        }
    }

    pub fn media_mut(&mut self) -> Option<&mut MediaClipData> {
        match self {
            Self::Video(clip) | Self::Audio(clip) => Some(clip),
            Self::Text(_) => None,
        }
    }

    pub fn text(&self) -> Option<&TextClip> {
        let Self::Text(clip) = self else {
            return None;
        };
        Some(clip)
    }

    pub fn frame_length(&self, frame_rate: FrameRate) -> TimelineTime {
        match self {
            Self::Video(clip) | Self::Audio(clip) => {
                (clip.source_out - clip.source_in).max(TimelineTime::ZERO)
            }
            Self::Text(clip) => clip.frame_length(frame_rate).max(TimelineTime::ZERO),
        }
    }

    pub fn timeline_end(&self, frame_rate: FrameRate) -> TimelineTime {
        self.timeline_start() + self.frame_length(frame_rate)
    }

    pub fn source_time_at(&self, timeline_position: TimelineTime) -> Option<TimelineTime> {
        let clip = self.media()?;
        let local = (timeline_position - clip.timeline_start)
            .clamp(TimelineTime::ZERO, clip.source_out - clip.source_in);
        Some((clip.source_in + local).min(clip.source_out))
    }
}
