use super::*;

#[test]
fn nested_timeline_files_are_selectable() {
    let entry = file_tree_entry(
        PathBuf::from("timelines/devlog.timeline.json"),
        "devlog.timeline.json".to_string(),
        1,
        false,
        Some(1024),
        false,
    );

    assert!(entry.is_timeline);
}

#[test]
fn older_settings_use_the_default_timeline_zoom() {
    let settings: WorkspaceSettings =
        serde_json::from_str(r#"{"project_root":"/tmp/project"}"#).unwrap();
    assert_eq!(
        settings.timeline_pixels_per_second,
        super::super::DEFAULT_TIMELINE_PIXELS_PER_SECOND
    );
}
