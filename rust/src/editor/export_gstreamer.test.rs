//! Integration-style tests for the GStreamer exporter.

use super::*;
use crate::editor::model::{AudioClipProperties, TimelineTime, VideoClipProperties, probe_media};
use std::path::Path;

#[test]
fn exports_every_video_in_the_mini_fixture_as_one_sequence() {
    export_mini_fixture(ExportEncoder::Software, "assembled-export.mp4");
}

pub(super) fn export_mini_fixture(encoder: ExportEncoder, output_name: &str) {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("data/tests/mini测试");
    let output = project_root.join(output_name);
    let mut source_paths = std::fs::read_dir(&project_root)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| !name.starts_with('.') && !name.starts_with("assembled-export"))
        })
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("mp4"))
        })
        .collect::<Vec<_>>();
    source_paths.sort();
    assert!(!source_paths.is_empty(), "mini fixture has no videos");

    let mut project = Project::default();
    // The fixture mixes 480p and 720p inputs. A fixed Full HD output exercises
    // GES source transitions, scaling, encoding, and muxing.
    project.settings.width = 1920;
    project.settings.height = 1080;
    let video_track = project
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
        .unwrap()
        .id;
    let mut timeline_start = TimelineTime::ZERO;

    for (index, source_path) in source_paths.iter().enumerate() {
        let asset_id = 100 + index as u64 * 2;
        let clip_id = asset_id + 1;
        let mut asset = probe_media(source_path, asset_id).unwrap();
        asset.path = source_path.strip_prefix(&project_root).unwrap().into();
        let duration = project.ceil_time(asset.duration);
        project.assets.push(asset);
        project.clips.push(TimelineClip {
            id: clip_id,
            track_id: video_track,
            asset_id: Some(asset_id),
            timeline_start,
            source_in: TimelineTime::ZERO,
            source_out: duration,
            video_properties: VideoClipProperties::default(),
            audio_properties: AudioClipProperties::default(),
        });
        timeline_start += duration;
    }

    assert_eq!(project.clips.len(), source_paths.len());
    assert_eq!(project.content_duration(), timeline_start);
    let expected_duration = project.seconds(timeline_start);

    let mut options = ExportOptions::from_project(&project);
    options.encoder = encoder;
    export_project(&project, &project_root, &output, options, |_| {}).unwrap();

    let exported = probe_media(&output, u64::MAX).unwrap();
    assert_eq!(
        (exported.width, exported.height),
        (project.settings.width, project.settings.height)
    );
    assert!(
        (exported.duration - expected_duration).abs() <= 0.1,
        "expected a {expected_duration:.3}s sequence, got {:.3}s",
        exported.duration
    );
}
