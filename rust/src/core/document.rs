use crate::{
    cli_error, cli_try,
    core::{error::Result, time::FrameRate},
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use ulid::Ulid;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Document {
    #[schemars(range(min = 1, max = 1))]
    pub version: u32,
    pub settings: Settings,
    pub assets: Vec<Asset>,
    pub tracks: Vec<Track>,
    pub clips: Vec<Clip>,
    #[serde(default)]
    pub transitions: Vec<Transition>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Settings {
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
    pub sample_rate: u32,
    pub background: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            frame_rate: FrameRate::default(),
            sample_rate: 48000,
            background: "#000000".into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Asset {
    pub id: String,
    pub path: PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum TrackKind {
    #[serde(rename = "video")]
    Video,
    #[serde(rename = "audio")]
    Audio,
    #[serde(rename = "text")]
    Text,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Track {
    pub id: String,
    pub kind: TrackKind,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub muted: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
pub enum Clip {
    #[serde(rename = "media")]
    Media {
        #[serde(flatten)]
        common: Common,
        asset_id: String,
        source_in: i64,
        source_out: i64,
        #[serde(default)]
        video_properties: VideoProperties,
        #[serde(default)]
        audio_properties: AudioProperties,
    },
    #[serde(rename = "image")]
    Image {
        #[serde(flatten)]
        common: Common,
        asset_id: String,
        length: i64,
        #[serde(default)]
        video_properties: VideoProperties,
    },
    #[serde(rename = "text")]
    Text {
        #[serde(flatten)]
        common: Common,
        length: Length,
        #[serde(default)]
        properties: TextProperties,
    },
}

impl Clip {
    pub fn common(&self) -> &Common {
        match self {
            Self::Media { common, .. } | Self::Image { common, .. } | Self::Text { common, .. } => {
                common
            }
        }
    }
    pub fn length(&self, fps: FrameRate) -> i64 {
        match self {
            Self::Media {
                source_in,
                source_out,
                ..
            } => source_out.saturating_sub(*source_in),
            Self::Image { length, .. } => *length,
            Self::Text { length, .. } => length.frames(fps),
        }
    }
    pub fn end(&self, fps: FrameRate) -> i64 {
        self.common()
            .timeline_start
            .saturating_add(self.length(fps))
    }
    pub fn asset_id(&self) -> Option<&str> {
        match self {
            Self::Media { asset_id, .. } | Self::Image { asset_id, .. } => Some(asset_id),
            Self::Text { .. } => None,
        }
    }
    pub fn video(&self) -> VideoProperties {
        match self {
            Self::Media {
                video_properties, ..
            }
            | Self::Image {
                video_properties, ..
            } => *video_properties,
            Self::Text { properties, .. } => VideoProperties {
                position_x: properties.position_x,
                position_y: properties.position_y,
                scale: 1.0,
            },
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Common {
    pub id: String,
    pub track_id: String,
    pub timeline_start: i64,
    #[serde(default = "one")]
    pub opacity: f64,
    #[serde(default)]
    pub effects: Vec<Effect>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Length {
    Frames(i64),
    Duration { secs: u64, nanos: u32 },
}

impl Length {
    pub fn frames(&self, fps: FrameRate) -> i64 {
        match self {
            Self::Frames(frames) => *frames,
            Self::Duration { secs, nanos } => {
                let n = (*secs as u128 * 1_000_000_000 + *nanos as u128) * fps.numerator as u128;
                let d = fps.denominator.max(1) as u128 * 1_000_000_000;
                ((n + d / 2) / d).min(i64::MAX as u128) as i64
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct VideoProperties {
    pub position_x: f64,
    pub position_y: f64,
    pub scale: f64,
}
impl Default for VideoProperties {
    fn default() -> Self {
        Self {
            position_x: 0.5,
            position_y: 0.5,
            scale: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct AudioProperties {
    pub gain_db: f64,
    pub muted: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct TextProperties {
    pub text: String,
    pub font: String,
    pub font_size: f64,
    pub color: u32,
    pub position_x: f64,
    pub position_y: f64,
}
impl Default for TextProperties {
    fn default() -> Self {
        Self {
            text: "Text".into(),
            font: "Sans".into(),
            font_size: 64.0,
            color: 0xffffffff,
            position_x: 0.5,
            position_y: 0.5,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
pub enum Effect {
    #[serde(rename = "gaussian_blur")]
    GaussianBlur { radius: f64 },
    #[serde(rename = "color_adjust")]
    ColorAdjust {
        #[serde(default)]
        brightness: f64,
        #[serde(default = "one")]
        contrast: f64,
        #[serde(default = "one")]
        saturation: f64,
    },
    #[serde(rename = "crop")]
    Crop {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    },
    #[serde(rename = "flip")]
    Flip {
        #[serde(default)]
        horizontal: bool,
        #[serde(default)]
        vertical: bool,
    },
}

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, JsonSchema)]
pub enum Direction {
    #[default]
    #[serde(rename = "left")]
    Left,
    #[serde(rename = "right")]
    Right,
    #[serde(rename = "up")]
    Up,
    #[serde(rename = "down")]
    Down,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Transition {
    pub id: String,
    pub from_clip: String,
    pub to_clip: String,
    pub duration: i64,
    #[serde(flatten)]
    pub effect: TransitionEffect,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
pub enum TransitionEffect {
    #[serde(rename = "crossfade")]
    Crossfade,
    #[serde(rename = "dip_to_color")]
    DipToColor { color: String },
    #[serde(rename = "wipe")]
    Wipe {
        #[serde(default)]
        direction: Direction,
    },
    #[serde(rename = "slide")]
    Slide {
        #[serde(default)]
        direction: Direction,
    },
}

impl Document {
    pub fn duration(&self) -> i64 {
        let mut end = 0;
        for clip in &self.clips {
            end = end.max(clip.end(self.settings.frame_rate));
        }
        end
    }
    pub fn clip(&self, id: &str) -> Option<&Clip> {
        self.clips.iter().find(|c| c.common().id == id)
    }
    pub fn asset_path(&self, id: &str, base: &Path) -> Result<PathBuf> {
        let Some(asset) = self.assets.iter().find(|asset| asset.id == id) else {
            return Err(cli_error!(
                "unknown_asset",
                "/assets",
                3,
                "unknown asset {id}"
            ));
        };
        Ok(base.join(&asset.path))
    }
}

pub fn load(path: &Path) -> Result<(Value, Document)> {
    let contents = cli_try!(fs::read(path), "io_error", "", 6);
    let value: Value = cli_try!(serde_json::from_slice(&contents), "invalid_json", "", 3);
    let document = parse(&value)?;
    Ok((value, document))
}

pub fn parse(value: &Value) -> Result<Document> {
    if value.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(cli_error!(
            "unsupported_version",
            "/version",
            3,
            "expected document version 1"
        ));
    }
    match serde_path_to_error::deserialize(value) {
        Ok(document) => Ok(document),
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
            Err(cli_error!("schema_error", &pointer, 3, "{}", error.inner()))
        }
    }
}

pub fn write_atomic(path: &Path, value: &Value, overwrite: bool) -> Result<()> {
    let bytes = cli_try!(serde_json::to_vec_pretty(value), "invalid_json", "", 3);
    let parent = path.parent().unwrap_or(Path::new("."));
    let temp = parent.join(format!(".opencut-{}.tmp", Ulid::generate()));
    let result = (|| {
        let mut file = cli_try!(
            OpenOptions::new().write(true).create_new(true).open(&temp),
            "io_error",
            "",
            6
        );
        cli_try!(file.write_all(&bytes), "io_error", "", 6);
        cli_try!(file.sync_all(), "io_error", "", 6);
        if overwrite {
            cli_try!(fs::rename(&temp, path), "io_error", "", 6);
        } else {
            cli_try!(fs::hard_link(&temp, path), "io_error", "", 6);
        }
        Ok(())
    })();
    if temp.exists() {
        cli_try!(fs::remove_file(&temp), "io_error", "", 6);
    }
    result
}

pub fn color(value: &str) -> Option<[u8; 4]> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 && hex.len() != 8 {
        return None;
    }
    let parsed = u32::from_str_radix(hex, 16).ok()?;
    Some(if hex.len() == 6 {
        [(parsed >> 16) as u8, (parsed >> 8) as u8, parsed as u8, 255]
    } else {
        parsed.to_be_bytes()
    })
}

fn one() -> f64 {
    1.0
}
