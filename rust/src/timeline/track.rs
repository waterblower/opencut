use super::deserialize_ulid;
use serde::{Deserialize, Serialize};
use ulid::Ulid;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
pub enum TrackKind {
    #[default]
    #[serde(alias = "video")]
    Video,
    #[serde(alias = "audio")]
    Audio,
    #[serde(alias = "text")]
    Text,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
pub struct Track {
    #[serde(deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
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
