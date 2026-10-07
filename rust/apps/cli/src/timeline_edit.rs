use crate::args::KeepTextSectionsArgs;
use crate::document;
use anyhow::{Context as _, Result, bail, ensure};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
use timeline::{Clip, TimelineEditingState, TimelineFrameIndex, TimelineSerialization};
use ulid::Ulid;

pub fn keep_text_sections(options: KeepTextSectionsArgs) -> Result<Value> {
    let input = std::path::absolute(&options.timeline)?;
    let input_directory =
        fs::canonicalize(input.parent().context("Input has no parent directory")?)?;
    let output = if options.write_inplace {
        fs::canonicalize(&input)?
    } else {
        std::path::absolute(options.output.context("An output path is required")?)?
    };
    if !options.write_inplace {
        ensure!(
            !output.try_exists()?,
            "Output already exists: {}",
            output.display()
        );
    }
    let output_directory =
        fs::canonicalize(output.parent().context("Output has no parent directory")?)?;
    let original = TimelineSerialization::load(&input)?;
    let editing_state = original.to_editing_state();
    let before_frames = editing_state.content_duration();
    let mut compacted = compact_text_sections(&editing_state)?;
    if input_directory != output_directory {
        for asset in &mut compacted.assets {
            if asset.path.is_absolute() {
                continue;
            }
            let absolute = input_directory.join(&asset.path);
            let base = output_directory.components().collect::<Vec<_>>();
            let target = absolute.components().collect::<Vec<_>>();
            if base.first() != target.first() {
                asset.path = absolute;
                continue;
            }
            let shared = base
                .iter()
                .zip(&target)
                .take_while(|(left, right)| left == right)
                .count();
            let mut relative = PathBuf::new();
            for _ in shared..base.len() {
                relative.push("..");
            }
            for component in &target[shared..] {
                relative.push(component);
            }
            asset.path = relative;
        }
    }
    let after_frames = compacted.content_duration();
    let mut result = TimelineSerialization::from_editing_state(&compacted);
    result.set_view_state(
        TimelineFrameIndex::ZERO,
        (0.0, original.scroll_offset().1),
        original.pixels_per_second(),
        original.snapping_enabled(),
        original.track_magnet_enabled(),
    );
    result.to_editing_state().validate()?;
    document::write_atomic(
        &output,
        &serde_json::to_value(&result)?,
        options.write_inplace,
    )?;
    Ok(json!({
        "path": output,
        "before_frames": i64::from(before_frames),
        "after_frames": i64::from(after_frames),
        "removed_frames": i64::from(before_frames - after_frames),
    }))
}

/// Keeps text-covered intervals with breathing room and compacts all tracks without file I/O.
fn compact_text_sections(editing_state: &TimelineEditingState) -> Result<TimelineEditingState> {
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
            Clip::Video(_) | Clip::Audio(_) => None,
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
    let mut compacted = editing_state.clone();
    compacted.clips.clear();
    for clip in &editing_state.clips {
        let mut output_start = TimelineFrameIndex::ZERO;
        let mut kept_original_id = false;
        for &(section_start, section_end) in &sections {
            let start = clip.timeline_start().max(section_start);
            let end = clip.timeline_end(frame_rate).min(section_end);
            if start < end {
                let mut fragment = clip.clone();
                if kept_original_id {
                    fragment.set_id(Ulid::generate());
                }
                kept_original_id = true;
                fragment.set_timeline_start(output_start + start - section_start);
                match &mut fragment {
                    Clip::Video(media) | Clip::Audio(media) => {
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
