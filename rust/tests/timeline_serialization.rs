#![cfg(feature = "timeline")]

use opencut_player::timeline::{Clip, TimelineSerialization, parse};
use serde_json::{Value, json};
use std::time::Duration;

#[test]
fn legacy_text_frame_lengths_use_document_rate_in_both_envelopes() {
    let mut editing: Value =
        serde_json::from_str(include_str!("fixtures/shared.timeline.json")).unwrap();
    editing["settings"]["frame_rate"] = json!({"numerator": 60, "denominator": 1});
    editing["clips"][3]["data"]["length"] = json!(120);
    for value in [editing.clone(), json!({"editing_state": editing})] {
        let document = parse(&value).unwrap();
        let runtime = document.to_editing_state();
        let Clip::Text(text) = &runtime.clips[3] else {
            panic!("expected text")
        };
        assert_eq!(text.length, Duration::from_secs(2));
        assert_eq!(
            serde_json::to_value(document).unwrap()["editing_state"]["clips"][3]["data"]["length"],
            json!({"secs": 2, "nanos": 0})
        );
    }
}

#[test]
fn lowercase_media_and_track_aliases_are_normalized_at_disk_boundary() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/shared.timeline.json")).unwrap();
    let canonical = serde_json::to_value(parse(&fixture).unwrap()).unwrap();
    let mut legacy = fixture;
    for field in ["assets", "tracks"] {
        for item in legacy[field].as_array_mut().unwrap() {
            item["kind"] = json!(item["kind"].as_str().unwrap().to_lowercase());
        }
    }
    assert_eq!(
        serde_json::to_value(parse(&legacy).unwrap()).unwrap(),
        canonical
    );
}

#[test]
fn editing_content_survives_conversion_without_sharing_mutable_state() {
    let mut editing: Value =
        serde_json::from_str(include_str!("fixtures/shared.timeline.json")).unwrap();
    editing.as_object_mut().unwrap().remove("view");
    let document = parse(&json!({"editing_state": editing})).unwrap();
    let mut runtime = document.to_editing_state();
    runtime.validate().unwrap();

    let captured = TimelineSerialization::from_editing_state(&runtime);
    assert_eq!(
        serde_json::to_value(&captured).unwrap()["editing_state"],
        editing
    );

    // Both directions produce independent ownership, including nested clip data.
    runtime.assets[0].name = "Changed asset".into();
    runtime.tracks[0].name = "Changed track".into();
    let Clip::Text(text) = &mut runtime.clips[3] else {
        panic!("fixture must include a text clip");
    };
    text.properties.text = "Changed caption".into();
    assert_eq!(
        serde_json::to_value(&document).unwrap()["editing_state"],
        editing
    );
    assert_eq!(
        serde_json::to_value(&captured).unwrap()["editing_state"],
        editing
    );
}
