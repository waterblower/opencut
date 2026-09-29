use anyhow::{Context, Result, bail};
use ffmpeg_next::Rational;

/// Signed normalized microseconds: preserve stream offsets and preroll.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MediaTime(pub i64);

// Round to the closest microsecond, with ties away from zero. i128 arithmetic
// avoids intermediate overflow and keeps large/nonzero stream origins exact.
pub fn timestamp_microseconds(value: i64, time_base: Rational) -> Result<i64> {
    if time_base.numerator() <= 0 || time_base.denominator() <= 0 {
        bail!("invalid stream time base: {time_base:?}");
    }
    let numerator = i128::from(value) * i128::from(time_base.numerator()) * 1_000_000;
    let denominator = i128::from(time_base.denominator());
    let rounded = if numerator < 0 {
        (numerator - denominator / 2) / denominator
    } else {
        (numerator + denominator / 2) / denominator
    };
    i64::try_from(rounded).context("timestamp exceeds microsecond range")
}
