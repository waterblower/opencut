use super::{model::deserialize_ulid, timeline::TimelineTime};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

/// Static visual adjustments for one timeline clip.
///
/// Position is an offset in timeline pixels from the clip's centered placement. Scale is a
/// normalized multiplier, so `1.0` means 100%.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub(super) struct VideoClipProperties {
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
/// `0 dB` is unity gain and pan ranges from `-1.0` (left) to `1.0` (right).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub(super) struct AudioClipProperties {
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Clip {
    #[serde(deserialize_with = "deserialize_ulid")]
    pub id: Ulid,
    #[serde(deserialize_with = "deserialize_ulid")]
    pub track_id: Ulid,
    #[serde(default = "Ulid::nil", deserialize_with = "deserialize_ulid")]
    pub asset_id: Ulid,
    pub timeline_start: TimelineTime,
    pub source_in: TimelineTime,
    pub source_out: TimelineTime,
    #[serde(default)]
    pub video_properties: VideoClipProperties,
    #[serde(default)]
    pub audio_properties: AudioClipProperties,
}

impl Clip {
    pub fn duration(&self) -> TimelineTime {
        (self.source_out - self.source_in).max(TimelineTime::ZERO)
    }

    pub fn timeline_end(&self) -> TimelineTime {
        self.timeline_start + self.duration()
    }

    pub fn source_time_at(&self, timeline_position: TimelineTime) -> TimelineTime {
        let local =
            (timeline_position - self.timeline_start).clamp(TimelineTime::ZERO, self.duration());
        (self.source_in + local).min(self.source_out)
    }

    pub fn split_at(&self, timeline_position: TimelineTime) -> Option<(Self, Self)> {
        let local = timeline_position - self.timeline_start;
        if local < TimelineTime::ONE_FRAME || local > self.duration() - TimelineTime::ONE_FRAME {
            return None;
        }

        let source_split = self.source_time_at(timeline_position);
        let mut left = self.clone();
        left.source_out = source_split;
        let mut right = self.clone();
        right.id = Ulid::generate();
        right.timeline_start = timeline_position;
        right.source_in = source_split;
        Some((left, right))
    }
}
