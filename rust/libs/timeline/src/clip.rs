use crate::serialization::deserialize_ulid;
use crate::{FrameRate, TimelineFrameIndex};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use ulid::Ulid;

/// Static visual adjustments for one timeline clip.
///
/// Position is an offset in timeline pixels from the clip's centered placement.
/// Scale multiplies the aspect-ratio-preserving fit to the canvas; `1.0` shows the full image.
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
    /// Text color as big-endian ARGB.
    pub color: u32,
    #[serde(flatten, with = "TextPosition")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "TextPosition"))]
    pub position: gpui::Point<f64>,
}

impl Default for TextClipProperties {
    fn default() -> Self {
        Self {
            text: "Text".to_string(),
            font: "Sans".to_string(),
            font_size: 64.0,
            color: 0xffffffff,
            position: gpui::point(0.5, 0.5),
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
pub struct VideoClip {
    #[serde(deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    id: Ulid,
    #[serde(alias = "layer_id", deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub track_id: Ulid,
    #[serde(default = "Ulid::nil", deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub asset_id: Ulid,
    pub timeline_start: TimelineFrameIndex,
    pub source_in: TimelineFrameIndex,
    pub source_out: TimelineFrameIndex,
    #[serde(default)]
    pub video_properties: VideoClipProperties,
    #[serde(default)]
    pub audio_properties: AudioClipProperties,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
pub struct AudioClip {
    #[serde(deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    id: Ulid,
    #[serde(alias = "layer_id", deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub track_id: Ulid,
    #[serde(default = "Ulid::nil", deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub asset_id: Ulid,
    pub timeline_start: TimelineFrameIndex,
    pub source_in: TimelineFrameIndex,
    pub source_out: TimelineFrameIndex,
    #[serde(default)]
    pub audio_properties: AudioClipProperties,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
pub struct TextClip {
    #[serde(deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    id: Ulid,
    #[serde(deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub track_id: Ulid,
    pub timeline_start: TimelineFrameIndex,
    #[serde(rename = "length")]
    pub duration: Duration,
    pub properties: TextClipProperties,
}

// Public APIs only
impl VideoClip {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Ulid,
        track_id: Ulid,
        asset_id: Ulid,
        timeline_start: TimelineFrameIndex,
        source_in: TimelineFrameIndex,
        source_out: TimelineFrameIndex,
        video_properties: VideoClipProperties,
        audio_properties: AudioClipProperties,
    ) -> Self {
        Self {
            id,
            track_id,
            asset_id,
            timeline_start,
            source_in,
            source_out,
            video_properties,
            audio_properties,
        }
    }

    pub fn id(&self) -> Ulid {
        self.id
    }

    /// Creates a new clip with the supplied identity; cloning preserves identity.
    pub fn copy(&self, new_id: Ulid) -> Self {
        Self {
            id: new_id,
            ..self.clone()
        }
    }
}

// Public APIs only
impl AudioClip {
    pub fn new(
        id: Ulid,
        track_id: Ulid,
        asset_id: Ulid,
        timeline_start: TimelineFrameIndex,
        source_in: TimelineFrameIndex,
        source_out: TimelineFrameIndex,
        audio_properties: AudioClipProperties,
    ) -> Self {
        Self {
            id,
            track_id,
            asset_id,
            timeline_start,
            source_in,
            source_out,
            audio_properties,
        }
    }

    pub fn id(&self) -> Ulid {
        self.id
    }

    /// Creates a new clip with the supplied identity; cloning preserves identity.
    pub fn copy(&self, new_id: Ulid) -> Self {
        Self {
            id: new_id,
            ..self.clone()
        }
    }
}

// Public APIs only
impl TextClip {
    pub fn new(
        id: Ulid,
        track_id: Ulid,
        timeline_start: TimelineFrameIndex,
        duration: Duration,
        properties: TextClipProperties,
    ) -> Self {
        Self {
            id,
            track_id,
            timeline_start,
            duration,
            properties,
        }
    }

    pub fn id(&self) -> Ulid {
        self.id
    }

    /// Creates a new clip with the supplied identity; cloning preserves identity.
    pub fn copy(&self, new_id: Ulid) -> Self {
        Self {
            id: new_id,
            ..self.clone()
        }
    }

    pub fn frame_length(&self, frame_rate: FrameRate) -> TimelineFrameIndex {
        frame_rate.frames_from_duration_nearest(self.duration)
    }
}

// Public APIs only
impl Clip {
    pub fn id(&self) -> Ulid {
        match self {
            Self::Video(clip) => clip.id(),
            Self::Audio(clip) => clip.id(),
            Self::Text(clip) => clip.id(),
        }
    }

    pub fn copy(&self, new_id: Ulid) -> Self {
        match self {
            Self::Video(clip) => Self::Video(clip.copy(new_id)),
            Self::Audio(clip) => Self::Audio(clip.copy(new_id)),
            Self::Text(clip) => Self::Text(clip.copy(new_id)),
        }
    }

    pub fn track_id(&self) -> Ulid {
        match self {
            Self::Video(clip) => clip.track_id,
            Self::Audio(clip) => clip.track_id,
            Self::Text(clip) => clip.track_id,
        }
    }

    pub fn set_track_id(&mut self, track_id: Ulid) {
        match self {
            Self::Video(clip) => clip.track_id = track_id,
            Self::Audio(clip) => clip.track_id = track_id,
            Self::Text(clip) => clip.track_id = track_id,
        }
    }

    pub fn timeline_start(&self) -> TimelineFrameIndex {
        match self {
            Self::Video(clip) => clip.timeline_start,
            Self::Audio(clip) => clip.timeline_start,
            Self::Text(clip) => clip.timeline_start,
        }
    }

    pub fn set_timeline_start(&mut self, timeline_start: TimelineFrameIndex) {
        match self {
            Self::Video(clip) => clip.timeline_start = timeline_start,
            Self::Audio(clip) => clip.timeline_start = timeline_start,
            Self::Text(clip) => clip.timeline_start = timeline_start,
        }
    }

    pub fn video(&self) -> Option<&VideoClip> {
        if let Self::Video(clip) = self {
            Some(clip)
        } else {
            None
        }
    }

    pub fn video_mut(&mut self) -> Option<&mut VideoClip> {
        if let Self::Video(clip) = self {
            Some(clip)
        } else {
            None
        }
    }

    pub fn audio(&self) -> Option<&AudioClip> {
        if let Self::Audio(clip) = self {
            Some(clip)
        } else {
            None
        }
    }

    pub fn audio_mut(&mut self) -> Option<&mut AudioClip> {
        if let Self::Audio(clip) = self {
            Some(clip)
        } else {
            None
        }
    }

    pub fn asset_id(&self) -> Option<Ulid> {
        match self {
            Self::Video(clip) => Some(clip.asset_id),
            Self::Audio(clip) => Some(clip.asset_id),
            Self::Text(_) => None,
        }
    }

    pub fn source_in(&self) -> Option<TimelineFrameIndex> {
        match self {
            Self::Video(clip) => Some(clip.source_in),
            Self::Audio(clip) => Some(clip.source_in),
            Self::Text(_) => None,
        }
    }

    pub fn source_out(&self) -> Option<TimelineFrameIndex> {
        match self {
            Self::Video(clip) => Some(clip.source_out),
            Self::Audio(clip) => Some(clip.source_out),
            Self::Text(_) => None,
        }
    }

    pub fn text(&self) -> Option<&TextClip> {
        let Self::Text(clip) = self else {
            return None;
        };
        Some(clip)
    }

    pub fn frame_length(&self, frame_rate: FrameRate) -> TimelineFrameIndex {
        match self {
            Self::Video(clip) => (clip.source_out - clip.source_in).max(TimelineFrameIndex::ZERO),
            Self::Audio(clip) => (clip.source_out - clip.source_in).max(TimelineFrameIndex::ZERO),
            Self::Text(clip) => clip.frame_length(frame_rate).max(TimelineFrameIndex::ZERO),
        }
    }

    pub fn timeline_end(&self, frame_rate: FrameRate) -> TimelineFrameIndex {
        self.timeline_start() + self.frame_length(frame_rate)
    }

    pub fn source_time_at(
        &self,
        timeline_position: TimelineFrameIndex,
    ) -> Option<TimelineFrameIndex> {
        let source_in = self.source_in()?;
        let source_out = self.source_out()?;
        let local = (timeline_position - self.timeline_start())
            .clamp(TimelineFrameIndex::ZERO, source_out - source_in);
        Some((source_in + local).min(source_out))
    }
}

// Maps GPUI coordinates to the existing flat JSON fields without duplicating clip data.
#[derive(Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
#[serde(remote = "gpui::Point<f64>")]
struct TextPosition {
    #[serde(rename = "position_x", default = "default_text_position")]
    x: f64,
    #[serde(rename = "position_y", default = "default_text_position")]
    y: f64,
}

fn default_text_position() -> f64 {
    0.5
}
