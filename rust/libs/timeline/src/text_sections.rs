use crate::{Clip, TimelineEditingState, TimelineFrameIndex};
use anyhow::{Result, bail};
use ulid::Ulid;

/// Keeps text-covered intervals with breathing room and compacts all tracks without file I/O.
pub fn compact_text_sections(editing_state: &TimelineEditingState) -> Result<TimelineEditingState> {
    let sections = text_sections(editing_state)?;
    compact_to_sections(editing_state, &sections)
}

/// Text-covered intervals with breathing room, sorted and merged so they do not overlap.
fn text_sections(
    editing_state: &TimelineEditingState,
) -> Result<Vec<(TimelineFrameIndex, TimelineFrameIndex)>> {
    editing_state.validate()?;
    let frame_rate = editing_state.settings.frame_rate;
    let padding = frame_rate.nearest(0.025); // 字幕前后各保留约 25ms 原始内容，按时间线帧率取整。
    let content_end = editing_state.content_duration();
    let mut intervals = editing_state
        .clips
        .iter()
        .filter_map(|clip| match clip {
            Clip::Text(_) => Some((
                (clip.timeline_start() - padding).max(TimelineFrameIndex::ZERO),
                (clip.timeline_end(frame_rate) + padding).min(content_end),
            )),
            Clip::Video(_) => None,
            Clip::Audio(_) => None,
        })
        .collect::<Vec<_>>();
    intervals.sort_unstable();
    let mut sections: Vec<(TimelineFrameIndex, TimelineFrameIndex)> = Vec::new();
    for (start, end) in intervals {
        if end <= start {
            continue;
        }
        if let Some(last) = sections.last_mut() {
            if start <= last.1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        sections.push((start, end));
    }
    if sections.is_empty() {
        bail!("Timeline has no text sections to keep");
    }
    Ok(sections)
}

/// Keeps only `sections` (sorted, non-overlapping) on every track and closes the gaps between
/// them, without file I/O.
fn compact_to_sections(
    editing_state: &TimelineEditingState,
    sections: &[(TimelineFrameIndex, TimelineFrameIndex)],
) -> Result<TimelineEditingState> {
    let frame_rate = editing_state.settings.frame_rate;
    let mut compacted = editing_state.clone();
    compacted.clips.clear();
    for clip in &editing_state.clips {
        let mut output_start = TimelineFrameIndex::ZERO;
        let mut kept_original_id = false;
        for &(section_start, section_end) in sections {
            let start = clip.timeline_start().max(section_start);
            let end = clip.timeline_end(frame_rate).min(section_end);
            if start < end {
                let mut fragment = if kept_original_id {
                    clip.copy(Ulid::generate())
                } else {
                    clip.clone()
                };
                kept_original_id = true;
                fragment.set_timeline_start(output_start + start - section_start);
                match &mut fragment {
                    Clip::Video(media) => {
                        media.source_in += start - clip.timeline_start();
                        media.source_out = media.source_in + end - start;
                    }
                    Clip::Audio(media) => {
                        media.source_in += start - clip.timeline_start();
                        media.source_out = media.source_in + end - start;
                    }
                    Clip::Text(text) => text.duration = frame_rate.duration(end - start),
                }
                compacted.clips.push(fragment);
            }
            output_start += section_end - section_start;
        }
    }
    compacted.validate()?;
    Ok(compacted)
}
