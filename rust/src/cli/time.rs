pub use crate::timeline::FrameRate;
use anyhow::{Context as _, Result, anyhow};

impl FrameRate {
    pub fn parse_time(self, input: &str, total: Option<i64>) -> Result<i64> {
        if self.numerator == 0 || self.denominator == 0 {
            return Err(anyhow!(
                "invalid_frame_rate: frame rate must be positive (/settings/frame_rate) at {}:{}",
                file!(),
                line!()
            ));
        }
        if let Some(value) = input.strip_suffix('f') {
            let frame: i64 =
                value
                    .parse()
                    .context(format!("invalid_time at {}:{}", file!(), line!()))?;
            if frame < 0 {
                return Err(anyhow!(
                    "invalid_time: time cannot be negative at {}:{}",
                    file!(),
                    line!()
                ));
            }
            return Ok(frame);
        }
        let (n, d, multiplier) = if let Some(value) = input.strip_suffix('%') {
            let Some(total) = total else {
                return Err(anyhow!(
                    "invalid_time: percent time is supported only for still --at at {}:{}",
                    file!(),
                    line!()
                ));
            };
            let (n, d) = decimal(value)?;
            if n > d * 100 {
                return Err(anyhow!(
                    "invalid_time: percentage must be between 0 and 100 at {}:{}",
                    file!(),
                    line!()
                ));
            }
            (n, d * 100, total.saturating_sub(1) as i128)
        } else {
            let value = input.strip_suffix('s').unwrap_or(input);
            let parts: Vec<_> = value.split(':').collect();
            let (n, d) = match parts.as_slice() {
                [seconds] => decimal(seconds)?,
                [hours, minutes, seconds] => {
                    let h: i128 = hours.parse().context(format!(
                        "invalid_time at {}:{}",
                        file!(),
                        line!()
                    ))?;
                    let m: i128 = minutes.parse().context(format!(
                        "invalid_time at {}:{}",
                        file!(),
                        line!()
                    ))?;
                    let (s, d) = decimal(seconds)?;
                    if h < 0 || !(0..60).contains(&m) || s >= 60 * d || h > 1_000_000_000 {
                        return Err(anyhow!(
                            "invalid_time: invalid timecode at {}:{}",
                            file!(),
                            line!()
                        ));
                    }
                    ((h * 3600 + m * 60) * d + s, d)
                }
                _ => {
                    return Err(anyhow!(
                        "invalid_time: expected seconds, frames, or HH:MM:SS at {}:{}",
                        file!(),
                        line!()
                    ));
                }
            };
            (n, d * self.denominator as i128, self.numerator as i128)
        };
        let Some(scaled) = n.checked_mul(multiplier) else {
            return Err(anyhow!(
                "invalid_time: time is too large at {}:{}",
                file!(),
                line!()
            ));
        };
        let value = (scaled + d / 2) / d;
        Ok(i64::try_from(value).context(format!("invalid_time at {}:{}", file!(), line!()))?)
    }
}

pub fn parse_rate(input: &str) -> Result<FrameRate> {
    let (n, d) = if let Some((n, d)) = input.split_once('/') {
        (
            n.parse::<i128>()
                .context(format!("invalid_frame_rate at {}:{}", file!(), line!()))?,
            d.parse::<i128>()
                .context(format!("invalid_frame_rate at {}:{}", file!(), line!()))?,
        )
    } else {
        decimal(input)?
    };
    if n <= 0 || d <= 0 {
        return Err(anyhow!(
            "invalid_frame_rate: frame rate must be positive at {}:{}",
            file!(),
            line!()
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
        numerator: u32::try_from(n / a).context(format!(
            "invalid_frame_rate at {}:{}",
            file!(),
            line!()
        ))?,
        denominator: u32::try_from(d / a).context(format!(
            "invalid_frame_rate at {}:{}",
            file!(),
            line!()
        ))?,
    })
}

fn decimal(value: &str) -> Result<(i128, i128)> {
    let parts: Vec<_> = value.split('.').collect();
    if parts.len() > 2
        || value.is_empty()
        || value.len() > 18
        || value.bytes().any(|b| !b.is_ascii_digit() && b != b'.')
    {
        return Err(anyhow!(
            "invalid_time: invalid nonnegative decimal: {value} at {}:{}",
            file!(),
            line!()
        ));
    }
    let places = if parts.len() == 2 { parts[1].len() } else { 0 };
    let digits = value.replace('.', "");
    Ok((
        digits
            .parse()
            .context(format!("invalid_time at {}:{}", file!(), line!()))?,
        10_i128.pow(places as u32),
    ))
}
