use crate::args::ExportArgs;
use crate::document;
use anyhow::Result;
use engine::export::ExportOption;
use serde_json::{Value, json};
use timeline::TimelineSerialization;

pub fn export(args: ExportArgs) -> Result<Value> {
    let document = TimelineSerialization::load(&args.timeline)?;
    let project_root = match args.project_root {
        Some(root) => root,
        None => document::asset_base(&args.timeline)?,
    };
    engine::export::export_timeline(
        &document,
        &args.output,
        &ExportOption {
            project_root,
            video_bitrate: args.video_bitrate * 1_000,
            overwrite: args.overwrite,
        },
        gpui_platform::current_platform(true).text_system(),
    )?;
    Ok(json!({"path": args.output, "frames": document.frame_count()}))
}
