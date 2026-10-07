use super::*;
use crate::layout::DEFAULT_TIMELINE_PIXELS_PER_SECOND;
use crate::model::{DEFAULT_IMAGE_CLIP_DURATION, MediaAsset, MediaKind};
use crate::test_support::{TimelineTestExt, ulid};
use crate::timeline_clip::{
    AudioClipProperties, Clip, ClipEditingExt, TextClip, TextClipProperties, VideoClip,
    VideoClipProperties,
};
use crate::track::{Track, TrackKind};
use ::timeline::{TimelineEditingState, TimelineSerialization, TimelineSettings};
use std::collections::HashSet;
use std::time::Duration;

#[test]
fn timeline_view_state_is_sanitized_at_the_persistence_boundary() {
    let mut document = TimelineSerialization::default();
    document.set_view_state(
        TimelineFrameIndex::from(-10),
        (f32::NAN, -20.0),
        f32::NAN,
        false,
        false,
    );
    assert_eq!(document.playhead(), TimelineFrameIndex::ZERO);
    assert_eq!(document.scroll_offset(), (0.0, 0.0));
    assert_eq!(
        document.pixels_per_second(),
        DEFAULT_TIMELINE_PIXELS_PER_SECOND
    );
    assert!(!document.snapping_enabled() && !document.track_magnet_enabled());
}

#[test]
fn missing_timeline_view_fields_use_defaults() {
    let document = serde_json::from_str::<TimelineSerialization>(
        r#"{"editing_state": {}, "view_state": {"horizontal_scroll":20.0}}"#,
    )
    .unwrap();
    assert!(document.snapping_enabled() && document.track_magnet_enabled());
    assert_eq!(document.scroll_offset(), (20.0, 0.0));
    assert_eq!(document.playhead(), TimelineFrameIndex::ZERO);
    assert_eq!(
        document.pixels_per_second(),
        DEFAULT_TIMELINE_PIXELS_PER_SECOND
    );
}

#[test]
fn timeline_view_zoom_round_trips_through_timeline_json() {
    let mut document = TimelineSerialization::default();
    document.set_view_state(TimelineFrameIndex::ZERO, (0.0, 0.0), 144.0, true, true);
    let restored =
        serde_json::from_str::<TimelineSerialization>(&serde_json::to_string(&document).unwrap())
            .unwrap();
    assert_eq!(restored.pixels_per_second(), 144.0);
}

#[cfg(test)]
impl TimelineTestExt for TimelineEditingState {
    fn with_test_tracks() -> Self {
        Self {
            tracks: vec![
                Track {
                    id: ulid(1),
                    name: "Video 1".into(),
                    kind: TrackKind::Video,
                    locked: false,
                    muted: false,
                    visible: true,
                },
                Track {
                    id: ulid(2),
                    name: "Audio 1".into(),
                    kind: TrackKind::Audio,
                    locked: false,
                    muted: false,
                    visible: true,
                },
            ],
            ..Self::default()
        }
    }
}

fn video_clip(id: u64, start: i64, duration: i64) -> Clip {
    Clip::Video(VideoClip::new(
        ulid(id),
        ulid(1),
        ulid(100),
        TimelineFrameIndex::from(start),
        TimelineFrameIndex::ZERO,
        TimelineFrameIndex::from(duration),
        VideoClipProperties::default(),
        AudioClipProperties::default(),
    ))
}

fn video_asset() -> MediaAsset {
    MediaAsset {
        id: ulid(100),
        kind: MediaKind::Video,
        path: "clip.mp4".into(),
        name: "clip".into(),
        duration: 30.0,
        width: 1920,
        height: 1080,
        framerate: 30.0,
        frame_rate_numerator: 30,
        frame_rate_denominator: 1,
        codec: "h264".into(),
        has_audio: true,
    }
}

fn image_asset() -> MediaAsset {
    MediaAsset {
        id: ulid(100),
        kind: MediaKind::Image,
        path: "still.png".into(),
        name: "still".into(),
        duration: DEFAULT_IMAGE_CLIP_DURATION,
        width: 1920,
        height: 1080,
        framerate: 0.0,
        frame_rate_numerator: 0,
        frame_rate_denominator: 0,
        codec: "PNG".into(),
        has_audio: false,
    }
}

#[test]
fn new_timelines_have_no_tracks() {
    assert!(TimelineEditingState::default().tracks.is_empty());
}

#[test]
fn frame_rate_labels_use_presets_and_format_custom_rates() {
    assert_eq!(FrameRate::new(24_000, 1_001).label(), "23.976 fps");
    assert_eq!(FrameRate::new(15, 1).label(), "15 fps");
    assert_eq!(FrameRate::new(31, 2).label(), "15.50 fps");
    assert_eq!(FrameRate::new(2_469, 200).label(), "12.35 fps");
}

#[test]
fn repairs_overlapping_clips_when_loading_a_timeline() {
    let mut project = TimelineEditingState {
        assets: vec![video_asset()],
        clips: vec![video_clip(10, 0, 150), video_clip(11, 90, 120)],
        ..TimelineEditingState::with_test_tracks()
    };

    project.repair_and_prune_invalid_data();

    assert_eq!(
        project.clips[0].timeline_start(),
        TimelineFrameIndex::from(0)
    );
    assert_eq!(
        project.clips[1].timeline_start(),
        TimelineFrameIndex::from(150)
    );
}

#[test]
fn still_image_clips_can_extend_beyond_their_default_duration() {
    let mut project = TimelineEditingState {
        assets: vec![image_asset()],
        clips: vec![video_clip(10, 0, 300)],
        ..TimelineEditingState::with_test_tracks()
    };

    project.repair_and_prune_invalid_data();

    assert_eq!(
        project.clips[0].frame_length(project.settings.frame_rate),
        TimelineFrameIndex::from(300)
    );
    assert_eq!(
        project.seconds(project.clips[0].frame_length(project.settings.frame_rate)),
        10.0
    );
}

#[test]
fn time_based_media_remains_bounded_by_its_source_duration() {
    let mut project = TimelineEditingState {
        assets: vec![video_asset()],
        clips: vec![video_clip(10, 0, 1_200)],
        ..TimelineEditingState::with_test_tracks()
    };

    project.repair_and_prune_invalid_data();

    assert_eq!(
        project.clips[0].frame_length(project.settings.frame_rate),
        TimelineFrameIndex::from(900)
    );
}

#[test]
fn assetless_text_clips_survive_timeline_repair() {
    let track_id = ulid(3);
    let mut project = TimelineEditingState {
        tracks: vec![Track {
            id: track_id,
            name: "Text 1".into(),
            kind: TrackKind::Text,
            locked: false,
            muted: false,
            visible: true,
        }],
        clips: vec![Clip::Text(TextClip::new(
            ulid(10),
            track_id,
            TimelineFrameIndex::ZERO,
            FrameRate::default().duration(TimelineFrameIndex::from(150)),
            TextClipProperties::default(),
        ))],
        ..TimelineEditingState::default()
    };

    project.repair_and_prune_invalid_data();

    assert_eq!(project.clips.len(), 1);
}

#[test]
fn text_clips_can_move_without_a_media_asset() {
    let track_id = ulid(3);
    let clip_id = ulid(10);
    let project = TimelineEditingState {
        tracks: vec![Track {
            id: track_id,
            name: "Text 1".into(),
            kind: TrackKind::Text,
            locked: false,
            muted: false,
            visible: true,
        }],
        clips: vec![Clip::Text(TextClip::new(
            clip_id,
            track_id,
            TimelineFrameIndex::ZERO,
            FrameRate::default().duration(TimelineFrameIndex::from(150)),
            TextClipProperties::default(),
        ))],
        ..TimelineEditingState::default()
    };

    assert!(
        project
            .validate_clip_move_placements(
                &[(clip_id, track_id, TimelineFrameIndex::from(30))],
                &HashSet::from([clip_id]),
            )
            .is_ok()
    );
}

#[test]
fn validates_a_thousand_clip_moves_within_one_frame_budget() {
    use std::hint::black_box;
    use std::time::Instant;

    let mut project = TimelineEditingState {
        assets: vec![video_asset()],
        ..TimelineEditingState::with_test_tracks()
    };
    let mut placements = Vec::new();
    let mut ignored = HashSet::new();
    for index in 0..1_000_u64 {
        let selected = video_clip(1_000 + index, index as i64 * 120, 30);
        ignored.insert(selected.id());
        placements.push((
            selected.id(),
            ulid(1),
            selected.timeline_start() + TimelineFrameIndex::from(10),
        ));
        project.clips.push(selected);
        project
            .clips
            .push(video_clip(2_000 + index, index as i64 * 120 + 60, 30));
    }
    // 无序输入，且目标轨道含未选中片段，避免只测全选或已排序的捷径。
    project.clips.reverse();
    placements.reverse();
    project
        .validate_clip_move_placements(&placements, &ignored)
        .unwrap();

    let mut timings = Vec::new();
    for _ in 0..31 {
        let started = Instant::now();
        black_box(&project)
            .validate_clip_move_placements(black_box(&placements), black_box(&ignored))
            .unwrap();
        timings.push(started.elapsed());
    }
    timings.sort_unstable();
    let median = timings[timings.len() / 2];
    eprintln!(
        "1,000 selected / 2,000 total clips, 31 runs: median={median:?}, min={:?}, max={:?}",
        timings[0],
        timings[timings.len() - 1]
    );
    assert!(
        median < Duration::from_micros(16_667),
        "Move validation exceeded a 60 Hz frame budget: {median:?}"
    );

    placements[0].2 += TimelineFrameIndex::from(50);
    let error = project
        .validate_clip_move_placements(&placements, &ignored)
        .unwrap_err();
    assert_eq!(
        error.downcast_ref::<ClipPlacementRejection>(),
        Some(&ClipPlacementRejection::ExistingClipOverlap)
    );
    placements[0] = (placements[0].0, placements[1].1, placements[1].2);
    let overlap_error = project
        .validate_clip_move_placements(&placements, &ignored)
        .unwrap_err();
    assert_eq!(
        overlap_error.downcast_ref::<ClipPlacementRejection>(),
        Some(&ClipPlacementRejection::ProposedClipsOverlap)
    );
}

#[test]
fn changing_frame_rate_keeps_text_duration_and_recomputes_frame_length() {
    let track_id = ulid(3);
    let mut project = TimelineEditingState {
        settings: TimelineSettings {
            frame_rate: FrameRate::new(30, 1),
            ..TimelineSettings::default()
        },
        tracks: vec![Track {
            id: track_id,
            name: "Text 1".into(),
            kind: TrackKind::Text,
            locked: false,
            muted: false,
            visible: true,
        }],
        clips: vec![Clip::Text(TextClip::new(
            ulid(10),
            track_id,
            TimelineFrameIndex::ZERO,
            Duration::from_secs(5),
            TextClipProperties::default(),
        ))],
        ..TimelineEditingState::default()
    };

    project.set_frame_rate(FrameRate::new(24, 1));

    let text = project.clips[0].text().unwrap();
    assert_eq!(text.duration, Duration::from_secs(5));
    assert_eq!(
        text.frame_length(project.settings.frame_rate),
        TimelineFrameIndex::from(120)
    );
}

#[test]
fn fractional_frame_rates_round_trip_without_drift() {
    for frame_rate in [
        FrameRate {
            numerator: 24_000,
            denominator: 1_001,
        },
        FrameRate {
            numerator: 30_000,
            denominator: 1_001,
        },
        FrameRate {
            numerator: 60_000,
            denominator: 1_001,
        },
    ] {
        let original = TimelineFrameIndex::from(1_000_003);
        let seconds = frame_rate.seconds(original);
        assert_eq!(frame_rate.nearest(seconds), original);
    }
}

#[test]
fn frame_boundaries_round_trip_through_duration() {
    for frame_rate in [
        FrameRate::new(24, 1),
        FrameRate::new(30, 1),
        FrameRate::new(60, 1),
        FrameRate::new(24_000, 1_001),
        FrameRate::new(30_000, 1_001),
        FrameRate::new(60_000, 1_001),
    ] {
        for frame in 0..10_000 {
            let time = TimelineFrameIndex::from(frame);
            assert_eq!(
                frame_rate.frames_from_duration_nearest(frame_rate.duration(time)),
                time,
                "frame {frame} did not round-trip at {frame_rate:?}",
            );
        }
    }
}

#[test]
fn durations_convert_to_the_nearest_frame() {
    let frame_rate = FrameRate::new(30, 1);
    for (nanoseconds, expected_frame) in [
        (0, 0),
        (16_666_666, 0),
        (16_666_667, 1),
        (33_333_333, 1),
        (49_999_999, 1),
        (50_000_000, 2),
    ] {
        assert_eq!(
            frame_rate.frames_from_duration_nearest(Duration::from_nanos(nanoseconds)),
            TimelineFrameIndex::from(expected_frame),
        );
    }
}

#[test]
fn repeated_frame_splits_preserve_the_total_duration() {
    let mut remaining = video_clip(10, 0, 10_000);
    let frame_rate = FrameRate::default();
    let original_duration = remaining.frame_length(frame_rate);
    let mut pieces = Vec::new();
    for split in [1, 17, 301, 999, 2_048] {
        let position = remaining.timeline_start() + TimelineFrameIndex::from(split);
        let (left, right) = remaining.split_at(position, frame_rate).unwrap();
        pieces.push(left.frame_length(frame_rate));
        remaining = right;
    }
    let reconstructed = pieces
        .into_iter()
        .fold(remaining.frame_length(frame_rate), |duration, piece| {
            duration + piece
        });
    assert_eq!(reconstructed, original_duration);
}

#[test]
fn long_timeline_duration_uses_exact_frame_counts() {
    let frame_rate = FrameRate {
        numerator: 30_000,
        denominator: 1_001,
    };
    let ten_hours = frame_rate.nearest(10.0 * 60.0 * 60.0);
    assert_eq!(frame_rate.nearest(frame_rate.seconds(ten_hours)), ten_hours);
    assert_eq!(
        frame_rate.frames_from_duration_nearest(frame_rate.duration(ten_hours)),
        ten_hours
    );
}

#[test]
fn preview_and_export_boundaries_share_the_same_frame_time() {
    let project = TimelineEditingState {
        settings: TimelineSettings {
            frame_rate: FrameRate {
                numerator: 24_000,
                denominator: 1_001,
            },
            ..TimelineSettings::default()
        },
        ..TimelineEditingState::default()
    };
    let boundary = TimelineFrameIndex::from(98_765);
    let preview_duration = project.position_at_frame(boundary).as_secs_f64();
    let export_seconds = project.seconds(boundary);
    assert!((preview_duration - export_seconds).abs() <= 1.0e-9);
}

#[test]
fn timeline_frames_map_to_exact_audio_samples() {
    let frame_rate = FrameRate {
        numerator: 30_000,
        denominator: 1_001,
    };
    assert_eq!(
        frame_rate.audio_samples(TimelineFrameIndex::from(30_000), 48_000),
        48_048_000
    );
}

#[test]
fn maps_30_fps_source_frames_onto_a_24_fps_timeline() {
    let project = TimelineEditingState {
        settings: TimelineSettings {
            frame_rate: FrameRate::new(24, 1),
            ..TimelineSettings::default()
        },
        assets: vec![video_asset()],
        clips: vec![video_clip(10, 0, 24)],
        ..TimelineEditingState::default()
    };
    let clip = &project.clips[0];
    let mapped = (0..=8)
        .map(|frame| {
            project
                .source_frame_at(clip, TimelineFrameIndex::from(frame))
                .unwrap()
        })
        .collect::<Vec<_>>();

    assert_eq!(mapped, vec![0, 1, 2, 3, 5, 6, 7, 8, 10]);
}

#[test]
fn changing_timeline_rate_preserves_elapsed_edit_times() {
    let mut project = TimelineEditingState {
        assets: vec![video_asset()],
        clips: vec![video_clip(10, 30, 300)],
        ..TimelineEditingState::with_test_tracks()
    };

    project.set_frame_rate(FrameRate::new(24, 1));

    assert_eq!(
        project.clips[0].timeline_start(),
        TimelineFrameIndex::from(24)
    );
    assert_eq!(
        project.clips[0].frame_length(project.settings.frame_rate),
        TimelineFrameIndex::from(240)
    );
}

#[test]
fn clip_source_time_clamps_to_its_source_range() {
    let mut clip = video_clip(10, 100, 60);
    clip.video_mut().unwrap().source_in = TimelineFrameIndex::from(30);
    clip.video_mut().unwrap().source_out = TimelineFrameIndex::from(90);

    assert_eq!(
        clip.source_time_at(TimelineFrameIndex::from(50)),
        Some(TimelineFrameIndex::from(30))
    );
    assert_eq!(
        clip.source_time_at(TimelineFrameIndex::from(100)),
        Some(TimelineFrameIndex::from(30))
    );
    assert_eq!(
        clip.source_time_at(TimelineFrameIndex::from(125)),
        Some(TimelineFrameIndex::from(55))
    );
    assert_eq!(
        clip.source_time_at(TimelineFrameIndex::from(160)),
        Some(TimelineFrameIndex::from(90))
    );
    assert_eq!(
        clip.source_time_at(TimelineFrameIndex::from(200)),
        Some(TimelineFrameIndex::from(90))
    );
}

#[test]
fn splitting_clip_preserves_ranges_and_properties() {
    let mut clip = video_clip(10, 100, 60);
    let media = clip.video_mut().unwrap();
    media.source_in = TimelineFrameIndex::from(30);
    media.source_out = TimelineFrameIndex::from(90);
    media.video_properties.position_x = 42.0;
    media.audio_properties.gain_db = -6.0;
    media.audio_properties.muted = true;

    let (left, right) = clip
        .split_at(TimelineFrameIndex::from(125), FrameRate::default())
        .unwrap();

    assert_eq!(left.id(), ulid(10));
    assert_eq!(left.timeline_start(), TimelineFrameIndex::from(100));
    assert_eq!(left.source_in().unwrap(), TimelineFrameIndex::from(30));
    assert_eq!(left.source_out().unwrap(), TimelineFrameIndex::from(55));
    assert_ne!(right.id(), clip.id());
    assert_eq!(right.timeline_start(), TimelineFrameIndex::from(125));
    assert_eq!(right.source_in().unwrap(), TimelineFrameIndex::from(55));
    assert_eq!(right.source_out().unwrap(), TimelineFrameIndex::from(90));
    assert_eq!(
        left.video().unwrap().video_properties,
        clip.video().unwrap().video_properties
    );
    assert_eq!(
        right.video().unwrap().video_properties,
        clip.video().unwrap().video_properties
    );
    assert_eq!(
        left.video().unwrap().audio_properties,
        clip.video().unwrap().audio_properties
    );
    assert_eq!(
        right.video().unwrap().audio_properties,
        clip.video().unwrap().audio_properties
    );
}

#[test]
fn splitting_clip_rejects_its_outer_frames() {
    let clip = video_clip(10, 100, 60);

    let frame_rate = FrameRate::default();
    assert!(
        clip.split_at(TimelineFrameIndex::from(100), frame_rate)
            .is_none()
    );
    assert!(
        clip.split_at(TimelineFrameIndex::from(160), frame_rate)
            .is_none()
    );
    assert!(
        clip.split_at(TimelineFrameIndex::from(101), frame_rate)
            .is_some()
    );
    assert!(
        clip.split_at(TimelineFrameIndex::from(159), frame_rate)
            .is_some()
    );
}

#[test]
fn splitting_text_clip_preserves_text_and_divides_length() {
    let clip = Clip::Text(TextClip::new(
        ulid(10),
        ulid(3),
        TimelineFrameIndex::from(100),
        FrameRate::default().duration(TimelineFrameIndex::from(60)),
        TextClipProperties {
            text: "Title".to_string(),
            ..TextClipProperties::default()
        },
    ));

    let frame_rate = FrameRate::default();
    let (left, right) = clip
        .split_at(TimelineFrameIndex::from(125), frame_rate)
        .unwrap();

    assert_eq!(left.frame_length(frame_rate), TimelineFrameIndex::from(25));
    assert_eq!(right.timeline_start(), TimelineFrameIndex::from(125));
    assert_eq!(right.frame_length(frame_rate), TimelineFrameIndex::from(35));
    assert_eq!(
        left.text().unwrap().properties,
        clip.text().unwrap().properties
    );
    assert_eq!(
        right.text().unwrap().properties,
        clip.text().unwrap().properties
    );
}

#[test]
fn timeline_serialization_stores_integer_frames_and_rational_rate() {
    let project = TimelineEditingState {
        assets: vec![video_asset()],
        clips: vec![video_clip(10, 17, 83)],
        ..TimelineEditingState::default()
    };
    let json = serde_json::to_value(TimelineSerialization::from_editing_state(&project)).unwrap()["editing_state"].clone();
    assert!(json.get("version").is_none());
    assert_eq!(json["settings"]["frame_rate"]["numerator"], 30);
    assert_eq!(json["settings"]["frame_rate"]["denominator"], 1);
    assert_eq!(json["assets"][0]["id"], ulid(100).to_string());
    assert_eq!(json["clips"][0]["kind"], "Video");
    assert_eq!(json["clips"][0]["data"]["id"], ulid(10).to_string());
    assert_eq!(json["clips"][0]["data"]["track_id"], ulid(1).to_string());
    assert_eq!(json["clips"][0]["data"]["asset_id"], ulid(100).to_string());
    assert_eq!(json["clips"][0]["data"]["timeline_start"], 17);
    assert_eq!(json["clips"][0]["data"]["source_out"], 83);
}

#[test]
fn clip_properties_have_neutral_defaults() {
    assert_eq!(
        VideoClipProperties::default(),
        VideoClipProperties {
            position_x: 0.0,
            position_y: 0.0,
            scale: 1.0,
        }
    );
    assert_eq!(
        AudioClipProperties::default(),
        AudioClipProperties {
            gain_db: 0.0,
            muted: false,
        }
    );
}

#[test]
fn untagged_media_clip_is_rejected() {
    let value = serde_json::json!({
        "id": 10,
        "track_id": 1,
        "asset_id": 100,
        "timeline_start": 0,
        "source_in": 0,
        "source_out": 30
    });

    assert!(parse_clip(value).is_err());
}

#[test]
fn text_properties_clip_is_rejected() {
    let value = serde_json::json!({
        "id": 10,
        "track_id": 3,
        "asset_id": 0,
        "timeline_start": 12,
        "source_in": 0,
        "source_out": 90,
        "text_properties": {
            "text": "Legacy title"
        }
    });

    assert!(parse_clip(value).is_err());
}

#[test]
fn tagged_text_clip_deserializes_duration() {
    let value = serde_json::json!({
        "kind": "Text",
        "data": {
            "id": "01M0P9DJ506ZPJ4R4V7CH565X7",
            "track_id": "01M0MCDDQWBN2HXFPJXY4BMQES",
            "timeline_start": 12,
            "length": {
                "secs": 3,
                "nanos": 0
            },
            "properties": {
                "text": "Frame title",
                "future_text_field": true
            },
            "future_clip_field": { "value": 1 }
        },
    });

    let clip = parse_clip(value).unwrap();
    assert_eq!(clip.text().unwrap().duration, Duration::from_secs(3));
}

#[test]
fn timeline_deserialization_preserves_text_duration() {
    let json = serde_json::json!({
        "settings": {
            "frame_rate": { "numerator": 60, "denominator": 1 },
            "width": 1920,
            "height": 1080,
            "audio_sample_rate": 48000
        },
        "clips": [{
            "kind": "Text",
            "data": {
                "id": "01M0P9DJ506ZPJ4R4V7CH565X7",
                "track_id": "01M0MCDDQWBN2HXFPJXY4BMQES",
                "timeline_start": 9090,
                "length": {
                    "secs": 300,
                    "nanos": 0,
                },
                "properties": { "text": "Text", "future_field": true },
                "future_clip_field": true
            },
            "future_clip_field": true
        }],
        "future_timeline_field": true
    });

    let timeline =
        serde_json::from_value::<TimelineSerialization>(serde_json::json!({"editing_state": json}))
            .inspect_err(|e| eprintln!("{e:#}"))
            .unwrap();
    assert_eq!(
        timeline.editing_state.clips[0].text().unwrap().duration,
        Duration::from_mins(5)
    )
}

#[test]
fn text_clip_round_trip_uses_text_specific_fields() {
    let clip = Clip::Text(TextClip::new(
        ulid(10),
        ulid(3),
        TimelineFrameIndex::from(12),
        FrameRate::default().duration(TimelineFrameIndex::from(90)),
        TextClipProperties::default(),
    ));

    let mut value = clip_json(&clip);
    assert_eq!(value["kind"], "Text");
    assert_eq!(value["data"]["length"]["secs"], 3);
    assert_eq!(value["data"]["length"]["nanos"], 0);
    assert_eq!(value["data"]["properties"]["text"], "Text");
    assert!(value["data"].get("asset_id").is_none());
    assert!(value["data"].get("source_in").is_none());
    value["future_clip_field"] = serde_json::json!(true);
    value["data"]["future_data_field"] = serde_json::json!(true);
    value["data"]["properties"]["future_property_field"] = serde_json::json!(true);

    let restored = parse_clip(value).unwrap();
    assert_eq!(
        restored.text().unwrap().frame_length(FrameRate::default()),
        TimelineFrameIndex::from(90)
    );
}

#[test]
fn clip_properties_round_trip_through_timeline_json() {
    let mut clip = video_clip(10, 0, 30);
    clip.video_mut().unwrap().video_properties = VideoClipProperties {
        position_x: 120.0,
        position_y: -45.0,
        scale: 1.25,
    };
    clip.video_mut().unwrap().audio_properties = AudioClipProperties {
        gain_db: -6.0,
        muted: true,
    };
    let value = clip_json(&clip);
    let restored = parse_clip(value).unwrap();

    assert_eq!(
        restored.video().unwrap().video_properties,
        clip.video().unwrap().video_properties
    );
    assert_eq!(
        restored.video().unwrap().audio_properties,
        clip.video().unwrap().audio_properties
    );
}

fn parse_clip(value: serde_json::Value) -> anyhow::Result<Clip> {
    let mut document: TimelineSerialization =
        serde_json::from_value(serde_json::json!({"editing_state": {"clips": [value]}}))?;
    Ok(document.editing_state.clips.remove(0))
}

fn clip_json(clip: &Clip) -> serde_json::Value {
    let state = TimelineEditingState {
        clips: vec![clip.clone()],
        ..Default::default()
    };
    serde_json::to_value(TimelineSerialization::from_editing_state(&state)).unwrap()["editing_state"]["clips"][0].clone()
}
