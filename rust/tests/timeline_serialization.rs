#![cfg(feature = "timeline")]

use opencut_player::timeline::{Clip, TimelineSerialization, parse};
use serde_json::{Value, json};

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
