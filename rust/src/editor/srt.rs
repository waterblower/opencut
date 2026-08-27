use std::{path::Path, time::Duration};

use ulid::Ulid;

use crate::editor::{FrameRate, TextClip, TextClipProperties};

#[allow(dead_code)] // Called once SRT drop handling is implemented.
pub(in crate::editor) async fn srt_to_text_clips(path: &Path) -> Vec<TextClip> {
    let contents = async_fs::read_to_string(path)
        .await
        .expect("should not fail");
    parse_srt_text_clips(&contents)
}

#[allow(dead_code)] // Used by srt_to_text_clips, which is not wired to drop handling yet.
fn parse_srt_text_clips(contents: &str) -> Vec<TextClip> {
    let contents = contents
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n");
    contents
        .split("\n\n")
        .filter(|block| !block.trim().is_empty())
        .enumerate()
        .filter_map(|(block_index, block)| {
            let mut lines = block.lines();
            let first_line = lines
                .next()
                .expect("non-empty SRT block should contain a line")
                .trim();
            let timing_line = if first_line.parse::<u64>().is_ok() {
                lines
                    .next()
                    .unwrap_or_else(|| panic!("SRT cue {} has no timestamp line", block_index + 1))
            } else {
                first_line
            };
            let Some((start, end)) = timing_line.split_once("-->") else {
                panic!("SRT cue {} has an invalid timestamp line", block_index + 1);
            };
            let start = parse_srt_timestamp(start).unwrap_or_else(|| {
                panic!("SRT cue {} has an invalid start timestamp", block_index + 1)
            });
            let end = end
                .split_ascii_whitespace()
                .next()
                .and_then(parse_srt_timestamp)
                .unwrap_or_else(|| {
                    panic!("SRT cue {} has an invalid end timestamp", block_index + 1)
                });
            let Some(length) = end.checked_sub(start) else {
                panic!("SRT cue {} ends before it starts", block_index + 1);
            };
            if length.is_zero() {
                panic!("SRT cue {} has zero duration", block_index + 1);
            }
            let text = lines.collect::<Vec<_>>().join("\n").trim().to_string();
            if text.is_empty() {
                return None;
            }
            Some(TextClip {
                id: Ulid::generate(),
                track_id: Ulid::nil(),
                timeline_start: FrameRate::default().frames_from_duration_nearest(start),
                length,
                properties: TextClipProperties {
                    text,
                    ..TextClipProperties::default()
                },
            })
        })
        .collect()
}

#[allow(dead_code)] // Used by srt_to_text_clips, which is not wired to drop handling yet.
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
