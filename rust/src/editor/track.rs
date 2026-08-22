use super::{model::deserialize_ulid, timeline::TimelineSerialization, timeline_clip::Clip};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(super) enum TrackKind {
    #[default]
    #[serde(alias = "video")]
    Video,
    #[serde(alias = "audio")]
    Audio,
    #[serde(alias = "text")]
    Text,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Track {
    #[serde(deserialize_with = "deserialize_ulid")]
    pub id: Ulid,
    pub name: String,
    pub kind: TrackKind,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub visible: bool,
}

impl TimelineSerialization {
    pub fn clip_locked(&self, clip_id: Ulid) -> bool {
        self.clip(clip_id)
            .and_then(|clip| self.track(clip.track_id))
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
            .filter(move |clip| clip.track_id == track_id)
    }
}
