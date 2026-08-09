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
