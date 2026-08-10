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

    assert_eq!(entry.kind, FileTreeEntryKind::Timeline);
}

#[test]
fn directory_entries_own_their_expansion_state() {
    let entry = file_tree_entry(
        PathBuf::from("media"),
        "media".to_string(),
        0,
        true,
        None,
        true,
    );

    assert_eq!(entry.kind, FileTreeEntryKind::Directory { expanded: true });
}
