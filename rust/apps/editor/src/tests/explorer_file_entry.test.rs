use super::*;

#[test]
fn nested_timeline_files_are_selectable() {
    let entry = file_tree_entry(
        Path::new("/project"),
        PathBuf::from("timelines/devlog.timeline.json"),
        "devlog.timeline.json".to_string(),
        1,
        false,
        Some(1024),
        false,
    );

    assert_eq!(entry.kind, FileTreeEntryKind::Timeline);
    assert_eq!(
        entry.absolute_path,
        PathBuf::from("/project/timelines/devlog.timeline.json")
    );
}

#[test]
fn directory_entries_own_their_expansion_state() {
    let entry = file_tree_entry(
        Path::new("/project"),
        PathBuf::from("media"),
        "media".to_string(),
        0,
        true,
        None,
        true,
    );

    assert_eq!(entry.kind, FileTreeEntryKind::Directory { expanded: true });
}

#[test]
fn only_the_active_timeline_uses_active_metadata() {
    let entry = file_tree_entry(
        Path::new("/project"),
        PathBuf::from("timelines/devlog.timeline.json"),
        "devlog.timeline.json".to_string(),
        1,
        false,
        Some(1024),
        false,
    );

    assert_eq!(file_entry_metadata(&entry, true).as_deref(), Some("ACTIVE"));
    assert_eq!(file_entry_metadata(&entry, false).as_deref(), Some("1 KB"));
}
