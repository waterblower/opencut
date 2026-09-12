use std::path::Path;

use anyhow::{Result, bail};
use opencut_player::transcribe::SRT;
use ulid::Ulid;

use crate::editor::{FrameRate, TextClip, TextClipProperties};

pub fn write_srt(path: &Path, srt: &SRT) -> Result<()> {
    if !path.is_absolute() {
        anyhow::bail!(
            "SRT output path must be absolute at {}:{}",
            file!(),
            line!()
        );
    }
    std::fs::write(path, srt.to_string())?;
    Ok(())
}

pub fn parse_srt_text_clips(contents: &str, frame_rate: FrameRate) -> Result<Vec<TextClip>> {
    let srt = SRT::from_string(contents)?;
    let mut clips = Vec::with_capacity(srt.subtitles.len());
    for (index, subtitle) in srt.subtitles.into_iter().enumerate() {
        let length = subtitle.end - subtitle.start;
        if length.is_zero() {
            bail!(
                "SRT cue {} has zero duration at {}:{}",
                index + 1,
                file!(),
                line!()
            );
        }
        clips.push(TextClip {
            id: Ulid::generate(),
            track_id: Ulid::nil(),
            timeline_start: frame_rate.frames_from_duration_nearest(subtitle.start),
            length,
            properties: TextClipProperties {
                text: subtitle.text.trim().to_string(),
                ..TextClipProperties::default()
            },
        });
    }
    Ok(clips)
}

#[cfg(test)]
#[path = "tests/srt.test.rs"]
mod tests;
