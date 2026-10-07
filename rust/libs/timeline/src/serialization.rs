use crate as runtime;
use crate::TimelineEditingState as RuntimeTimelineEditingState;
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};
use ulid::Ulid;

/// Disk-only document. Runtime content crosses this boundary through explicit copies.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
pub struct TimelineSerialization {
    pub editing_state: TimelineEditingState,
    #[serde(default)]
    view_state: TimelineViewState,
}

impl TimelineSerialization {
    /// Frame count from frame zero to the latest clip end, including gaps.
    pub fn frame_count(&self) -> i64 {
        let mut frame_count = 0;
        for clip in &self.editing_state.clips {
            let end_frame = clip.end_frame(self.editing_state.settings.frame_rate);
            if end_frame > frame_count {
                frame_count = end_frame;
            }
        }
        frame_count
    }

    /// Captures editing content with default persisted view preferences.
    pub fn from_editing_state(editing_state: &RuntimeTimelineEditingState) -> Self {
        Self {
            editing_state: TimelineEditingState::from_runtime(editing_state),
            view_state: TimelineViewState::default(),
        }
    }

    /// Rebuilds independent runtime content for editing.
    pub fn to_editing_state(&self) -> RuntimeTimelineEditingState {
        self.editing_state.to_runtime()
    }

    pub fn set_view_state(
        &mut self,
        playhead: runtime::TimelineFrameIndex,
        scroll: (f32, f32),
        pixels_per_second: f32,
        snapping_enabled: bool,
        track_magnet_enabled: bool,
    ) {
        self.view_state = TimelineViewState {
            saved_playhead_frame: i64::from(playhead).max(0),
            horizontal_scroll: nonnegative_finite(scroll.0),
            vertical_scroll: nonnegative_finite(scroll.1),
            pixels_per_second: if pixels_per_second.is_finite() && pixels_per_second > 0.0 {
                pixels_per_second
            } else {
                72.0
            },
            snapping_enabled,
            track_magnet_enabled,
        };
    }

    pub fn playhead(&self) -> runtime::TimelineFrameIndex {
        self.view_state.saved_playhead_frame.max(0).into()
    }

    pub fn scroll_offset(&self) -> (f32, f32) {
        (
            nonnegative_finite(self.view_state.horizontal_scroll),
            nonnegative_finite(self.view_state.vertical_scroll),
        )
    }

    pub fn pixels_per_second(&self) -> f32 {
        let zoom = self.view_state.pixels_per_second;
        if zoom.is_finite() && zoom > 0.0 {
            zoom
        } else {
            72.0
        }
    }

    pub fn snapping_enabled(&self) -> bool {
        self.view_state.snapping_enabled
    }

    pub fn track_magnet_enabled(&self) -> bool {
        self.view_state.track_magnet_enabled
    }

    pub fn load(path: &Path) -> Result<Self> {
        let contents = fs::read(path).context(format!("Reading timeline {}", path.display()))?;
        let document: Self = serde_json::from_slice(&contents)
            .context(format!("Parsing timeline JSON {}", path.display()))?;
        document
            .to_editing_state()
            .validate()
            .context(format!("Validating timeline {}", path.display()))?;
        Ok(document)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let directory = path
            .parent()
            .context("Timeline path has no parent directory")?;
        fs::create_dir_all(directory).context(format!(
            "Creating timeline directory {}",
            directory.display()
        ))?;
        let mut bytes = serde_json::to_vec_pretty(self).context("Serializing timeline")?;
        bytes.push(b'\n');
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, bytes).context(format!(
            "Writing temporary timeline {}",
            temporary.display()
        ))?;
        fs::rename(&temporary, path).context(format!("Replacing timeline {}", path.display()))?;
        Ok(())
    }
}

fn nonnegative_finite(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

/// Persisted editing content, independent of runtime models.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct TimelineEditingState {
    pub settings: TimelineSettings,
    pub assets: Vec<MediaAsset>,
    #[serde(alias = "layers")]
    pub tracks: Vec<Track>,
    pub clips: Vec<Clip>,
}

/// Only the view preferences selected for persistence.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
#[serde(default)]
struct TimelineViewState {
    saved_playhead_frame: i64,
    horizontal_scroll: f32,
    vertical_scroll: f32,
    pixels_per_second: f32,
    snapping_enabled: bool,
    track_magnet_enabled: bool,
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
            saved_playhead_frame: 0,
            horizontal_scroll: 0.0,
            vertical_scroll: 0.0,
            pixels_per_second: 72.0,
            snapping_enabled: true,
            track_magnet_enabled: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
pub enum MediaKind {
    #[default]
    #[serde(alias = "video")]
    Video,
    #[serde(alias = "image")]
    Image,
    #[serde(alias = "audio")]
    Audio,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
pub struct MediaAsset {
    #[serde(deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub id: Ulid,
    #[serde(default)]
    pub kind: MediaKind,
    pub path: PathBuf,
    name: String,
    duration: f64,
    width: u32,
    height: u32,
    pub framerate: f64,
    #[serde(default)]
    pub frame_rate_numerator: u32,
    #[serde(default)]
    pub frame_rate_denominator: u32,
    codec: String,
    pub has_audio: bool,
}

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
    name: String,
    pub kind: TrackKind,
    #[serde(default)]
    locked: bool,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub visible: bool,
}
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

impl Clip {
    /// Exclusive end frame on the timeline, using its frame rate.
    pub fn end_frame(&self, fps: FrameRate) -> i64 {
        let (start, length) = match self {
            Self::Video(media) => (
                media.timeline_start,
                media.source_out.saturating_sub(media.source_in).max(0),
            ),
            Self::Audio(media) => (
                media.timeline_start,
                media.source_out.saturating_sub(media.source_in).max(0),
            ),
            Self::Text(text) => {
                let numerator = text.length.as_nanos() * u128::from(fps.numerator);
                let denominator = 1_000_000_000 * u128::from(fps.denominator);
                let length =
                    ((numerator + denominator / 2) / denominator).min(i64::MAX as u128) as i64;
                (text.timeline_start, length)
            }
        };
        start.saturating_add(length)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
pub struct VideoClip {
    #[serde(deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub id: Ulid,
    #[serde(alias = "layer_id", deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub track_id: Ulid,
    #[serde(default = "Ulid::nil", deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub asset_id: Ulid,
    pub timeline_start: i64,
    pub source_in: i64,
    pub source_out: i64,
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
    pub id: Ulid,
    #[serde(alias = "layer_id", deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub track_id: Ulid,
    #[serde(default = "Ulid::nil", deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub asset_id: Ulid,
    pub timeline_start: i64,
    pub source_in: i64,
    pub source_out: i64,
    #[serde(default)]
    pub audio_properties: AudioClipProperties,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
pub struct TextClip {
    #[serde(deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub id: Ulid,
    #[serde(deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    pub track_id: Ulid,
    pub timeline_start: i64,
    pub length: Duration,
    pub properties: TextClipProperties,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
pub struct FrameRate {
    pub numerator: u32,
    pub denominator: u32,
}

impl Default for FrameRate {
    fn default() -> Self {
        Self {
            numerator: 30,
            denominator: 1,
        }
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

impl TimelineEditingState {
    fn from_runtime(value: &RuntimeTimelineEditingState) -> Self {
        Self {
            settings: TimelineSettings::from_runtime(&value.settings),
            assets: value.assets.iter().map(MediaAsset::from_runtime).collect(),
            tracks: value.tracks.iter().map(Track::from_runtime).collect(),
            clips: value.clips.iter().map(Clip::from_runtime).collect(),
        }
    }

    fn to_runtime(&self) -> RuntimeTimelineEditingState {
        RuntimeTimelineEditingState {
            settings: self.settings.to_runtime(),
            assets: self.assets.iter().map(MediaAsset::to_runtime).collect(),
            tracks: self.tracks.iter().map(Track::to_runtime).collect(),
            clips: self.clips.iter().map(Clip::to_runtime).collect(),
        }
    }
}

impl TimelineSettings {
    fn from_runtime(value: &runtime::TimelineSettings) -> Self {
        Self {
            frame_rate: FrameRate::from_runtime(&value.frame_rate),
            width: value.width,
            height: value.height,
            audio_sample_rate: value.audio_sample_rate,
        }
    }

    fn to_runtime(&self) -> runtime::TimelineSettings {
        runtime::TimelineSettings {
            frame_rate: self.frame_rate.to_runtime(),
            width: self.width,
            height: self.height,
            audio_sample_rate: self.audio_sample_rate,
        }
    }
}

impl FrameRate {
    fn from_runtime(value: &runtime::FrameRate) -> Self {
        Self {
            numerator: value.numerator,
            denominator: value.denominator,
        }
    }

    fn to_runtime(&self) -> runtime::FrameRate {
        runtime::FrameRate {
            numerator: self.numerator,
            denominator: self.denominator,
        }
    }
}

impl MediaAsset {
    fn from_runtime(value: &runtime::MediaAsset) -> Self {
        Self {
            id: value.id,
            kind: MediaKind::from_runtime(&value.kind),
            path: value.path.clone(),
            name: value.name.clone(),
            duration: value.duration,
            width: value.width,
            height: value.height,
            framerate: value.framerate,
            frame_rate_numerator: value.frame_rate_numerator,
            frame_rate_denominator: value.frame_rate_denominator,
            codec: value.codec.clone(),
            has_audio: value.has_audio,
        }
    }

    fn to_runtime(&self) -> runtime::MediaAsset {
        runtime::MediaAsset {
            id: self.id,
            kind: self.kind.to_runtime(),
            path: self.path.clone(),
            name: self.name.clone(),
            duration: self.duration,
            width: self.width,
            height: self.height,
            framerate: self.framerate,
            frame_rate_numerator: self.frame_rate_numerator,
            frame_rate_denominator: self.frame_rate_denominator,
            codec: self.codec.clone(),
            has_audio: self.has_audio,
        }
    }
}

impl Track {
    fn from_runtime(value: &runtime::Track) -> Self {
        Self {
            id: value.id,
            name: value.name.clone(),
            kind: TrackKind::from_runtime(&value.kind),
            locked: value.locked,
            muted: value.muted,
            visible: value.visible,
        }
    }

    fn to_runtime(&self) -> runtime::Track {
        runtime::Track {
            id: self.id,
            name: self.name.clone(),
            kind: self.kind.to_runtime(),
            locked: self.locked,
            muted: self.muted,
            visible: self.visible,
        }
    }
}

impl VideoClip {
    fn from_runtime(value: &runtime::VideoClip) -> Self {
        Self {
            id: value.id,
            track_id: value.track_id,
            asset_id: value.asset_id,
            timeline_start: value.timeline_start.into(),
            source_in: value.source_in.into(),
            source_out: value.source_out.into(),
            video_properties: VideoClipProperties::from_runtime(&value.video_properties),
            audio_properties: AudioClipProperties::from_runtime(&value.audio_properties),
        }
    }

    fn to_runtime(&self) -> runtime::VideoClip {
        runtime::VideoClip {
            id: self.id,
            track_id: self.track_id,
            asset_id: self.asset_id,
            timeline_start: self.timeline_start.into(),
            source_in: self.source_in.into(),
            source_out: self.source_out.into(),
            video_properties: self.video_properties.to_runtime(),
            audio_properties: self.audio_properties.to_runtime(),
        }
    }
}

impl AudioClip {
    fn from_runtime(value: &runtime::AudioClip) -> Self {
        Self {
            id: value.id,
            track_id: value.track_id,
            asset_id: value.asset_id,
            timeline_start: value.timeline_start.into(),
            source_in: value.source_in.into(),
            source_out: value.source_out.into(),
            audio_properties: AudioClipProperties::from_runtime(&value.audio_properties),
        }
    }

    fn to_runtime(&self) -> runtime::AudioClip {
        runtime::AudioClip {
            id: self.id,
            track_id: self.track_id,
            asset_id: self.asset_id,
            timeline_start: self.timeline_start.into(),
            source_in: self.source_in.into(),
            source_out: self.source_out.into(),
            audio_properties: self.audio_properties.to_runtime(),
        }
    }
}

impl TextClip {
    fn from_runtime(value: &runtime::TextClip) -> Self {
        Self {
            id: value.id,
            track_id: value.track_id,
            timeline_start: value.timeline_start.into(),
            length: value.duration,
            properties: TextClipProperties::from_runtime(&value.properties),
        }
    }

    fn to_runtime(&self) -> runtime::TextClip {
        runtime::TextClip {
            id: self.id,
            track_id: self.track_id,
            timeline_start: self.timeline_start.into(),
            duration: self.length,
            properties: self.properties.to_runtime(),
        }
    }
}

impl VideoClipProperties {
    fn from_runtime(value: &runtime::VideoClipProperties) -> Self {
        Self {
            position_x: value.position_x,
            position_y: value.position_y,
            scale: value.scale,
        }
    }

    fn to_runtime(&self) -> runtime::VideoClipProperties {
        runtime::VideoClipProperties {
            position_x: self.position_x,
            position_y: self.position_y,
            scale: self.scale,
        }
    }
}

impl AudioClipProperties {
    fn from_runtime(value: &runtime::AudioClipProperties) -> Self {
        Self {
            gain_db: value.gain_db,
            muted: value.muted,
        }
    }

    fn to_runtime(&self) -> runtime::AudioClipProperties {
        runtime::AudioClipProperties {
            gain_db: self.gain_db,
            muted: self.muted,
        }
    }
}

impl TextClipProperties {
    fn from_runtime(value: &runtime::TextClipProperties) -> Self {
        Self {
            text: value.text.clone(),
            font: value.font.clone(),
            font_size: value.font_size,
            color: value.color,
            position_x: value.position.x,
            position_y: value.position.y,
        }
    }

    fn to_runtime(&self) -> runtime::TextClipProperties {
        runtime::TextClipProperties {
            text: self.text.clone(),
            font: self.font.clone(),
            font_size: self.font_size,
            color: self.color,
            position: gpui::point(self.position_x, self.position_y),
        }
    }
}

impl MediaKind {
    fn from_runtime(value: &runtime::MediaKind) -> Self {
        match value {
            runtime::MediaKind::Video => Self::Video,
            runtime::MediaKind::Image => Self::Image,
            runtime::MediaKind::Audio => Self::Audio,
        }
    }

    fn to_runtime(&self) -> runtime::MediaKind {
        match self {
            Self::Video => runtime::MediaKind::Video,
            Self::Image => runtime::MediaKind::Image,
            Self::Audio => runtime::MediaKind::Audio,
        }
    }
}

impl TrackKind {
    fn from_runtime(value: &runtime::TrackKind) -> Self {
        match value {
            runtime::TrackKind::Video => Self::Video,
            runtime::TrackKind::Audio => Self::Audio,
            runtime::TrackKind::Text => Self::Text,
        }
    }

    fn to_runtime(&self) -> runtime::TrackKind {
        match self {
            Self::Video => runtime::TrackKind::Video,
            Self::Audio => runtime::TrackKind::Audio,
            Self::Text => runtime::TrackKind::Text,
        }
    }
}

impl Clip {
    fn from_runtime(value: &runtime::Clip) -> Self {
        match value {
            runtime::Clip::Video(clip) => Self::Video(VideoClip::from_runtime(clip)),
            runtime::Clip::Audio(clip) => Self::Audio(AudioClip::from_runtime(clip)),
            runtime::Clip::Text(clip) => Self::Text(TextClip::from_runtime(clip)),
        }
    }

    fn to_runtime(&self) -> runtime::Clip {
        match self {
            Self::Video(clip) => runtime::Clip::Video(clip.to_runtime()),
            Self::Audio(clip) => runtime::Clip::Audio(clip.to_runtime()),
            Self::Text(clip) => runtime::Clip::Text(clip.to_runtime()),
        }
    }
}
