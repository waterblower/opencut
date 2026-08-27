use super::timeline::FrameRate;
use serde::{Deserialize, Deserializer, Serialize, de};
use std::path::PathBuf;
use ulid::Ulid;

pub(super) const DEFAULT_IMAGE_CLIP_DURATION: f64 = 5.0;

pub(super) fn deserialize_ulid<'de, D>(deserializer: D) -> Result<Ulid, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum SerializedUlid {
        String(String),
        Legacy(u64),
    }

    match SerializedUlid::deserialize(deserializer)? {
        SerializedUlid::String(value) => value.parse().map_err(de::Error::custom),
        SerializedUlid::Legacy(value) => Ok(Ulid::from(u128::from(value))),
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub(super) enum MediaKind {
    #[default]
    #[serde(alias = "video")]
    Video,
    #[serde(alias = "image")]
    Image,
    #[serde(alias = "audio")]
    Audio,
}

impl MediaKind {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Video => "VIDEO",
            Self::Image => "IMAGE",
            Self::Audio => "AUDIO",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct MediaAsset {
    #[serde(deserialize_with = "deserialize_ulid")]
    pub id: Ulid,
    #[serde(default)]
    pub kind: MediaKind,
    pub path: PathBuf,
    pub name: String,
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub framerate: f64,
    #[serde(default)]
    pub frame_rate_numerator: u32,
    #[serde(default)]
    pub frame_rate_denominator: u32,
    pub codec: String,
    pub has_audio: bool,
}

impl MediaAsset {
    pub fn frame_rate(&self) -> Option<FrameRate> {
        if self.kind != MediaKind::Video {
            return None;
        }
        if self.frame_rate_numerator > 0 && self.frame_rate_denominator > 0 {
            return Some(FrameRate::new(
                self.frame_rate_numerator,
                self.frame_rate_denominator,
            ));
        }
        approximate_frame_rate(self.framerate)
    }
}

fn approximate_frame_rate(fps: f64) -> Option<FrameRate> {
    if !fps.is_finite() || fps <= 0.0 {
        return None;
    }
    for rate in [
        FrameRate::new(24_000, 1_001),
        FrameRate::new(30_000, 1_001),
        FrameRate::new(60_000, 1_001),
    ] {
        if (rate.frames_per_second() - fps).abs() < 0.01 {
            return Some(rate);
        }
    }
    Some(FrameRate::new(
        fps.round().clamp(1.0, u32::MAX as f64) as u32,
        1,
    ))
}
