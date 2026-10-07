use serde::{Deserialize, Serialize};
use std::{
    ops::{Add, AddAssign, Sub, SubAssign},
    time::Duration,
};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
#[serde(transparent)]
pub struct TimelineFrameIndex(i64);

impl TimelineFrameIndex {
    pub const ZERO: Self = Self(0);
    pub const ONE_FRAME: Self = Self(1);

    pub fn abs_diff(self, other: Self) -> u64 {
        self.0.abs_diff(other.0)
    }
}

impl From<i64> for TimelineFrameIndex {
    fn from(value: i64) -> Self {
        Self(value)
    }
}

impl From<TimelineFrameIndex> for i64 {
    fn from(value: TimelineFrameIndex) -> Self {
        value.0
    }
}

impl Add for TimelineFrameIndex {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl AddAssign for TimelineFrameIndex {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for TimelineFrameIndex {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl SubAssign for TimelineFrameIndex {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
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

impl FrameRate {
    pub fn samples(self, frames: i64, rate: u32) -> i64 {
        let n = frames as i128 * self.denominator as i128 * rate as i128;
        ((n + self.numerator as i128 / 2) / self.numerator.max(1) as i128)
            .clamp(i64::MIN as i128, i64::MAX as i128) as i64
    }

    pub const fn new(numerator: u32, denominator: u32) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    pub fn frames_per_second(self) -> f64 {
        self.numerator as f64 / self.denominator.max(1) as f64
    }

    pub fn seconds(self, time: TimelineFrameIndex) -> f64 {
        i64::from(time) as f64 * self.denominator.max(1) as f64 / self.numerator.max(1) as f64
    }

    pub fn duration(self, time: TimelineFrameIndex) -> Duration {
        let frames = i64::from(time).max(0) as u128;
        let numerator = frames
            .saturating_mul(self.denominator.max(1) as u128)
            .saturating_mul(1_000_000_000);
        let nanos = divide_round(numerator, self.numerator.max(1) as u128);
        Duration::from_nanos(nanos.min(u64::MAX as u128) as u64)
    }

    pub fn frames_from_duration_nearest(self, duration: Duration) -> TimelineFrameIndex {
        let numerator = duration
            .as_nanos()
            .saturating_mul(self.numerator.max(1) as u128);
        let denominator = (self.denominator.max(1) as u128).saturating_mul(1_000_000_000);

        (divide_round(numerator, denominator).min(i64::MAX as u128) as i64).into()
    }

    pub fn audio_samples(self, time: TimelineFrameIndex, sample_rate: u32) -> u64 {
        let frames = i64::from(time).max(0) as u128;
        let numerator = frames
            .saturating_mul(self.denominator.max(1) as u128)
            .saturating_mul(sample_rate as u128);
        divide_round(numerator, self.numerator.max(1) as u128).min(u64::MAX as u128) as u64
    }

    pub fn nearest(self, seconds: f64) -> TimelineFrameIndex {
        if !seconds.is_finite() || seconds <= 0.0 {
            return TimelineFrameIndex::ZERO;
        }
        ((seconds * self.frames_per_second())
            .round()
            .clamp(0.0, i64::MAX as f64) as i64)
            .into()
    }
    pub fn ceil(self, seconds: f64) -> TimelineFrameIndex {
        if !seconds.is_finite() || seconds <= 0.0 {
            return TimelineFrameIndex::ZERO;
        }
        ((seconds * self.frames_per_second())
            .ceil()
            .clamp(0.0, i64::MAX as f64) as i64)
            .into()
    }
    pub fn delta(self, seconds: f64) -> TimelineFrameIndex {
        if !seconds.is_finite() {
            return TimelineFrameIndex::ZERO;
        }
        ((seconds * self.frames_per_second())
            .round()
            .clamp(i64::MIN as f64, i64::MAX as f64) as i64)
            .into()
    }
    pub fn rescale_nearest(self, time: TimelineFrameIndex, target: Self) -> TimelineFrameIndex {
        if time <= TimelineFrameIndex::ZERO {
            return TimelineFrameIndex::ZERO;
        }
        let numerator = i64::from(time) as u128
            * self.denominator.max(1) as u128
            * target.numerator.max(1) as u128;
        let denominator = self.numerator.max(1) as u128 * target.denominator.max(1) as u128;
        (divide_round(numerator, denominator).min(i64::MAX as u128) as i64).into()
    }
    pub fn rescale_floor(self, time: TimelineFrameIndex, target: Self) -> TimelineFrameIndex {
        if time <= TimelineFrameIndex::ZERO {
            return TimelineFrameIndex::ZERO;
        }
        let numerator = i64::from(time) as u128
            * self.denominator.max(1) as u128
            * target.numerator.max(1) as u128;
        let denominator = self.numerator.max(1) as u128 * target.denominator.max(1) as u128;
        ((numerator / denominator).min(i64::MAX as u128) as i64).into()
    }
}

fn divide_round(numerator: u128, denominator: u128) -> u128 {
    numerator.saturating_add(denominator / 2) / denominator.max(1)
}
