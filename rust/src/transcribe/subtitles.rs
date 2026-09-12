use anyhow::Result;
use std::{fmt, time::Duration};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SRT {
    pub subtitles: Vec<Subtitle>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subtitle {
    pub text: String,
    pub start: Duration,
    pub end: Duration,
}

impl SRT {
    /// Parse SRT text, accepting LF or CRLF and an optional UTF-8 BOM.
    /// Cue numbers are validated but regenerated when serialized.
    pub fn from_string(input: &str) -> Result<Self> {
        let normalized = input.trim_start_matches('\u{feff}').replace("\r\n", "\n");
        let mut subtitles = Vec::new();
        for block in normalized.trim_matches('\n').split("\n\n") {
            if block.trim().is_empty() {
                continue;
            }
            let mut lines = block.lines();
            if let Err(error) = lines.next().unwrap_or("").parse::<u64>() {
                anyhow::bail!(
                    "invalid_srt: invalid cue number: {error} at {}:{}",
                    file!(),
                    line!()
                );
            }
            let Some((start, end)) = lines.next().unwrap_or("").split_once(" --> ") else {
                anyhow::bail!(
                    "invalid_srt: expected SRT time range at {}:{}",
                    file!(),
                    line!()
                );
            };
            let start = Duration::from_millis(timestamp_ms(start)?);
            let end = Duration::from_millis(timestamp_ms(end)?);
            let text = lines.collect::<Vec<_>>().join("\n");
            if end <= start || text.trim().is_empty() {
                anyhow::bail!(
                    "invalid_srt: invalid cue duration or empty text at {}:{}",
                    file!(),
                    line!()
                );
            }
            subtitles.push(Subtitle { text, start, end });
        }
        Ok(Self { subtitles })
    }
}

/// Provides `to_string()` with sequential cue numbers and LF line endings.
/// SRT has millisecond precision; sub-millisecond timestamps are truncated.
impl fmt::Display for SRT {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, subtitle) in self.subtitles.iter().enumerate() {
            writeln!(formatter, "{}", index + 1)?;
            for (position, time) in [subtitle.start, subtitle.end].iter().enumerate() {
                if position == 1 {
                    write!(formatter, " --> ")?;
                }
                let millis = time.as_millis();
                write!(
                    formatter,
                    "{:02}:{:02}:{:02},{:03}",
                    millis / 3_600_000,
                    millis / 60_000 % 60,
                    millis / 1000 % 60,
                    millis % 1000
                )?;
            }
            writeln!(formatter, "\n{}\n", subtitle.text)?;
        }
        Ok(())
    }
}

/// Merge adjacent SRT cues separated by less than 100 ms, including overlaps.
/// Concatenate text verbatim and renumber cues; output uses LF line endings.
pub fn merge_srt_sections(srt: &SRT) -> Result<SRT> {
    let mut merged = SRT::default();
    for subtitle in &srt.subtitles {
        if let Some(previous) = merged.subtitles.last_mut() {
            if subtitle.start < previous.start {
                anyhow::bail!(
                    "invalid_srt: cues must be ordered by start time at {}:{}",
                    file!(),
                    line!()
                );
            }
            if subtitle.start.saturating_sub(previous.end) < Duration::from_millis(100) {
                previous.end = previous.end.max(subtitle.end);
                previous.text.push_str(&subtitle.text);
                continue;
            }
        }
        merged.subtitles.push(subtitle.clone());
    }
    Ok(merged)
}

fn timestamp_ms(timestamp: &str) -> Result<u64> {
    let parts: Vec<_> = timestamp.split([':', ',']).collect();
    if parts.len() != 4
        || parts[0].len() < 2
        || parts[1].len() != 2
        || parts[2].len() != 2
        || parts[3].len() != 3
        || !parts
            .iter()
            .all(|part| part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return Err(anyhow::anyhow!(
            "invalid_srt: {} at {}:{}",
            format!("invalid timestamp: {timestamp}"),
            file!(),
            line!()
        ));
    }
    let mut values = [0_u64; 4];
    for (value, part) in values.iter_mut().zip(parts) {
        *value = match part.parse::<u64>() {
            Ok(value) => value,
            Err(error) => {
                return Err(anyhow::anyhow!(
                    "invalid_srt: {} at {}:{}",
                    error,
                    file!(),
                    line!()
                ));
            }
        };
    }
    let [hours, minutes, seconds, millis] = values;
    if minutes >= 60 || seconds >= 60 {
        return Err(anyhow::anyhow!(
            "invalid_srt: {} at {}:{}",
            format!("invalid timestamp: {timestamp}"),
            file!(),
            line!()
        ));
    }
    let Some(total) = hours
        .checked_mul(3_600_000)
        .and_then(|total| total.checked_add(minutes * 60_000 + seconds * 1000 + millis))
    else {
        return Err(anyhow::anyhow!(
            "invalid_srt: {} at {}:{}",
            format!("timestamp overflow: {timestamp}"),
            file!(),
            line!()
        ));
    };
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_serializes_multiline_unicode_subtitles() {
        let input = "\u{feff}7\r\n01:02:03,004 --> 01:02:04,005\r\n你好\r\nhello\r\n\r\n";
        let srt = SRT::from_string(input).unwrap();
        assert_eq!(srt.subtitles[0].start, Duration::from_millis(3_723_004));
        assert_eq!(srt.subtitles[0].end, Duration::from_millis(3_724_005));
        assert_eq!(srt.subtitles[0].text, "你好\nhello");
        assert_eq!(
            srt.to_string(),
            "1\n01:02:03,004 --> 01:02:04,005\n你好\nhello\n\n"
        );
        assert_eq!(SRT::from_string(&srt.to_string()).unwrap(), srt);
        assert_eq!(SRT::from_string("").unwrap(), SRT::default());
        assert_eq!(SRT::default().to_string(), "");
    }

    #[test]
    fn rejects_invalid_subtitles() {
        for input in [
            "x\n00:00:00,000 --> 00:00:01,000\ntext",
            "1\n00:00:00,000\ntext",
            "1\n00:60:00,000 --> 01:00:01,000\ntext",
            "1\n00:00:02,000 --> 00:00:01,000\ntext",
            "1\n00:00:01,000 --> 00:00:01,000\ntext",
            "1\n00:00:00,000 --> 00:00:01,000\n",
        ] {
            assert!(SRT::from_string(input).is_err(), "{input}");
        }
    }

    #[test]
    fn merges_chains_but_keeps_exactly_100_ms_gaps() {
        let input = "1\r\n00:00:00,300 --> 00:00:00,440\r\n我\r\n\r\n2\r\n00:00:00,440 --> 00:00:00,520\r\n们\r\n\r\n3\r\n00:00:00,619 --> 00:00:00,700\r\n好\r\n\r\n4\r\n00:00:00,800 --> 00:00:01,000\r\n再见。\r\n";
        assert_eq!(
            merge_srt_sections(&SRT::from_string(input).unwrap())
                .unwrap()
                .to_string(),
            "1\n00:00:00,300 --> 00:00:00,700\n我们好\n\n2\n00:00:00,800 --> 00:00:01,000\n再见。\n\n"
        );
    }

    #[test]
    fn preserves_outer_end_for_overlapping_cues() {
        let input =
            "1\n00:00:00,000 --> 00:00:02,000\nhello \n\n2\n00:00:01,000 --> 00:00:01,500\nworld\n";
        assert_eq!(
            merge_srt_sections(&SRT::from_string(input).unwrap())
                .unwrap()
                .to_string(),
            "1\n00:00:00,000 --> 00:00:02,000\nhello world\n\n"
        );
        assert_eq!(merge_srt_sections(&SRT::default()).unwrap(), SRT::default());
        assert!(SRT::from_string("1\n00:00:61,000 --> 00:01:02,000\nx").is_err());
        assert!(SRT::from_string("1\n00:00:02,000 --> 00:00:01,000\nx").is_err());
    }
}
