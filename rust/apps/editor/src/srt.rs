use crate::timeline::{FrameRate, TimelineFrameIndex};
use crate::timeline_clip::{TextClip, TextClipProperties};
use crate::track::TrackKind;
use ::timeline::TimelineEditingState;
use ::transcribe::{SRT, Subtitle};
use anyhow::{Result, bail};
use std::path::Path;
use ulid::Ulid;

pub(crate) fn write_srt(path: &Path, srt: &SRT) -> Result<()> {
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

pub(crate) fn srt_text_clips(srt: &SRT, frame_rate: FrameRate) -> Vec<TextClip> {
    let mut clips = Vec::with_capacity(srt.subtitles.len());
    for subtitle in &srt.subtitles {
        let length =
            (subtitle.end - subtitle.start).max(frame_rate.duration(TimelineFrameIndex::ONE_FRAME)); // 极短字幕不能取整为零帧。
        clips.push(TextClip::new(
            Ulid::generate(),
            Ulid::nil(),
            frame_rate.frames_from_duration_nearest(subtitle.start),
            length,
            TextClipProperties {
                text: subtitle.text.trim().to_string(),
                ..TextClipProperties::default()
            },
        ));
    }
    clips
}

pub(crate) fn text_track_to_srt(state: &TimelineEditingState, track_id: Ulid) -> Result<SRT> {
    let Some(track) = state.track(track_id) else {
        bail!("Track {track_id} does not exist");
    };
    if track.kind != TrackKind::Text {
        bail!("Track {} is not a text track", track.name);
    }
    let frame_rate = state.settings.frame_rate;
    let mut subtitles = Vec::new();
    for clip in state.clips_on_track(track_id) {
        let Some(text_clip) = clip.text() else {
            bail!(
                "Clip {} on text track {} is not a text clip",
                clip.id(),
                track.name
            );
        };
        let start = frame_rate.duration(text_clip.timeline_start);
        subtitles.push(Subtitle {
            text: text_clip.properties.text.clone(),
            start,
            end: start + text_clip.duration,
        });
    }
    subtitles.sort_by_key(|subtitle| subtitle.start);
    Ok(SRT { subtitles })
}

#[cfg(test)]
#[path = "tests/srt.test.rs"]
mod tests;
