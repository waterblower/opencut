//! End-to-end export benchmark for 100 half-second clips in source order.
#![cfg(target_os = "macos")]
use anyhow::{Context as _, Result, ensure};
use engine::export::{ExportCompletion, ExportControl, ExportOption, export_timeline};
use std::{fs, path::PathBuf, sync::Arc, time::Instant};
use timeline::TimelineSerialization;
use ulid::Ulid;

#[test]
fn export_source_ordered_clips() -> Result<()> {
    let document: TimelineSerialization =
        serde_json::from_str(include_str!("export_source_ordered_clips.timeline"))?;
    let content = document.to_editing_state();
    let frames = document.frame_count();
    ensure!(frames > 0, "Performance fixture must not be empty");
    let duration = content.settings.frame_rate.seconds(frames.into());
    // 此 fixture 只有视频，不需要字体；避免在测试线程创建 macOS AppKit platform。
    ensure!(
        content
            .clips
            .iter()
            .all(|clip| matches!(clip, timeline::Clip::Video(_))),
        "Source-ordered fixture must contain only video clips"
    );
    let text_system = Arc::new(gpui::NoopTextSystem::new());
    let output = std::env::temp_dir().join(format!("opencut-export-perf-{}.mp4", Ulid::generate()));
    let options = ExportOption {
        project_root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests"),
        video_bitrate: 8_000_000,
        overwrite: false,
    };
    println!(
        "Fixture: {}\n{} clips, {frames} frames, {duration:.3}s, {}x{}, 8 Mbps",
        "export_source_ordered_clips.timeline",
        content.clips.len(),
        content.settings.width,
        content.settings.height,
    );

    let control = ExportControl::default();
    let started = Instant::now();
    let completion = export_timeline(&document, &output, &options, text_system.clone(), &control)?;
    let elapsed = started.elapsed().as_secs_f64();
    ensure!(
        matches!(completion, ExportCompletion::Completed),
        "Export stopped unexpectedly"
    );
    ensure!(
        control.completed_frames() == frames,
        "Export did not process every frame"
    );
    ensure!(fs::metadata(&output)?.len() > 0, "Export output is empty");
    println!(
        "Time spend: {elapsed:.3}s, {:.2} fps, {:.3} ms/frame, {:.2}x realtime",
        frames as f64 / elapsed,
        elapsed * 1000.0 / frames as f64,
        duration / elapsed
    );
    fs::remove_file(&output)?;
    return Ok(());
}
