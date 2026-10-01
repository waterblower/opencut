use crate::args::ExportArgs;
use crate::document;
use anyhow::Result;
use engine::export::ExportOption;
use serde_json::{Value, json};
use timeline::TimelineSerialization;

pub fn export_v1(args: ExportArgs) -> Result<Value> {
    let document = TimelineSerialization::load(&args.timeline)?;
    let project_root = match args.project_root {
        Some(root) => root,
        None => document::asset_base(&args.timeline)?,
    };
    engine::export::export(
        &document,
        &args.output,
        &ExportOption {
            project_root,
            video_bitrate: args.video_bitrate * 1_000,
            overwrite: args.overwrite,
        },
    )?;
    Ok(json!({"path": args.output, "frames": document.frame_count()}))
}

pub fn export_v2(args: ExportArgs) -> Result<Value> {
    let document = TimelineSerialization::load(&args.timeline)?;
    let project_root = match args.project_root {
        Some(root) => root,
        None => document::asset_base(&args.timeline)?,
    };
    engine::export_v2::export_v2(
        &document,
        &args.output,
        &ExportOption {
            project_root,
            video_bitrate: args.video_bitrate * 1_000,
            overwrite: args.overwrite,
        },
    )?;
    Ok(json!({"path": args.output, "frames": document.frame_count()}))
}
