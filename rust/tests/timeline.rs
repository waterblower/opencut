#![cfg(feature = "timeline")]
#[cfg(feature = "cli")]
use opencut_player::cli::validate;
use opencut_player::timeline::{self as document, *};
use serde_json::{Value, json};

#[test]
fn editor_fixture_round_trips_without_losing_document_fields() {
    let raw: Value = serde_json::from_str(include_str!("fixtures/shared.timeline.json")).unwrap();
    let doc = document::parse(&raw).unwrap();
    #[cfg(feature = "cli")]
    validate::require_valid(&doc, None).unwrap();
    assert_eq!(serde_json::to_value(&doc).unwrap(), raw);
    assert_eq!(doc.content_duration().frames(), 60);
    assert_eq!(doc.view.saved_playhead_frame.frames(), 20);
    assert!(doc.tracks[0].locked);
    assert_eq!(
        doc.clips[3].frame_length(doc.settings.frame_rate).frames(),
        30
    );
    assert_eq!(
        doc.source_frame_at(&doc.clips[0], TimelineTime::ZERO),
        Some(12)
    );
    assert_eq!(
        doc.source_position_at(&doc.clips[0], TimelineTime::ZERO)
            .as_secs_f64(),
        0.5
    );
}

#[test]
fn preserves_gui_aliases_and_normalizes_media_clip_kind() {
    let mut raw: Value =
        serde_json::from_str(include_str!("fixtures/shared.timeline.json")).unwrap();
    raw["tracks"][2]["kind"] = json!("audio");
    raw["assets"][1]["kind"] = json!("audio");
    raw["clips"][2]["kind"] = json!("Media");
    raw["clips"][2]["data"]["id"] = json!(99);
    let tracks = raw.as_object_mut().unwrap().remove("tracks").unwrap();
    raw["layers"] = tracks;
    let doc = document::parse(&raw).unwrap();
    assert!(matches!(&doc.clips[2], Clip::Audio(_)));
    assert_eq!(doc.clips[2].id(), ulid::Ulid::from(99_u128));
}

#[test]
fn legacy_cli_documents_are_rejected_explicitly() {
    for raw in [
        json!({"version":1,"clips":[]}),
        json!({"transitions":[]}),
        json!({"clips":[{"type":"media"}]}),
    ] {
        let error = document::parse(&raw).unwrap_err();
        assert_eq!(error.code, "legacy_cli_format");
        assert!(!error.file.is_empty());
        assert!(error.line > 0);
    }
}

#[cfg(feature = "cli")]
#[test]
fn validation_reports_reference_and_overlap_errors_without_mutation() {
    let raw: Value = serde_json::from_str(include_str!("fixtures/shared.timeline.json")).unwrap();
    let mut doc = document::parse(&raw).unwrap();
    let mut duplicate = doc.clips[0].clone();
    duplicate.set_id(ulid::Ulid::from(100_u128));
    doc.clips.push(duplicate);
    doc.clips[1].set_track_id(ulid::Ulid::from(101_u128));
    let before = serde_json::to_value(&doc).unwrap();
    let findings = validate::validate(&doc, None);
    assert!(findings.iter().any(|f| f.error.code == "unknown_track"));
    assert!(findings.iter().any(|f| f.error.code == "overlap"));
    assert_eq!(serde_json::to_value(&doc).unwrap(), before);
}

#[test]
fn shared_time_rounding_and_text_duration_are_rational() {
    let fps = FrameRate::new(30000, 1001);
    let time = TimelineTime::from_frames(30000);
    assert_eq!(fps.duration(time).as_secs(), 1001);
    assert_eq!(fps.audio_samples(time, 48000), 48_048_000);
    assert_eq!(
        fps.rescale_floor(TimelineTime::from_frames(15), FrameRate::new(24, 1))
            .frames(),
        12
    );
    assert_eq!(fps.frames_from_duration_nearest(fps.duration(time)), time);
}
