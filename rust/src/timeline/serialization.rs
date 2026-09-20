use crate::timeline as runtime;
use crate::timeline::TimelineEditingState as RuntimeTimelineEditingState;
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fmt, fs,
    path::{Path, PathBuf},
    time::Duration,
};
use ulid::Ulid;

/// Disk-only document. Runtime content crosses this boundary through explicit copies.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct TimelineSerialization {
    editing_state: TimelineEditingState,
    view_state: TimelineViewState,
}

impl TimelineSerialization {
    /// Captures editing content with default persisted view preferences.
    pub fn from_editing_state(editing_state: &RuntimeTimelineEditingState) -> Self {
        Self {
            editing_state: TimelineEditingState::from_runtime(editing_state),
            view_state: TimelineViewState::default(),
        }
    }

    /// Rebuilds independent runtime content without exposing disk types.
    pub fn to_editing_state(&self) -> RuntimeTimelineEditingState {
        self.editing_state.to_runtime()
    }

    pub fn set_view_state(
        &mut self,
        playhead: runtime::TimelineTime,
        scroll: (f32, f32),
        pixels_per_second: f32,
        snapping_enabled: bool,
        track_magnet_enabled: bool,
    ) {
        self.view_state = TimelineViewState {
            saved_playhead_frame: playhead.frames().max(0),
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

    pub fn playhead(&self) -> runtime::TimelineTime {
        runtime::TimelineTime::from_frames(self.view_state.saved_playhead_frame.max(0))
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
        let value = serde_json::from_slice(&contents)
            .context(format!("Parsing timeline JSON {}", path.display()))?;
        Ok(parse(&value)?)
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

#[derive(Debug, Serialize)]
pub struct ParseError {
    pub code: &'static str,
    pub pointer: String,
    pub message: String,
    pub file: &'static str,
    pub line: u32,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {} ({}, {}:{})",
            self.code, self.message, self.pointer, self.file, self.line
        )
    }
}
impl std::error::Error for ParseError {}

/// Deserialize the GUI timeline representation, including its existing aliases.
/// The input and output are in memory; callers own file I/O and validation policy.
pub fn parse(value: &Value) -> Result<TimelineSerialization, ParseError> {
    if value.get("version").is_some()
        || value.get("transitions").is_some()
        || value
            .get("clips")
            .and_then(Value::as_array)
            .is_some_and(|clips| clips.iter().any(|clip| clip.get("type").is_some()))
    {
        return Err(ParseError {
            code: "legacy_cli_format",
            pointer: String::new(),
            message:
                "the legacy CLI timeline is unsupported; use the shared editor timeline format"
                    .into(),
            file: file!(),
            line: line!(),
        });
    }
    let mut value = value.clone();
    // Existing editor files stored content and view preferences at the root.
    if let Some(object) = value.as_object_mut()
        && !object.contains_key("editing_state")
    {
        let view = object
            .remove("view_state")
            .or_else(|| object.remove("view"));
        let editing = Value::Object(std::mem::take(object));
        object.insert("editing_state".into(), editing);
        if let Some(view) = view {
            object.insert("view_state".into(), view);
        }
    }
    let frame_rate = match value.pointer("/editing_state/settings/frame_rate") {
        Some(rate) => match serde_json::from_value::<FrameRate>(rate.clone()) {
            Ok(rate) => rate,
            Err(error) => {
                return Err(ParseError {
                    code: "schema_error",
                    pointer: "/editing_state/settings/frame_rate".into(),
                    message: error.to_string(),
                    file: file!(),
                    line: line!(),
                });
            }
        },
        None => FrameRate::default(),
    };
    if let Some(clips) = value
        .pointer_mut("/editing_state/clips")
        .and_then(Value::as_array_mut)
    {
        for clip in clips {
            if clip.get("kind").and_then(Value::as_str) != Some("Text") {
                continue;
            }
            let Some(clip) = clip.get_mut("data").and_then(Value::as_object_mut) else {
                continue;
            };
            let Some(frames) = clip.get("length").and_then(Value::as_i64) else {
                continue;
            };
            let duration = frame_rate
                .to_runtime()
                .duration(runtime::TimelineTime::from_frames(frames));
            clip.insert(
                "length".into(),
                serde_json::json!({"secs": duration.as_secs(), "nanos": duration.subsec_nanos()}),
            );
        }
    }
    match serde_path_to_error::deserialize::<_, TimelineSerialization>(&value) {
        Ok(mut document) => {
            for clip in &mut document.editing_state.clips {
                let track_id = match clip {
                    Clip::Video(clip) | Clip::Audio(clip) => clip.track_id,
                    Clip::Text(clip) => clip.track_id,
                };
                let track_kind = document
                    .editing_state
                    .tracks
                    .iter()
                    .find(|track| track.id == track_id)
                    .map(|track| track.kind);
                let replacement = match (track_kind, &*clip) {
                    (Some(TrackKind::Audio), Clip::Video(data)) => Some(Clip::Audio(data.clone())),
                    (Some(TrackKind::Video), Clip::Audio(data)) => Some(Clip::Video(data.clone())),
                    _ => None,
                };
                if let Some(replacement) = replacement {
                    *clip = replacement;
                }
            }
            Ok(document)
        }
        Err(error) => {
            let mut pointer = String::new();
            for segment in error.path() {
                let token = match segment {
                    serde_path_to_error::Segment::Seq { index } => index.to_string(),
                    serde_path_to_error::Segment::Map { key } => key.clone(),
                    serde_path_to_error::Segment::Enum { variant } => variant.clone(),
                    serde_path_to_error::Segment::Unknown => continue,
                };
                pointer.push('/');
                pointer.push_str(&token.replace('~', "~0").replace('/', "~1"));
            }
            Err(ParseError {
                code: "schema_error",
                pointer,
                message: error.inner().to_string(),
                file: file!(),
                line: line!(),
            })
        }
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
struct TimelineEditingState {
    settings: TimelineSettings,
    assets: Vec<MediaAsset>,
    #[serde(alias = "layers")]
    tracks: Vec<Track>,
    clips: Vec<Clip>,
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
struct TimelineSettings {
    frame_rate: FrameRate,
    width: u32,
    height: u32,
    audio_sample_rate: u32,
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
enum MediaKind {
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
struct MediaAsset {
    #[serde(deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    id: Ulid,
    #[serde(default)]
    kind: MediaKind,
    path: PathBuf,
    name: String,
    duration: f64,
    width: u32,
    height: u32,
    framerate: f64,
    #[serde(default)]
    frame_rate_numerator: u32,
    #[serde(default)]
    frame_rate_denominator: u32,
    codec: String,
    has_audio: bool,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
enum TrackKind {
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
struct Track {
    #[serde(deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    id: Ulid,
    name: String,
    kind: TrackKind,
    #[serde(default)]
    locked: bool,
    #[serde(default)]
    muted: bool,
    #[serde(default)]
    visible: bool,
}
/// Static visual adjustments for one timeline clip.
///
/// Position is an offset in timeline pixels from the clip's centered placement. Scale is a
/// normalized multiplier, so `1.0` means 100%.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
#[serde(default)]
struct VideoClipProperties {
    position_x: f64,
    position_y: f64,
    scale: f64,
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
struct AudioClipProperties {
    gain_db: f64,
    muted: bool,
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
struct TextClipProperties {
    text: String,
    font: String,
    font_size: f64,
    /// Text color as big-endian ARGB.
    color: u32,
    position_x: f64,
    position_y: f64,
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
enum Clip {
    #[serde(alias = "Media")]
    Video(MediaClipData),
    Audio(MediaClipData),
    Text(TextClip),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
struct MediaClipData {
    #[serde(deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    id: Ulid,
    #[serde(alias = "layer_id", deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    track_id: Ulid,
    #[serde(default = "Ulid::nil", deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    asset_id: Ulid,
    timeline_start: i64,
    source_in: i64,
    source_out: i64,
    #[serde(default)]
    video_properties: VideoClipProperties,
    #[serde(default)]
    audio_properties: AudioClipProperties,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
struct TextClip {
    #[serde(deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    id: Ulid,
    #[serde(deserialize_with = "deserialize_ulid")]
    #[cfg_attr(feature = "timeline-schema", schemars(with = "String"))]
    track_id: Ulid,
    timeline_start: i64,
    length: Duration,
    properties: TextClipProperties,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
struct FrameRate {
    numerator: u32,
    denominator: u32,
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

impl MediaClipData {
    fn from_runtime(value: &runtime::MediaClipData) -> Self {
        Self {
            id: value.id,
            track_id: value.track_id,
            asset_id: value.asset_id,
            timeline_start: value.timeline_start.frames(),
            source_in: value.source_in.frames(),
            source_out: value.source_out.frames(),
            video_properties: VideoClipProperties::from_runtime(&value.video_properties),
            audio_properties: AudioClipProperties::from_runtime(&value.audio_properties),
        }
    }

    fn to_runtime(&self) -> runtime::MediaClipData {
        runtime::MediaClipData {
            id: self.id,
            track_id: self.track_id,
            asset_id: self.asset_id,
            timeline_start: runtime::TimelineTime::from_frames(self.timeline_start),
            source_in: runtime::TimelineTime::from_frames(self.source_in),
            source_out: runtime::TimelineTime::from_frames(self.source_out),
            video_properties: self.video_properties.to_runtime(),
            audio_properties: self.audio_properties.to_runtime(),
        }
    }
}

impl TextClip {
    fn from_runtime(value: &runtime::TextClip) -> Self {
        Self {
            id: value.id,
            track_id: value.track_id,
            timeline_start: value.timeline_start.frames(),
            length: value.length,
            properties: TextClipProperties::from_runtime(&value.properties),
        }
    }

    fn to_runtime(&self) -> runtime::TextClip {
        runtime::TextClip {
            id: self.id,
            track_id: self.track_id,
            timeline_start: runtime::TimelineTime::from_frames(self.timeline_start),
            length: self.length,
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
            position_x: value.position_x,
            position_y: value.position_y,
        }
    }

    fn to_runtime(&self) -> runtime::TextClipProperties {
        runtime::TextClipProperties {
            text: self.text.clone(),
            font: self.font.clone(),
            font_size: self.font_size,
            color: self.color,
            position_x: self.position_x,
            position_y: self.position_y,
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
            runtime::Clip::Video(clip) => Self::Video(MediaClipData::from_runtime(clip)),
            runtime::Clip::Audio(clip) => Self::Audio(MediaClipData::from_runtime(clip)),
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
