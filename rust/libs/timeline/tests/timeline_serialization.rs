use serde_json::{Value, json};
use std::time::Duration;
use timeline::{Clip, TimelineSerialization};

#[test]
fn text_duration_is_independent_of_document_frame_rate() {
    let mut value: Value =
        serde_json::from_str(include_str!("fixtures/shared.timeline.json")).unwrap();
    value["editing_state"]["clips"][3]["data"]["length"] = json!({"secs": 2, "nanos": 0});
    for rate in [30, 60] {
        value["editing_state"]["settings"]["frame_rate"] =
            json!({"numerator": rate, "denominator": 1});
        let document: TimelineSerialization = serde_json::from_value(value.clone()).unwrap();
        let runtime = &document.editing_state;
        let Clip::Text(text) = &runtime.clips[3] else {
            panic!("expected text")
        };
        assert_eq!(text.duration, Duration::from_secs(2));
        assert_eq!(
            serde_json::to_value(document).unwrap()["editing_state"]["clips"][3]["data"]["length"],
            json!({"secs": 2, "nanos": 0})
        );
    }
    value["editing_state"]["clips"][3]["data"]["length"] = json!(120);
    assert!(serde_json::from_value::<TimelineSerialization>(value).is_err());
}

#[test]
fn lowercase_media_and_track_aliases_are_normalized_at_disk_boundary() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/shared.timeline.json")).unwrap();
    let document: TimelineSerialization = serde_json::from_value(fixture.clone()).unwrap();
    let canonical = serde_json::to_value(document).unwrap();
    let mut legacy = fixture;
    for field in ["assets", "tracks"] {
        for item in legacy["editing_state"][field].as_array_mut().unwrap() {
            item["kind"] = json!(item["kind"].as_str().unwrap().to_lowercase());
        }
    }
    assert_eq!(
        serde_json::to_value(serde_json::from_value::<TimelineSerialization>(legacy).unwrap())
            .unwrap(),
        canonical
    );
}

#[test]
fn editing_content_survives_conversion_without_sharing_mutable_state() {
    let raw: Value = serde_json::from_str(include_str!("fixtures/shared.timeline.json")).unwrap();
    let mut editing = raw["editing_state"].clone();
    let document: TimelineSerialization =
        serde_json::from_value(json!({"editing_state": editing})).unwrap();
    editing["clips"][2]["data"]
        .as_object_mut()
        .unwrap()
        .remove("video_properties");
    let mut runtime = document.editing_state.clone();
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
