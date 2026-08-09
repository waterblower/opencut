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
    assert!(settings.timeline_views.is_empty());
}

#[test]
fn timeline_view_state_sanitizes_values_before_persistence() {
    let view = timeline_view_state(
        Path::new("/tmp/project"),
        -10,
        f32::NAN,
        -20.0,
        false,
        false,
    );
    assert_eq!(view.playhead_frame, 0);
    assert_eq!(view.horizontal_scroll, 0.0);
    assert_eq!(view.vertical_scroll, 0.0);
    assert!(!view.snapping_enabled);
    assert!(!view.track_magnet_enabled);
}

#[test]
fn older_timeline_views_enable_snap_and_magnet_by_default() {
    let view: TimelineViewState = serde_json::from_str(
        r#"{
            "timeline_path": "/tmp/project/main.timeline.json",
            "playhead_frame": 10,
            "horizontal_scroll": 20.0,
            "vertical_scroll": 30.0
        }"#,
    )
    .unwrap();

    assert!(view.snapping_enabled);
    assert!(view.track_magnet_enabled);
}
