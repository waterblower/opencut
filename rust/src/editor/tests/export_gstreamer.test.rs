use crate::editor::tests::TimelineTestExt;
// Integration-style tests for the GStreamer exporter.

use super::*;

use super::super::{
    media_probe::probe_video,
    model::MediaAsset,
    tests::ulid,
    timeline::TimelineTime,
    timeline_clip::{
        AudioClipProperties, TextClip, TextClipProperties, VideoClip, VideoClipProperties,
    },
    track::Track,
};
use std::{path::Path, time::Duration};

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

    let mut project = TimelineSerialization::with_test_tracks();
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
        let asset_id = ulid(100 + index as u64 * 2);
        let clip_id = ulid(101 + index as u64 * 2);
        let mut asset = probe_video(source_path, asset_id).unwrap();
        asset.path = source_path.strip_prefix(&project_root).unwrap().into();
        let duration = project.ceil_time(asset.duration);
        project.assets.push(asset);
        project.clips.push(Clip::Video(VideoClip {
            id: clip_id,
            track_id: video_track,
            asset_id,
            timeline_start,
            source_in: TimelineTime::ZERO,
            source_out: duration,
            video_properties: VideoClipProperties::default(),
            audio_properties: AudioClipProperties::default(),
        }));
        timeline_start += duration;
    }

    assert_eq!(project.clips.len(), source_paths.len());
    assert_eq!(project.content_duration(), timeline_start);
    let expected_duration = project.seconds(timeline_start);

    let mut options = ExportOptions::from_timeline(&project);
    options.encoder = encoder;
    export_timeline(&project, &project_root, &output, options, |_| {}).unwrap();

    let exported = probe_video(&output, ulid(u64::MAX)).unwrap();
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
    let project = TimelineSerialization::with_test_tracks();
    let track = project
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
        .unwrap();
    let clip = Clip::Video(VideoClip {
        id: ulid(1),
        track_id: track.id,
        asset_id: ulid(2),
        timeline_start: TimelineTime::ZERO,
        source_in: TimelineTime::ZERO,
        source_out: TimelineTime::ONE_FRAME,
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    });
    let types = exported_track_types(track, &clip, MediaKind::Video, true);
    assert!(types.contains(ges::TrackType::VIDEO));
    assert!(types.contains(ges::TrackType::AUDIO));
}

#[test]
fn hidden_video_track_can_still_export_audio() {
    let mut project = TimelineSerialization::with_test_tracks();
    let track = project
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Video)
        .unwrap();
    track.visible = false;
    let clip = Clip::Video(VideoClip {
        id: ulid(1),
        track_id: track.id,
        asset_id: ulid(2),
        timeline_start: TimelineTime::ZERO,
        source_in: TimelineTime::ZERO,
        source_out: TimelineTime::ONE_FRAME,
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    });
    assert_eq!(
        exported_track_types(track, &clip, MediaKind::Video, true),
        ges::TrackType::AUDIO
    );
}

#[test]
fn applies_the_requested_bitrate_to_x264() {
    ges::init().unwrap();
    let pipeline = ges::Pipeline::new();
    configure_export_elements(&pipeline, 12_345_000);
    let encoder = gst::ElementFactory::make("x264enc").build().unwrap();
    pipeline.add(&encoder).unwrap();
    assert_eq!(encoder.property::<u32>("bitrate"), 12_345);
}

#[test]
fn configures_aac_encoders_for_export() {
    ges::init().unwrap();
    let pipeline = ges::Pipeline::new();
    configure_export_elements(&pipeline, 12_345_000);
    let audio_encoder = gst::ElementFactory::make("avenc_aac").build().unwrap();
    pipeline.add(&audio_encoder).unwrap();
    assert_eq!(audio_encoder.property::<i32>("bitrate"), AUDIO_BIT_RATE);

    #[cfg(target_os = "macos")]
    {
        let audio_encoder = gst::ElementFactory::make("atenc").build().unwrap();
        pipeline.add(&audio_encoder).unwrap();
        assert_eq!(
            audio_encoder.property::<u32>("bitrate"),
            AUDIO_BIT_RATE as u32
        );
    }
}

#[test]
fn selects_platform_aac_encoder_without_changing_audio_rank() {
    ges::init().unwrap();
    let expected = if cfg!(target_os = "macos") {
        "atenc"
    } else {
        "avenc_aac"
    };
    let audio_factory = gst::ElementFactory::find(expected).unwrap();
    let original_rank = audio_factory.rank();
    {
        let _selection = EncoderSelection::for_export(ExportEncoder::Software).unwrap();
        assert_eq!(audio_factory.rank(), original_rank);
        let timeline = TimelineSerialization::with_test_tracks();
        let profile = encoding_profile(ExportOptions::from_timeline(&timeline));
        let encodebin = gst::ElementFactory::make("encodebin")
            .property("profile", &profile)
            .build()
            .unwrap()
            .downcast::<gst::Bin>()
            .unwrap();
        encodebin.set_state(gst::State::Ready).unwrap();
        let mut audio_encoders = Vec::new();
        for element in encodebin.iterate_recurse() {
            let element = element.unwrap();
            let Some(factory) = element.factory() else {
                continue;
            };
            if matches!(
                factory.name().as_str(),
                "atenc" | "avenc_aac" | "faac" | "voaacenc"
            ) {
                audio_encoders.push(factory.name().to_string());
            }
        }
        encodebin.set_state(gst::State::Null).unwrap();
        assert_eq!(audio_encoders, vec![expected]);
    }
    assert_eq!(audio_factory.rank(), original_rank);
}

#[cfg(target_os = "macos")]
#[test]
fn configures_videotoolbox_for_mp4_timeline_export() {
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
    ges::init().unwrap();
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut project = TimelineSerialization::with_test_tracks();
    let video_track = project
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
        .unwrap()
        .id;
    project.assets.push(MediaAsset {
        id: ulid(10),
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
    };
    project.clips.push(Clip::Video(VideoClip {
        id: ulid(11),
        track_id: video_track,
        asset_id: ulid(10),
        timeline_start: TimelineTime::ZERO,
        source_in: TimelineTime::ZERO,
        source_out: TimelineTime::from_frames(3),
        video_properties,
        audio_properties: AudioClipProperties::default(),
    }));
    let timeline = build_ges_timeline(
        &project,
        project_root,
        ExportOptions::from_timeline(&project),
        false,
    )
    .unwrap();
    let layers = timeline.layers();
    assert_eq!(layers.len(), project.tracks.len() + 1);
    assert!(layers.last().unwrap().priority() > layers.first().unwrap().priority());
    let exported_clip = layers
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
        600
    );
    assert_eq!(
        exported_clip
            .child_property("posy")
            .unwrap()
            .get::<i32>()
            .unwrap(),
        210
    );
    assert_eq!(
        exported_clip
            .child_property("width")
            .unwrap()
            .get::<i32>()
            .unwrap(),
        960
    );
    assert_eq!(
        exported_clip
            .child_property("height")
            .unwrap()
            .get::<i32>()
            .unwrap(),
        540
    );
}

#[test]
fn adds_text_clips_as_independent_ges_titles() {
    ges::init().unwrap();
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut project = TimelineSerialization::with_test_tracks();
    let text_track_id = ulid(20);
    let text_clip_id = ulid(21);
    project.tracks.push(Track {
        id: text_track_id,
        name: "Text 1".into(),
        kind: TrackKind::Text,
        locked: false,
        muted: false,
        visible: true,
    });
    project.clips.push(Clip::Text(TextClip {
        id: text_clip_id,
        track_id: text_track_id,
        timeline_start: TimelineTime::from_frames(12),
        length: Duration::from_secs(2),
        properties: TextClipProperties {
            text: "GES text".into(),
            font_size: 72.0,
            color: 0x12_34_56_78,
            position_x: 0.25,
            position_y: 0.75,
            ..TextClipProperties::default()
        },
    }));

    let timeline = build_ges_timeline(
        &project,
        project_root,
        ExportOptions::from_timeline(&project),
        false,
    )
    .unwrap();
    let layers = timeline.layers();
    let overlay = layers
        .iter()
        .flat_map(|layer| layer.clips())
        .find(|clip| {
            clip.name().as_deref() == Some(format!("opencut-clip-{text_clip_id}").as_str())
        })
        .unwrap()
        .downcast::<ges::TitleClip>()
        .unwrap();

    assert_eq!(overlay.layer().unwrap().priority(), 0);
    assert_eq!(
        overlay
            .child_property("text")
            .unwrap()
            .get::<String>()
            .ok()
            .as_deref(),
        Some("GES text")
    );
    assert_eq!(
        overlay
            .child_property("font-desc")
            .unwrap()
            .get::<String>()
            .ok()
            .as_deref(),
        Some("Sans 72px")
    );
    assert_eq!(
        overlay
            .child_property("color")
            .unwrap()
            .get::<u32>()
            .unwrap(),
        0x12_34_56_78
    );
    assert_eq!(
        overlay
            .child_property("halignment")
            .unwrap()
            .transform::<i32>()
            .unwrap()
            .get::<i32>()
            .unwrap(),
        4
    );
    assert_eq!(
        overlay
            .child_property("valignment")
            .unwrap()
            .transform::<i32>()
            .unwrap()
            .get::<i32>()
            .unwrap(),
        3
    );
    assert_eq!(
        overlay
            .child_property("xpos")
            .unwrap()
            .get::<f64>()
            .unwrap(),
        0.25
    );
    assert_eq!(
        overlay
            .child_property("ypos")
            .unwrap()
            .get::<f64>()
            .unwrap(),
        0.75
    );
    assert_eq!(
        overlay.start(),
        gst::ClockTime::from_nseconds(
            clock_time(project.duration(TimelineTime::from_frames(12))).nseconds() - 1
        )
    );
    assert_eq!(
        overlay.duration(),
        gst::ClockTime::from_nseconds(2_000_000_001)
    );
}

#[test]
fn hidden_and_muted_tracks_keep_their_duration_as_black_video() {
    ges::init().unwrap();
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut project = TimelineSerialization::with_test_tracks();
    let track = project
        .tracks
        .iter_mut()
        .find(|track| track.kind == TrackKind::Video)
        .unwrap();
    track.visible = false;
    track.muted = true;
    let track_id = track.id;
    project.assets.push(MediaAsset {
        id: ulid(10),
        kind: MediaKind::Video,
        path: "hidden-video-does-not-need-to-exist.mp4".into(),
        name: "hidden video".into(),
        duration: 5.0,
        width: 320,
        height: 180,
        framerate: 30.0,
        frame_rate_numerator: 30,
        frame_rate_denominator: 1,
        codec: "h264".into(),
        has_audio: true,
    });
    project.clips.push(Clip::Video(VideoClip {
        id: ulid(11),
        track_id,
        asset_id: ulid(10),
        timeline_start: TimelineTime::from_frames(12),
        source_in: TimelineTime::ZERO,
        source_out: TimelineTime::from_frames(30),
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    }));

    let timeline = build_ges_timeline(
        &project,
        project_root,
        ExportOptions::from_timeline(&project),
        false,
    )
    .unwrap();
    let expected_duration = clock_time(project.duration(project.content_duration()));
    let background = timeline
        .layers()
        .into_iter()
        .flat_map(|layer| layer.clips())
        .find(|clip| clip.name().as_deref() == Some("opencut-black-background"))
        .unwrap()
        .downcast::<ges::TestClip>()
        .unwrap();

    assert_eq!(timeline.duration(), expected_duration);
    assert_eq!(background.duration(), expected_duration);
    assert_eq!(background.supported_formats(), ges::TrackType::VIDEO);
    assert_eq!(background.vpattern(), ges::VideoTestPattern::Black);
    assert!(background.is_muted());
}

#[test]
fn exports_real_media_with_audio() {
    let project_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut project = TimelineSerialization::with_test_tracks();
    project.settings.width = 320;
    project.settings.height = 180;
    let video_track = project
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
        .unwrap()
        .id;
    project.assets.push(MediaAsset {
        id: ulid(10),
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
    project.clips.push(Clip::Video(VideoClip {
        id: ulid(11),
        track_id: video_track,
        asset_id: ulid(10),
        timeline_start: TimelineTime::ZERO,
        source_in: TimelineTime::ZERO,
        source_out: TimelineTime::from_frames(30),
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    }));

    let output = std::env::temp_dir().join(format!("opencut-ges-video-{}.mp4", std::process::id()));
    export_timeline(
        &project,
        project_root,
        &output,
        ExportOptions::from_timeline(&project),
        |_| {},
    )
    .unwrap();
    assert!(std::fs::metadata(&output).unwrap().len() > 0);
    std::fs::remove_file(output).unwrap();
}

#[test]
fn exports_an_image_only_timeline() {
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

    let mut project = TimelineSerialization::with_test_tracks();
    project.settings.width = 64;
    project.settings.height = 64;
    let video_track = project
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
        .unwrap()
        .id;
    project.assets.push(MediaAsset {
        id: ulid(10),
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
    project.clips.push(Clip::Video(VideoClip {
        id: ulid(11),
        track_id: video_track,
        asset_id: ulid(10),
        timeline_start: TimelineTime::ZERO,
        source_in: TimelineTime::ZERO,
        source_out: TimelineTime::from_frames(3),
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    }));

    let output = project_root.join("image-export.mp4");
    export_timeline(
        &project,
        &project_root,
        &output,
        ExportOptions::from_timeline(&project),
        |_| {},
    )
    .unwrap();
    assert!(std::fs::metadata(&output).unwrap().len() > 0);
    std::fs::remove_dir_all(project_root).unwrap();
}
