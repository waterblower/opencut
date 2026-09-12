use super::error::{Error, Result};

/// Merge adjacent SRT cues separated by less than 100 ms, including overlaps.
/// Concatenate text verbatim and renumber cues; output uses LF line endings.
pub fn merge_srt_sections(srt: &str) -> Result<String> {
    let normalized = srt.replace("\r\n", "\n");
    let mut cues: Vec<(u64, u64, String, String, String)> = Vec::new();
    for block in normalized.trim_matches('\n').split("\n\n") {
        if block.trim().is_empty() {
            continue;
        }
        let mut lines = block.lines();
        match lines.next().unwrap_or("").parse::<u64>() {
            Ok(value) => value,
            Err(error) => return Err(Error::new("invalid_srt", error, file!(), line!())),
        };
        let Some((start, end)) = lines.next().unwrap_or("").split_once(" --> ") else {
            return Err(Error::new(
                "invalid_srt",
                "expected SRT time range",
                file!(),
                line!(),
            ));
        };
        let start_ms = timestamp_ms(start)?;
        let end_ms = timestamp_ms(end)?;
        let text = lines.collect::<Vec<_>>().join("\n");
        if end_ms < start_ms || text.is_empty() {
            return Err(Error::new(
                "invalid_srt",
                "invalid cue duration or empty text",
                file!(),
                line!(),
            ));
        }
        if let Some(previous) = cues.last_mut() {
            if start_ms < previous.0 {
                return Err(Error::new(
                    "invalid_srt",
                    "cues must be ordered by start time",
                    file!(),
                    line!(),
                ));
            }
            if start_ms.saturating_sub(previous.1) < 100 {
                if end_ms > previous.1 {
                    previous.1 = end_ms;
                    previous.3 = end.into();
                }
                previous.4.push_str(&text);
                continue;
            }
        }
        cues.push((start_ms, end_ms, start.into(), end.into(), text));
    }
    let mut output = String::new();
    for (index, (_, _, start, end, text)) in cues.iter().enumerate() {
        output.push_str(&format!("{}\n{start} --> {end}\n{text}\n\n", index + 1));
    }
    Ok(output)
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
        return Err(Error::new(
            "invalid_srt",
            format!("invalid timestamp: {timestamp}"),
            file!(),
            line!(),
        ));
    }
    let mut values = [0_u64; 4];
    for (value, part) in values.iter_mut().zip(parts) {
        *value = match part.parse::<u64>() {
            Ok(value) => value,
            Err(error) => return Err(Error::new("invalid_srt", error, file!(), line!())),
        };
    }
    let [hours, minutes, seconds, millis] = values;
    if minutes >= 60 || seconds >= 60 {
        return Err(Error::new(
            "invalid_srt",
            format!("invalid timestamp: {timestamp}"),
            file!(),
            line!(),
        ));
    }
    let Some(total) = hours
        .checked_mul(3_600_000)
        .and_then(|total| total.checked_add(minutes * 60_000 + seconds * 1000 + millis))
    else {
        return Err(Error::new(
            "invalid_srt",
            format!("timestamp overflow: {timestamp}"),
            file!(),
            line!(),
        ));
    };
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_chains_but_keeps_exactly_100_ms_gaps() {
        let input = "1\r\n00:00:00,300 --> 00:00:00,440\r\n我\r\n\r\n2\r\n00:00:00,440 --> 00:00:00,520\r\n们\r\n\r\n3\r\n00:00:00,619 --> 00:00:00,700\r\n好\r\n\r\n4\r\n00:00:00,800 --> 00:00:01,000\r\n再见。\r\n";
        assert_eq!(
            merge_srt_sections(input).unwrap(),
            "1\n00:00:00,300 --> 00:00:00,700\n我们好\n\n2\n00:00:00,800 --> 00:00:01,000\n再见。\n\n"
        );
    }

    #[test]
    fn preserves_outer_end_for_overlapping_cues() {
        let input =
            "1\n00:00:00,000 --> 00:00:02,000\nhello \n\n2\n00:00:01,000 --> 00:00:01,500\nworld\n";
        assert_eq!(
            merge_srt_sections(input).unwrap(),
            "1\n00:00:00,000 --> 00:00:02,000\nhello world\n\n"
        );
        assert_eq!(merge_srt_sections("").unwrap(), "");
        assert!(merge_srt_sections("1\n00:00:61,000 --> 00:01:02,000\nx").is_err());
        assert!(merge_srt_sections("1\n00:00:02,000 --> 00:00:01,000\nx").is_err());
    }
}
