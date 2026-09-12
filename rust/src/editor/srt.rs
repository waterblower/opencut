use anyhow::{Result, bail};
use std::path::Path;

use opencut_player::transcribe::SRT;
use ulid::Ulid;

use crate::editor::{FrameRate, TextClip, TextClipProperties};

pub fn write_srt(path: &Path, srt: &SRT) -> Result<()> {
    if !path.is_absolute() {
        bail!(
            "SRT output path must be absolute at {}:{}",
            file!(),
            line!()
        );
    }
    std::fs::write(path, srt.to_string())?;
    Ok(())
}

pub fn srt_text_clips(srt: &SRT, frame_rate: FrameRate) -> Vec<TextClip> {
    let mut clips = Vec::with_capacity(srt.subtitles.len());
    for subtitle in &srt.subtitles {
        let length = subtitle.end - subtitle.start;
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
    clips
}

#[cfg(test)]
#[path = "tests/srt.test.rs"]
mod tests;
