use serde::{Deserialize, Deserializer, Serialize, de};
use std::{
    ops::{Add, AddAssign, Sub, SubAssign},
    path::PathBuf,
    time::Duration,
};
use ulid::Ulid;

pub(super) const DEFAULT_IMAGE_CLIP_DURATION: f64 = 5.0;

fn deserialize_ulid<'de, D>(deserializer: D) -> Result<Ulid, D::Error>
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

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub(super) struct TimelineTime(i64);

impl TimelineTime {
    pub const ZERO: Self = Self(0);
    pub const ONE_FRAME: Self = Self(1);
    pub const MAX: Self = Self(i64::MAX);

    pub const fn from_frames(frames: i64) -> Self {
        Self(frames)
    }

    pub const fn frames(self) -> i64 {
        self.0
    }

    pub fn abs_diff(self, other: Self) -> u64 {
        self.0.abs_diff(other.0)
    }
}

impl Add for TimelineTime {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl AddAssign for TimelineTime {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for TimelineTime {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl SubAssign for TimelineTime {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct FrameRate {
    pub numerator: u32,
    pub denominator: u32,
}

pub(super) const FRAME_RATE_PRESETS: [(FrameRate, &str); 8] = [
    (FrameRate::new(24_000, 1_001), "23.976 fps"),
    (FrameRate::new(24, 1), "24 fps"),
    (FrameRate::new(25, 1), "25 fps"),
    (FrameRate::new(30_000, 1_001), "29.97 fps"),
    (FrameRate::new(30, 1), "30 fps"),
    (FrameRate::new(50, 1), "50 fps"),
    (FrameRate::new(60_000, 1_001), "59.94 fps"),
    (FrameRate::new(60, 1), "60 fps"),
];

impl Default for FrameRate {
    fn default() -> Self {
        Self {
            numerator: 30,
            denominator: 1,
        }
    }
}

impl FrameRate {
    pub const fn new(numerator: u32, denominator: u32) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    pub fn label(self) -> String {
        if let Some(label) = FRAME_RATE_PRESETS
            .iter()
            .find_map(|(candidate, label)| (*candidate == self).then_some(*label))
        {
            return label.to_string();
        }

        let frames_per_second = self.frames_per_second();
        if frames_per_second.fract().abs() < f64::EPSILON {
            format!("{frames_per_second:.0} fps")
        } else {
            format!("{frames_per_second:.2} fps")
        }
    }

    pub fn frames_per_second(self) -> f64 {
        self.numerator as f64 / self.denominator.max(1) as f64
    }

    pub fn seconds(self, time: TimelineTime) -> f64 {
        time.frames() as f64 * self.denominator.max(1) as f64 / self.numerator.max(1) as f64
    }

    pub fn duration(self, time: TimelineTime) -> Duration {
        let frames = time.frames().max(0) as u128;
        let numerator = frames
            .saturating_mul(self.denominator.max(1) as u128)
            .saturating_mul(1_000_000_000);
        let nanos = divide_round(numerator, self.numerator.max(1) as u128);
        Duration::from_nanos(nanos.min(u64::MAX as u128) as u64)
    }

    pub fn floor_duration(self, duration: Duration) -> TimelineTime {
        let numerator = duration
            .as_nanos()
            .saturating_mul(self.numerator.max(1) as u128);
        let denominator = (self.denominator.max(1) as u128).saturating_mul(1_000_000_000);
        TimelineTime::from_frames((numerator / denominator).min(i64::MAX as u128) as i64)
    }

    pub fn audio_samples(self, time: TimelineTime, sample_rate: u32) -> u64 {
        let frames = time.frames().max(0) as u128;
        let numerator = frames
            .saturating_mul(self.denominator.max(1) as u128)
            .saturating_mul(sample_rate as u128);
        divide_round(numerator, self.numerator.max(1) as u128).min(u64::MAX as u128) as u64
    }

    pub fn nearest(self, seconds: f64) -> TimelineTime {
        // Pointer-driven seeks and edits select the closest timeline frame.
        self.quantize_seconds(seconds, f64::round)
    }

    pub fn ceil(self, seconds: f64) -> TimelineTime {
        // Imported media durations round outward so the last partial frame is retained.
        self.quantize_seconds(seconds, f64::ceil)
    }

    pub fn delta(self, seconds: f64) -> TimelineTime {
        if !seconds.is_finite() {
            return TimelineTime::ZERO;
        }
        let frames = (seconds * self.frames_per_second()).round();
        TimelineTime::from_frames(frames.clamp(i64::MIN as f64, i64::MAX as f64) as i64)
    }

    pub fn rescale_nearest(self, time: TimelineTime, target: Self) -> TimelineTime {
        self.rescale(time, target, divide_round)
    }

    pub fn rescale_floor(self, time: TimelineTime, target: Self) -> TimelineTime {
        self.rescale(time, target, |numerator, denominator| {
            numerator / denominator.max(1)
        })
    }

    fn rescale(
        self,
        time: TimelineTime,
        target: Self,
        round: impl FnOnce(u128, u128) -> u128,
    ) -> TimelineTime {
        if time <= TimelineTime::ZERO {
            return TimelineTime::ZERO;
        }
        let numerator = (time.frames() as u128)
            .saturating_mul(self.denominator.max(1) as u128)
            .saturating_mul(target.numerator.max(1) as u128);
        let denominator =
            (self.numerator.max(1) as u128).saturating_mul(target.denominator.max(1) as u128);
        TimelineTime::from_frames(round(numerator, denominator).min(i64::MAX as u128) as i64)
    }

    fn quantize_seconds(self, seconds: f64, round: impl FnOnce(f64) -> f64) -> TimelineTime {
        if !seconds.is_finite() || seconds <= 0.0 {
            return TimelineTime::ZERO;
        }
        let frames = round(seconds * self.frames_per_second());
        TimelineTime::from_frames(frames.clamp(0.0, i64::MAX as f64) as i64)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct TimelineSettings {
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

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum MediaKind {
    #[default]
    Video,
    Image,
    Audio,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TrackKind {
    #[default]
    Video,
    Audio,
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
pub(super) struct TimelineClip {
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

impl TimelineClip {
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

pub fn timeline_ranges_overlap(
    left_start: TimelineTime,
    left_end: TimelineTime,
    right_start: TimelineTime,
    right_end: TimelineTime,
) -> bool {
    left_start < right_end && right_start < left_end
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct TimelineTrack {
    #[serde(deserialize_with = "deserialize_ulid")]
    pub id: Ulid,
    pub name: String,
    pub kind: TrackKind,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub muted: bool,
    #[serde(default = "default_visible")]
    pub visible: bool,
}

fn default_visible() -> bool {
    true
}

fn divide_round(numerator: u128, denominator: u128) -> u128 {
    numerator.saturating_add(denominator / 2) / denominator.max(1)
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
