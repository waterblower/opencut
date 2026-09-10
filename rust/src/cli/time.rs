pub use crate::timeline::FrameRate;
use crate::{cli::error::Result, cli_error, cli_try};

impl FrameRate {
    pub fn samples(self, frames: i64, rate: u32) -> i64 {
        let n = frames as i128 * self.denominator as i128 * rate as i128;
        ((n + self.numerator as i128 / 2) / self.numerator.max(1) as i128)
            .clamp(i64::MIN as i128, i64::MAX as i128) as i64
    }

    pub fn parse_time(self, input: &str, total: Option<i64>) -> Result<i64> {
        if self.numerator == 0 || self.denominator == 0 {
            return Err(cli_error!(
                "invalid_frame_rate",
                "/settings/frame_rate",
                3,
                "frame rate must be positive"
            ));
        }
        if let Some(value) = input.strip_suffix('f') {
            let frame: i64 = cli_try!(value.parse(), "invalid_time", "", 2);
            if frame < 0 {
                return Err(cli_error!("invalid_time", "", 2, "time cannot be negative"));
            }
            return Ok(frame);
        }
        let (n, d, multiplier) = if let Some(value) = input.strip_suffix('%') {
            let Some(total) = total else {
                return Err(cli_error!(
                    "invalid_time",
                    "",
                    2,
                    "percent time is supported only for still --at"
                ));
            };
            let (n, d) = decimal(value)?;
            if n > d * 100 {
                return Err(cli_error!(
                    "invalid_time",
                    "",
                    2,
                    "percentage must be between 0 and 100"
                ));
            }
            (n, d * 100, total.saturating_sub(1) as i128)
        } else {
            let value = input.strip_suffix('s').unwrap_or(input);
            let parts: Vec<_> = value.split(':').collect();
            let (n, d) = match parts.as_slice() {
                [seconds] => decimal(seconds)?,
                [hours, minutes, seconds] => {
                    let h: i128 = cli_try!(hours.parse(), "invalid_time", "", 2);
                    let m: i128 = cli_try!(minutes.parse(), "invalid_time", "", 2);
                    let (s, d) = decimal(seconds)?;
                    if h < 0 || !(0..60).contains(&m) || s >= 60 * d || h > 1_000_000_000 {
                        return Err(cli_error!("invalid_time", "", 2, "invalid timecode"));
                    }
                    ((h * 3600 + m * 60) * d + s, d)
                }
                _ => {
                    return Err(cli_error!(
                        "invalid_time",
                        "",
                        2,
                        "expected seconds, frames, or HH:MM:SS"
                    ));
                }
            };
            (n, d * self.denominator as i128, self.numerator as i128)
        };
        let Some(scaled) = n.checked_mul(multiplier) else {
            return Err(cli_error!("invalid_time", "", 2, "time is too large"));
        };
        let value = (scaled + d / 2) / d;
        Ok(cli_try!(i64::try_from(value), "invalid_time", "", 2))
    }
}

pub fn parse_rate(input: &str) -> Result<FrameRate> {
    let (n, d) = if let Some((n, d)) = input.split_once('/') {
        (
            cli_try!(n.parse::<i128>(), "invalid_frame_rate", "", 2),
            cli_try!(d.parse::<i128>(), "invalid_frame_rate", "", 2),
        )
    } else {
        decimal(input)?
    };
    if n <= 0 || d <= 0 {
        return Err(cli_error!(
            "invalid_frame_rate",
            "",
            2,
            "frame rate must be positive"
        ));
    }
    let mut a = n;
    let mut b = d;
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    Ok(FrameRate {
        numerator: cli_try!(u32::try_from(n / a), "invalid_frame_rate", "", 2),
        denominator: cli_try!(u32::try_from(d / a), "invalid_frame_rate", "", 2),
    })
}

fn decimal(value: &str) -> Result<(i128, i128)> {
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() > 2
        || value.is_empty()
        || value.len() > 18
        || value.bytes().any(|b| !b.is_ascii_digit() && b != b'.')
    {
        return Err(cli_error!(
            "invalid_time",
            "",
            2,
            "invalid nonnegative decimal: {value}"
        ));
    }
    let places = if parts.len() == 2 { parts[1].len() } else { 0 };
    let digits = value.replace('.', "");
    Ok((
        cli_try!(digits.parse(), "invalid_time", "", 2),
        10_i128.pow(places as u32),
    ))
}
