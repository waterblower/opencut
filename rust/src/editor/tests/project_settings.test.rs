use super::*;

#[test]
fn active_timeline_round_trips_in_project_local_settings() {
    let project_root = std::env::temp_dir().join(format!(
        "opencut-project-settings-{}",
        ulid::Ulid::generate()
    ));
    let active_timeline = Path::new("edits/intro.timeline.json");

    let split = HorizontalSplitState::new(410.0, 320.0);
    let mut settings = load_project_local_settings(&project_root);
    settings.active_timeline = Some(active_timeline.to_path_buf());
    save_project_local_settings(&project_root, &settings).unwrap();
    let mut settings = load_project_local_settings(&project_root);
    settings.upper_space_split_state = split.clone();
    save_project_local_settings(&project_root, &settings).unwrap();
    let mut settings = load_project_local_settings(&project_root);

    assert_eq!(settings.active_timeline.as_deref(), Some(active_timeline));
    assert_eq!(
        serde_json::to_value(&settings.upper_space_split_state).unwrap(),
        serde_json::json!({"left_width": 410.0, "right_width": 320.0})
    );
    let renamed_timeline = Path::new("edits/renamed.timeline.json");
    settings.active_timeline = Some(renamed_timeline.to_path_buf());
    save_project_local_settings(&project_root, &settings).unwrap();
    let settings = load_project_local_settings(&project_root);
    assert_eq!(settings.active_timeline.as_deref(), Some(renamed_timeline));
    assert_eq!(
        serde_json::to_value(&settings.upper_space_split_state).unwrap(),
        serde_json::to_value(&split).unwrap()
    );
    fs::remove_dir_all(project_root).unwrap();
}

#[test]
fn old_settings_restore_default_split_widths() {
    let settings: ProjectLocalSettings =
        serde_json::from_str(r#"{"active_timeline":"intro.timeline.json"}"#).unwrap();
    assert_eq!(
        serde_json::to_value(settings.upper_space_split_state).unwrap(),
        serde_json::to_value(ProjectLocalSettings::default().upper_space_split_state).unwrap()
    );
}
