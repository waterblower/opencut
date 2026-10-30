use serde_json::{Value, json};
use std::time::Duration;
use timeline::{Asset, Clip, TimelineSerialization};

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
fn asset_kinds_preserve_capitalization_and_reject_lowercase() {
    let fixture: Value =
        serde_json::from_str(include_str!("fixtures/shared.timeline.json")).unwrap();
    let document: TimelineSerialization = serde_json::from_value(fixture.clone()).unwrap();
    let canonical = serde_json::to_value(document).unwrap();
    for (index, kind) in ["Video", "Audio", "Image"].into_iter().enumerate() {
        assert_eq!(canonical["editing_state"]["assets"][index]["kind"], kind);
        let mut lowercase = fixture.clone();
        lowercase["editing_state"]["assets"][index]["kind"] = json!(kind.to_lowercase());
        assert!(serde_json::from_value::<TimelineSerialization>(lowercase).is_err());
    }
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
    for field in [
        "duration",
        "framerate",
        "frame_rate_numerator",
        "frame_rate_denominator",
        "has_audio",
    ] {
        editing["assets"][2].as_object_mut().unwrap().remove(field);
    }
    editing["clips"][3]["data"]["properties"]["background"] = Value::Null;
    let mut runtime = document.editing_state.clone();
    runtime.validate().unwrap();

    let captured = TimelineSerialization::from_editing_state(&runtime);
    assert_eq!(
        serde_json::to_value(&captured).unwrap()["editing_state"],
        editing
    );

    // Both directions produce independent ownership, including nested clip data.
    let Asset::Video(video) = &mut runtime.assets[0] else {
        panic!("fixture must include a video asset");
    };
    video.name = "Changed asset".into();
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
