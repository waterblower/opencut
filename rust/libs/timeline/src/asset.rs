use crate::FrameRate;
use crate::serialization::deserialize_ulid;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use ulid::Ulid;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
pub enum MediaKind {
    #[default]
    Video,
    Image,
    Audio,
}

impl MediaKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Video => "VIDEO",
            Self::Image => "IMAGE",
            Self::Audio => "AUDIO",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
#[serde(tag = "kind")]
pub enum Asset {
    Video(MediaAsset),
    Audio(MediaAsset),
    Image(ImageAsset),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
pub struct ImageAsset {
    #[serde(deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub id: Ulid,
    pub path: PathBuf,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub codec: String,
}

// Public APIs only
impl Asset {
    pub fn kind(&self) -> MediaKind {
        match self {
            Self::Video(_) => MediaKind::Video,
            Self::Audio(_) => MediaKind::Audio,
            Self::Image(_) => MediaKind::Image,
        }
    }
    pub fn duration(&self) -> Option<f64> {
        match self {
            Self::Video(a) | Self::Audio(a) => Some(a.duration),
            Self::Image(_) => None,
        }
    }
    pub fn has_audio(&self) -> bool {
        match self {
            Self::Video(a) | Self::Audio(a) => a.has_audio,
            Self::Image(_) => false,
        }
    }
    pub(crate) fn frame_rate(&self) -> Option<FrameRate> {
        match self {
            Self::Video(a) => a.frame_rate(),
            Self::Audio(_) | Self::Image(_) => None,
        }
    }
    pub fn id(&self) -> Ulid {
        match self {
            Self::Video(a) | Self::Audio(a) => a.id,
            Self::Image(a) => a.id,
        }
    }
    pub fn path(&self) -> &PathBuf {
        match self {
            Self::Video(a) | Self::Audio(a) => &a.path,
            Self::Image(a) => &a.path,
        }
    }
    pub fn name(&self) -> &String {
        match self {
            Self::Video(a) | Self::Audio(a) => &a.name,
            Self::Image(a) => &a.name,
        }
    }
    pub fn width(&self) -> u32 {
        match self {
            Self::Video(a) | Self::Audio(a) => a.width,
            Self::Image(a) => a.width,
        }
    }
    pub fn height(&self) -> u32 {
        match self {
            Self::Video(a) | Self::Audio(a) => a.height,
            Self::Image(a) => a.height,
        }
    }
    pub fn codec(&self) -> &String {
        match self {
            Self::Video(a) | Self::Audio(a) => &a.codec,
            Self::Image(a) => &a.codec,
        }
    }
    pub fn set_id(&mut self, value: Ulid) {
        match self {
            Self::Video(a) | Self::Audio(a) => a.id = value,
            Self::Image(a) => a.id = value,
        }
    }
    pub fn set_path(&mut self, value: PathBuf) {
        match self {
            Self::Video(a) | Self::Audio(a) => a.path = value,
            Self::Image(a) => a.path = value,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
pub struct MediaAsset {
    #[serde(deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub id: Ulid,
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
    pub(crate) fn frame_rate(&self) -> Option<FrameRate> {
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
