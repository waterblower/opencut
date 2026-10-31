use serde_json::{Value, json};
use std::time::Duration;
use timeline::*;

#[test]
fn editor_fixture_round_trips_without_audio_visual_properties() {
    let raw: Value = serde_json::from_str(include_str!("fixtures/shared.timeline.json")).unwrap();
    let serialized: TimelineSerialization = serde_json::from_value(raw.clone()).unwrap();
    let doc = &serialized.editing_state;
    doc.validate().unwrap();
    let mut expected = raw.clone();
    // 兼容旧音频字段，但保存时不再写出视频属性。
    assert!(
        expected["editing_state"]["clips"][2]["data"]
            .as_object_mut()
            .unwrap()
            .remove("video_properties")
            .is_some()
    );
    assert_eq!(serde_json::to_value(&serialized).unwrap(), expected);
    assert_eq!(i64::from(doc.content_duration()), 60);
    assert_eq!(i64::from(serialized.playhead()), 20);
    assert!(doc.tracks[0].locked);
    assert_eq!(
        i64::from(doc.clips[3].frame_length(doc.settings.frame_rate)),
        30
    );
    assert_eq!(
        doc.source_frame_at(&doc.clips[0], TimelineFrameIndex::ZERO),
        Some(12)
    );
    assert_eq!(
        doc.source_position_at(&doc.clips[0], TimelineFrameIndex::ZERO)
            .as_secs_f64(),
        0.5
    );
}

#[test]
fn preserves_gui_aliases_without_converting_clip_types() {
    let mut raw: Value =
        serde_json::from_str(include_str!("fixtures/shared.timeline.json")).unwrap();
    let editing = &mut raw["editing_state"];
    editing["tracks"][2]["kind"] = json!("audio");
    editing["assets"][1]["kind"] = json!("audio");
    editing["clips"][2]["data"]["id"] = json!(99);
    let tracks = editing.as_object_mut().unwrap().remove("tracks").unwrap();
    editing["layers"] = tracks;
    for kind in ["Media", "Video"] {
        raw["editing_state"]["clips"][0]["kind"] = json!(kind);
        let document: TimelineSerialization = serde_json::from_value(raw.clone()).unwrap();
        let doc = document.editing_state;
        doc.validate().unwrap();
        assert!(matches!(&doc.clips[0], Clip::Video(_)));
        assert!(matches!(&doc.clips[2], Clip::Audio(_)));
        assert_eq!(doc.clips[2].id(), ulid::Ulid::from(99_u128));
        raw["editing_state"]["clips"][2]["kind"] = json!(kind);
        let invalid: TimelineSerialization = serde_json::from_value(raw.clone()).unwrap();
        assert!(invalid.editing_state.validate().is_err());
        raw["editing_state"]["clips"][2]["kind"] = json!("Audio");
    }
    raw["editing_state"]["clips"][2]["data"]["track_id"] =
        raw["editing_state"]["layers"][0]["id"].clone();
    let document: TimelineSerialization = serde_json::from_value(raw).unwrap();
    assert!(
        document
            .editing_state
            .validate()
            .unwrap_err()
            .to_string()
            .contains("requires an audio track")
    );
}

#[test]
fn legacy_cli_documents_are_rejected_explicitly() {
    for raw in [
        json!({"version":1,"clips":[]}),
        json!({"transitions":[]}),
        json!({"clips":[{"type":"media"}]}),
    ] {
        let error = serde_json::from_value::<TimelineSerialization>(raw).unwrap_err();
        assert!(error.to_string().contains("missing field `editing_state`"));
    }
}

#[test]
fn validation_reports_invalid_data_without_mutation() {
    let raw: Value = serde_json::from_str(include_str!("fixtures/shared.timeline.json")).unwrap();
    let document: TimelineSerialization = serde_json::from_value(raw).unwrap();
    let mut doc = document.editing_state.clone();
    let audio_track_id = doc.clips[2].track_id();
    let video_track_id = doc.clips[0].track_id();
    doc.clips[2].set_track_id(video_track_id);
    assert!(
        doc.validate()
            .unwrap_err()
            .to_string()
            .contains("requires an audio track")
    );
    doc.clips[2].set_track_id(audio_track_id);
    doc.clips[1].set_track_id(ulid::Ulid::from(101_u128));
    let before = serde_json::to_value(TimelineSerialization::from_editing_state(&doc)).unwrap();
    let error = doc.validate().unwrap_err();
    assert!(error.to_string().contains("references missing track"));
    assert_eq!(
        serde_json::to_value(TimelineSerialization::from_editing_state(&doc)).unwrap(),
        before
    );
    let path = std::env::temp_dir().join(format!(
        "opencut-validation-{}.json",
        ulid::Ulid::generate()
    ));
    TimelineSerialization::from_editing_state(&doc)
        .save(&path)
        .unwrap();
    let loaded = TimelineSerialization::load(&path);
    std::fs::remove_file(&path).unwrap();
    assert!(format!("{:?}", loaded.unwrap_err()).contains("references missing track"));

    document.editing_state.validate().unwrap(); // 不同轨道上的重叠仍然合法。
    for original in &document.editing_state.clips {
        let mut with_copy = document.editing_state.clone();
        let copy_id = ulid::Ulid::generate();
        let mut adjacent = original.copy(copy_id);
        let end = original.timeline_end(with_copy.settings.frame_rate);
        adjacent.set_timeline_start(end);
        with_copy.clips.push(adjacent);
        with_copy.clips.reverse(); // 校验不能依赖输入顺序，也不能改变文档顺序。
        with_copy.validate().unwrap();

        for start in [
            original.timeline_start(),
            end - TimelineFrameIndex::ONE_FRAME,
        ] {
            with_copy
                .clip_mut(copy_id)
                .unwrap()
                .set_timeline_start(start);
            let before_overlap =
                serde_json::to_value(TimelineSerialization::from_editing_state(&with_copy))
                    .unwrap();
            let overlap_error = with_copy.validate().unwrap_err().to_string();
            assert!(overlap_error.contains("overlap on track"));
            assert!(overlap_error.contains(&original.id().to_string()));
            assert!(overlap_error.contains(&copy_id.to_string()));
            assert!(overlap_error.contains(&original.track_id().to_string()));
            assert_eq!(
                serde_json::to_value(TimelineSerialization::from_editing_state(&with_copy))
                    .unwrap(),
                before_overlap
            );
        }
    }
}

#[test]
fn shared_time_rounding_and_text_duration_are_rational() {
    let fps = FrameRate::new(30000, 1001);
    for frames in [i64::MIN, -1, 0] {
        let time = TimelineFrameIndex::from(frames);
        assert_eq!(fps.duration(time), std::time::Duration::ZERO);
        assert_eq!(fps.audio_samples(time, 48000), 0);
        assert_eq!(
            fps.rescale_nearest(time, FrameRate::new(24, 1)),
            TimelineFrameIndex::ZERO
        );
        assert_eq!(
            fps.rescale_floor(time, FrameRate::new(24, 1)),
            TimelineFrameIndex::ZERO
        );
    }
    let time = TimelineFrameIndex::from(30000);
    assert_eq!(fps.duration(time).as_secs(), 1001);
    assert_eq!(fps.audio_samples(time, 48000), 48_048_000);
    assert_eq!(
        i64::from(fps.rescale_floor(15.into(), FrameRate::new(24, 1))),
        12
    );
    assert_eq!(fps.frames_from_duration_nearest(fps.duration(time)), time);
}

#[test]
fn frame_rates_round_trip_through_seconds_and_duration() {
    for frame_rate in [
        FrameRate::new(24, 1),
        FrameRate::new(30, 1),
        FrameRate::new(60, 1),
        FrameRate::new(24_000, 1_001),
        FrameRate::new(30_000, 1_001),
        FrameRate::new(60_000, 1_001),
    ] {
        let ten_hours = frame_rate.nearest(10.0 * 60.0 * 60.0);
        for frame in (0..10_000).chain([1_000_003, i64::from(ten_hours)]) {
            let time = TimelineFrameIndex::from(frame);
            assert_eq!(
                frame_rate.nearest(frame_rate.seconds(time)),
                time,
                "seconds did not round-trip frame {frame} at {frame_rate:?}",
            );
            assert_eq!(
                frame_rate.frames_from_duration_nearest(frame_rate.duration(time)),
                time,
                "duration did not round-trip frame {frame} at {frame_rate:?}",
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
fn preview_and_export_boundaries_share_the_same_frame_time() {
    let project = TimelineEditingState {
        settings: TimelineSettings {
            frame_rate: FrameRate::new(24_000, 1_001),
            ..TimelineSettings::default()
        },
        ..TimelineEditingState::default()
    };
    let boundary = TimelineFrameIndex::from(98_765);
    let preview_duration = project.position_at_frame(boundary).as_secs_f64();
    let export_seconds = project.seconds(boundary);
    assert!((preview_duration - export_seconds).abs() <= 1.0e-9);
}
