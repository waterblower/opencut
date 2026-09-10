//! Shared timeline format and serialization rules; no file I/O or backend state.
mod serialization;
pub use serialization::{ParseError, parse};
mod asset;
mod clip;
mod time;
mod track;
pub use asset::*;
pub use clip::*;
use serde::{Deserialize, Serialize};
use std::time::Duration;
pub use time::*;
pub use track::*;
use ulid::Ulid;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct TimelineSerialization {
    pub settings: TimelineSettings,
    pub assets: Vec<MediaAsset>,
    #[serde(alias = "layers")]
    pub tracks: Vec<Track>,
    pub clips: Vec<Clip>,
    pub view: TimelineViewState,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct TimelineViewState {
    pub saved_playhead_frame: TimelineTime,
    pub horizontal_scroll: f32,
    pub vertical_scroll: f32,
    pub pixels_per_second: f32,
    pub snapping_enabled: bool,
    pub track_magnet_enabled: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
pub struct TimelineSettings {
    pub frame_rate: FrameRate,
    pub width: u32,
    pub height: u32,
    pub audio_sample_rate: u32,
}

impl Default for TimelineSettings {
    fn default() -> Self {
        Self {
            frame_rate: FrameRate::default(),
            width: 1920,
            height: 1080,
            audio_sample_rate: 48_000,
        }
    }
}

impl Default for TimelineViewState {
    fn default() -> Self {
        Self {
            saved_playhead_frame: TimelineTime::ZERO,
            horizontal_scroll: 0.0,
            vertical_scroll: 0.0,
            pixels_per_second: 72.0,
            snapping_enabled: true,
            track_magnet_enabled: true,
        }
    }
}

impl TimelineSerialization {
    pub fn asset(&self, id: Ulid) -> Option<&MediaAsset> {
        self.assets.iter().find(|asset| asset.id == id)
    }
    pub fn clip(&self, id: Ulid) -> Option<&Clip> {
        self.clips.iter().find(|clip| clip.id() == id)
    }
    pub fn clip_mut(&mut self, id: Ulid) -> Option<&mut Clip> {
        self.clips.iter_mut().find(|clip| clip.id() == id)
    }
    pub fn clip_index(&self, id: Ulid) -> Option<usize> {
        self.clips.iter().position(|clip| clip.id() == id)
    }
    pub fn content_duration(&self) -> TimelineTime {
        let frame_rate = self.settings.frame_rate;
        self.clips
            .iter()
            .map(|clip| clip.timeline_end(frame_rate))
            .max()
            .unwrap_or(TimelineTime::ZERO)
    }
    pub fn seconds(&self, time: TimelineTime) -> f64 {
        self.settings.frame_rate.seconds(time)
    }
    pub fn duration(&self, time: TimelineTime) -> Duration {
        self.settings.frame_rate.duration(time)
    }
    pub fn nearest_time(&self, seconds: f64) -> TimelineTime {
        self.settings.frame_rate.nearest(seconds)
    }
    pub fn audio_duration(&self, time: TimelineTime) -> Duration {
        let samples = self
            .settings
            .frame_rate
            .audio_samples(time, self.settings.audio_sample_rate);
        Duration::from_secs_f64(samples as f64 / self.settings.audio_sample_rate as f64)
    }
    pub fn source_frame_at(&self, clip: &Clip, timeline_position: TimelineTime) -> Option<i64> {
        let asset = self.asset(clip.media()?.asset_id)?;
        let source_rate = asset.frame_rate()?;
        let source_time = clip.source_time_at(timeline_position)?;
        Some(
            self.settings
                .frame_rate
                .rescale_floor(source_time, source_rate)
                .frames(),
        )
    }
    pub fn source_position_at(&self, clip: &Clip, timeline_position: TimelineTime) -> Duration {
        let Some(media) = clip.media() else {
            return Duration::ZERO;
        };
        let Some(asset) = self.asset(media.asset_id) else {
            return Duration::ZERO;
        };
        if let (Some(source_rate), Some(source_frame)) = (
            asset.frame_rate(),
            self.source_frame_at(clip, timeline_position),
        ) {
            return source_rate.duration(TimelineTime::from_frames(source_frame));
        }
        self.audio_duration(clip.source_time_at(timeline_position).unwrap_or_default())
    }
    pub fn source_start_seconds(&self, clip: &Clip) -> f64 {
        self.source_position_at(clip, clip.timeline_start())
            .as_secs_f64()
    }
    pub fn ceil_time(&self, seconds: f64) -> TimelineTime {
        self.settings.frame_rate.ceil(seconds)
    }
    pub fn clip_locked(&self, clip_id: Ulid) -> bool {
        self.clip(clip_id)
            .and_then(|clip| self.track(clip.track_id()))
            .is_some_and(|track| track.locked)
    }
    pub fn track(&self, id: Ulid) -> Option<&Track> {
        self.tracks.iter().find(|track| track.id == id)
    }
    pub fn track_mut(&mut self, id: Ulid) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|track| track.id == id)
    }
    pub fn clips_on_track(&self, track_id: Ulid) -> impl Iterator<Item = &Clip> {
        self.clips
            .iter()
            .filter(move |clip| clip.track_id() == track_id)
    }
}

fn deserialize_ulid<'de, D>(deserializer: D) -> Result<Ulid, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum SerializedUlid {
        String(String),
        Legacy(u64),
    }

    match SerializedUlid::deserialize(deserializer)? {
        SerializedUlid::String(value) => value.parse().map_err(serde::de::Error::custom),
        SerializedUlid::Legacy(value) => Ok(Ulid::from(u128::from(value))),
    }
}
