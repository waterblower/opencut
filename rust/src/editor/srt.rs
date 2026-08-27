use std::{fs, path::Path, time::Duration};

use anyhow::{Context as _, Result, bail};
use ulid::Ulid;

use crate::editor::{FrameRate, TextClip, TextClipProperties};

pub(in crate::editor) fn srt_to_text_clips(
    path: &Path,
    frame_rate: FrameRate,
) -> Result<Vec<TextClip>> {
    let contents =
        fs::read_to_string(path).with_context(|| format!("could not read {}", path.display()))?;
    parse_srt_text_clips(&contents, frame_rate)
}

fn parse_srt_text_clips(contents: &str, frame_rate: FrameRate) -> Result<Vec<TextClip>> {
    let contents = contents
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n");
    let mut clips = Vec::new();
    for (block_index, block) in contents
        .split("\n\n")
        .filter(|block| !block.trim().is_empty())
        .enumerate()
    {
        let cue_number = block_index + 1;
        let mut lines = block.lines();
        let Some(first_line) = lines.next() else {
            continue;
        };
        let first_line = first_line.trim();
        let timing_line = if first_line.parse::<u64>().is_ok() {
            let Some(timing_line) = lines.next() else {
                bail!("SRT cue {cue_number} has no timestamp line");
            };
            timing_line
        } else {
            first_line
        };
        let Some((start, end)) = timing_line.split_once("-->") else {
            bail!("SRT cue {cue_number} has an invalid timestamp line");
        };
        let Some(start) = parse_srt_timestamp(start) else {
            bail!("SRT cue {cue_number} has an invalid start timestamp");
        };
        let Some(end) = end
            .split_ascii_whitespace()
            .next()
            .and_then(parse_srt_timestamp)
        else {
            bail!("SRT cue {cue_number} has an invalid end timestamp");
        };
        let Some(length) = end.checked_sub(start) else {
            bail!("SRT cue {cue_number} ends before it starts");
        };
        if length.is_zero() {
            bail!("SRT cue {cue_number} has zero duration");
        }
        let text = lines.collect::<Vec<_>>().join("\n").trim().to_string();
        if text.is_empty() {
            continue;
        }
        clips.push(TextClip {
            id: Ulid::generate(),
            track_id: Ulid::nil(),
            timeline_start: frame_rate.frames_from_duration_nearest(start),
            length,
            properties: TextClipProperties {
                text,
                ..TextClipProperties::default()
            },
        });
    }
    Ok(clips)
}

fn parse_srt_timestamp(value: &str) -> Option<Duration> {
    let mut parts = value.trim().split(':');
    let hours = parts.next()?.parse::<u64>().ok()?;
    let minutes = parts.next()?.parse::<u64>().ok()?;
    let seconds_and_millis = parts.next()?;
    if parts.next().is_some() || minutes >= 60 {
        return None;
    }
    let (seconds, millis) = seconds_and_millis
        .split_once(',')
        .or_else(|| seconds_and_millis.split_once('.'))?;
    let seconds = seconds.parse::<u64>().ok()?;
    if seconds >= 60 || millis.is_empty() || millis.len() > 3 {
        return None;
    }
    let millis = millis.parse::<u64>().ok()?
        * match millis.len() {
            1 => 100,
            2 => 10,
            3 => 1,
            _ => return None,
        };
    let total_seconds = hours
        .checked_mul(3_600)?
        .checked_add(minutes.checked_mul(60)?)?
        .checked_add(seconds)?;
    Some(Duration::from_millis(
        total_seconds.checked_mul(1_000)?.checked_add(millis)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cues_at_the_requested_frame_rate() {
        let clips = parse_srt_text_clips(
            "1\n00:00:01,000 --> 00:00:02,500\nHello\nworld\n",
            FrameRate::new(24, 1),
        )
        .unwrap();

        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].timeline_start.frames(), 24);
        assert_eq!(clips[0].length, Duration::from_millis(1_500));
        assert_eq!(clips[0].properties.text, "Hello\nworld");
    }

    #[test]
    fn reports_the_invalid_cue_number() {
        let error =
            parse_srt_text_clips("1\n00:00:01,000 --> invalid\nHello\n", FrameRate::default())
                .unwrap_err();

        assert_eq!(error.to_string(), "SRT cue 1 has an invalid end timestamp");
    }
}
