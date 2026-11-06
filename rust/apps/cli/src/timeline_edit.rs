use crate::args::TimelineCommand;
use crate::document;
use anyhow::{Context as _, Result, ensure};
use serde_json::{Value, json};
use std::{fs, path::PathBuf};
use timeline::{TimelineFrameIndex, TimelineSerialization, compact_text_sections, remove_gaps};

pub(crate) fn edit(command: TimelineCommand) -> Result<Value> {
    let options = match &command {
        TimelineCommand::KeepTextSections(options) | TimelineCommand::RemoveGaps(options) => {
            options
        }
    };
    let input = std::path::absolute(&options.timeline)?;
    let input_directory =
        fs::canonicalize(input.parent().context("Input has no parent directory")?)?;
    let output = if options.write_inplace {
        fs::canonicalize(&input)?
    } else {
        std::path::absolute(
            options
                .output
                .as_ref()
                .context("An output path is required")?,
        )?
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
    let editing_state = &original.editing_state;
    let before_frames = editing_state.content_duration();
    let mut compacted = match &command {
        TimelineCommand::KeepTextSections(_) => compact_text_sections(editing_state)?,
        TimelineCommand::RemoveGaps(_) => remove_gaps(editing_state),
    };
    if input_directory != output_directory {
        for asset in &mut compacted.assets {
            if asset.path().is_absolute() {
                continue;
            }
            let absolute = input_directory.join(asset.path());
            let base = output_directory.components().collect::<Vec<_>>();
            let target = absolute.components().collect::<Vec<_>>();
            if base.first() != target.first() {
                asset.set_path(absolute);
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
            asset.set_path(relative);
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
    result.editing_state.validate()?;
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
