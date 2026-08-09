//! Integration-style tests for the GStreamer exporter.

use super::*;
use crate::editor::{
    media_probe::probe_video,
    model::{AudioClipProperties, MediaAsset, TimelineTime, VideoClipProperties},
};
use std::path::Path;

#[test]
fn exports_every_video_in_the_mini_fixture_as_one_sequence() {
    export_mini_fixture(ExportEncoder::Software, "assembled-export.mp4");
}

#[cfg(target_os = "macos")]
#[test]
fn exports_videotoolbox() {
    gst::init().unwrap();
    assert!(
        gst::ElementFactory::find("vtenc_h264_hw").is_some(),
        "VideoToolbox export test requires the GStreamer vtenc_h264_hw element"
    );

    export_mini_fixture(ExportEncoder::Hardware, "assembled-export-videotoolbox.mp4");
}

pub(super) fn export_mini_fixture(encoder: ExportEncoder, output_name: &str) {
    let _gstreamer_test = crate::editor::lock_gstreamer_test();
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

    let mut project = Project::with_test_tracks();
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
        let mut asset = probe_video(source_path, asset_id).unwrap();
        asset.path = source_path.strip_prefix(&project_root).unwrap().into();
        let duration = project.ceil_time(asset.duration);
        project.assets.push(asset);
        project.clips.push(TimelineClip {
            id: clip_id,
            track_id: video_track,
            asset_id,
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

    let exported = probe_video(&output, u64::MAX).unwrap();
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

#[test]
fn video_track_exports_visible_video_and_unmuted_audio() {
    let project = Project::with_test_tracks();
    let track = project
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
        .unwrap();
    let clip = TimelineClip {
        id: 1,
        track_id: track.id,
        asset_id: 2,
        timeline_start: TimelineTime::ZERO,
        source_in: TimelineTime::ZERO,
        source_out: TimelineTime::ONE_FRAME,
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    };
    let types = exported_track_types(track, &clip, MediaKind::Video, true);
    assert!(types.contains(ges::TrackType::VIDEO));
    assert!(types.contains(ges::TrackType::AUDIO));
}

#[test]
fn hidden_video_track_can_still_export_audio() {
    let mut project = Project::with_test_tracks();
    let track = project
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Video)
        .unwrap();
    track.visible = false;
    let clip = TimelineClip {
        id: 1,
        track_id: track.id,
        asset_id: 2,
        timeline_start: TimelineTime::ZERO,
        source_in: TimelineTime::ZERO,
        source_out: TimelineTime::ONE_FRAME,
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    };
    assert_eq!(
        exported_track_types(track, &clip, MediaKind::Video, true),
        ges::TrackType::AUDIO
    );
}

#[test]
fn applies_the_requested_bitrate_to_x264() {
    let _gstreamer_test = crate::editor::lock_gstreamer_test();
    ges::init().unwrap();
    let pipeline = ges::Pipeline::new();
    configure_export_elements(&pipeline, 12_345_000);
    let encoder = gst::ElementFactory::make("x264enc").build().unwrap();
    pipeline.add(&encoder).unwrap();
    assert_eq!(encoder.property::<u32>("bitrate"), 12_345);
    let audio_encoder = gst::ElementFactory::make("faac").build().unwrap();
    pipeline.add(&audio_encoder).unwrap();
    assert_eq!(audio_encoder.property::<i32>("bitrate"), AUDIO_BIT_RATE);
}

#[cfg(target_os = "macos")]
#[test]
fn configures_videotoolbox_for_mp4_timeline_export() {
    let _gstreamer_test = crate::editor::lock_gstreamer_test();
    ges::init().unwrap();
    let Some(factory) = gst::ElementFactory::find("vtenc_h264_hw") else {
        return;
    };
    let pipeline = ges::Pipeline::new();
    configure_export_elements(&pipeline, 12_345_000);
    let encoder = factory.create().build().unwrap();
    pipeline.add(&encoder).unwrap();
    assert_eq!(encoder.property::<u32>("bitrate"), 12_345);
    assert!(!encoder.property::<bool>("allow-frame-reordering"));
}

#[test]
fn enables_automatic_threading_for_video_conversion_and_scaling() {
    let _gstreamer_test = crate::editor::lock_gstreamer_test();
    ges::init().unwrap();
    let pipeline = ges::Pipeline::new();
    configure_export_elements(&pipeline, 12_345_000);

    for factory_name in ["videoconvert", "videoscale", "videoconvertscale"] {
        let element = gst::ElementFactory::make(factory_name).build().unwrap();
        pipeline.add(&element).unwrap();
        assert_eq!(
            element.property::<u32>("n-threads"),
            0,
            "{factory_name} should choose its thread count automatically"
        );
    }
}

#[test]
fn creates_gstreamer_timeline_from_real_media() {
    let _gstreamer_test = crate::editor::lock_gstreamer_test();
    ges::init().unwrap();
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut project = Project::with_test_tracks();
    let video_track = project
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
        .unwrap()
        .id;
    project.assets.push(MediaAsset {
        id: 10,
        kind: MediaKind::Video,
        path: "data/tests/mini测试/地铁-出站-mini-480.mp4".into(),
        name: "test1".into(),
        duration: 5.0,
        width: 320,
        height: 180,
        framerate: 30.0,
        frame_rate_numerator: 30,
        frame_rate_denominator: 1,
        codec: "h264".into(),
        has_audio: true,
    });
    let video_properties = VideoClipProperties {
        position_x: 120.0,
        position_y: -60.0,
        scale: 0.5,
        opacity: 0.25,
        crop_left: 0.1,
        crop_right: 0.2,
        crop_top: 0.1,
        crop_bottom: 0.2,
    };
    project.clips.push(TimelineClip {
        id: 11,
        track_id: video_track,
        asset_id: 10,
        timeline_start: TimelineTime::ZERO,
        source_in: TimelineTime::ZERO,
        source_out: TimelineTime::from_frames(3),
        video_properties,
        audio_properties: AudioClipProperties::default(),
    });
    let timeline = build_timeline(
        &project,
        project_root,
        ExportOptions::from_project(&project),
    )
    .unwrap();
    assert_eq!(timeline.layers().len(), project.tracks.len());
    let exported_clip = timeline
        .layers()
        .into_iter()
        .flat_map(|layer| layer.clips())
        .next()
        .unwrap();
    assert_eq!(
        exported_clip
            .child_property("posx")
            .unwrap()
            .get::<i32>()
            .unwrap(),
        696
    );
    assert_eq!(
        exported_clip
            .child_property("posy")
            .unwrap()
            .get::<i32>()
            .unwrap(),
        264
    );
    assert_eq!(
        exported_clip
            .child_property("width")
            .unwrap()
            .get::<i32>()
            .unwrap(),
        672
    );
    assert_eq!(
        exported_clip
            .child_property("height")
            .unwrap()
            .get::<i32>()
            .unwrap(),
        378
    );
    assert_eq!(
        exported_clip
            .child_property("alpha")
            .unwrap()
            .get::<f64>()
            .unwrap(),
        0.25
    );
    for (name, expected) in [("left", 32), ("right", 64), ("top", 18), ("bottom", 36)] {
        assert_eq!(
            exported_clip
                .child_property(name)
                .unwrap()
                .get::<i32>()
                .unwrap(),
            expected
        );
    }
}

#[test]
fn exports_real_media_with_audio() {
    let _gstreamer_test = crate::editor::lock_gstreamer_test();
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut project = Project::with_test_tracks();
    project.settings.width = 320;
    project.settings.height = 180;
    let video_track = project
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
        .unwrap()
        .id;
    project.assets.push(MediaAsset {
        id: 10,
        kind: MediaKind::Video,
        path: "data/tests/mini测试/地铁-出站-mini-480.mp4".into(),
        name: "test1".into(),
        duration: 5.0,
        width: 320,
        height: 180,
        framerate: 30.0,
        frame_rate_numerator: 30,
        frame_rate_denominator: 1,
        codec: "h264".into(),
        has_audio: true,
    });
    project.clips.push(TimelineClip {
        id: 11,
        track_id: video_track,
        asset_id: 10,
        timeline_start: TimelineTime::ZERO,
        source_in: TimelineTime::ZERO,
        source_out: TimelineTime::from_frames(30),
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    });

    let output = std::env::temp_dir().join(format!("opencut-ges-video-{}.mp4", std::process::id()));
    export_project(
        &project,
        project_root,
        &output,
        ExportOptions::from_project(&project),
        |_| {},
    )
    .unwrap();
    assert!(std::fs::metadata(&output).unwrap().len() > 0);
    std::fs::remove_file(output).unwrap();
}

#[test]
fn exports_an_image_only_timeline() {
    let _gstreamer_test = crate::editor::lock_gstreamer_test();
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let project_root = std::env::temp_dir().join(format!("opencut-ges-image-{unique}"));
    std::fs::create_dir_all(&project_root).unwrap();
    let image_path = project_root.join("still.png");
    image::save_buffer(
        &image_path,
        &[0x20; 64 * 64 * 4],
        64,
        64,
        image::ColorType::Rgba8,
    )
    .unwrap();

    let mut project = Project::with_test_tracks();
    project.settings.width = 64;
    project.settings.height = 64;
    let video_track = project
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
        .unwrap()
        .id;
    project.assets.push(MediaAsset {
        id: 10,
        kind: MediaKind::Image,
        path: "still.png".into(),
        name: "still".into(),
        duration: 5.0,
        width: 64,
        height: 64,
        framerate: 0.0,
        frame_rate_numerator: 0,
        frame_rate_denominator: 0,
        codec: "png".into(),
        has_audio: false,
    });
    project.clips.push(TimelineClip {
        id: 11,
        track_id: video_track,
        asset_id: 10,
        timeline_start: TimelineTime::ZERO,
        source_in: TimelineTime::ZERO,
        source_out: TimelineTime::from_frames(3),
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    });

    let output = project_root.join("image-export.mp4");
    export_project(
        &project,
        &project_root,
        &output,
        ExportOptions::from_project(&project),
        |_| {},
    )
    .unwrap();
    assert!(std::fs::metadata(&output).unwrap().len() > 0);
    std::fs::remove_dir_all(project_root).unwrap();
}
