use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn temporary_project_root() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("opencut-timelines-{unique}"))
}

#[test]
fn opening_an_empty_root_does_not_create_a_timeline() {
    let root = temporary_project_root();
    fs::create_dir_all(&root).unwrap();

    assert!(load_existing(&root, None).unwrap().is_none());
    assert!(timeline_files(&root).unwrap().is_empty());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn creates_and_discovers_multiple_root_timeline_files() {
    let root = temporary_project_root();
    fs::create_dir_all(&root).unwrap();

    let (first_path, _) = create(&root, Path::new(""), "timeline-1").unwrap();
    let (second_path, _) = create(&root, Path::new(""), "timeline-2").unwrap();

    assert_eq!(first_path, Path::new("timeline-1.timeline.json"));
    assert_eq!(second_path, Path::new("timeline-2.timeline.json"));
    assert_eq!(
        timeline_files(&root).unwrap(),
        vec![first_path, second_path]
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn creates_named_timelines_inside_a_subdirectory() {
    let root = temporary_project_root();
    fs::create_dir_all(root.join("scenes")).unwrap();

    let (opening, _) = create(&root, Path::new("scenes"), "Opening Scene").unwrap();

    assert_eq!(opening, Path::new("scenes/Opening Scene.timeline.json"));
    assert!(root.join(&opening).is_file());
    // Only root timelines are offered as the startup document.
    assert!(timeline_files(&root).unwrap().is_empty());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn suggests_the_next_unused_default_name_per_directory() {
    let root = temporary_project_root();
    fs::create_dir_all(root.join("scenes")).unwrap();

    assert_eq!(default_timeline_name(&root, Path::new("")), "timeline-1");
    create(&root, Path::new(""), "timeline-1").unwrap();
    assert_eq!(default_timeline_name(&root, Path::new("")), "timeline-2");
    // Numbering is independent per directory.
    assert_eq!(
        default_timeline_name(&root, Path::new("scenes")),
        "timeline-1"
    );

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn appends_the_extension_and_rejects_unusable_names() {
    assert_eq!(
        timeline_file_name("Opening Scene").unwrap(),
        "Opening Scene.timeline.json"
    );
    // An already-suffixed name is accepted rather than doubled up.
    assert_eq!(
        timeline_file_name("Opening Scene.timeline.json").unwrap(),
        "Opening Scene.timeline.json"
    );
    assert_eq!(
        timeline_file_name("  spaced  ").unwrap(),
        "spaced.timeline.json"
    );
    assert!(timeline_file_name("").is_none());
    assert!(timeline_file_name("   ").is_none());
    assert!(timeline_file_name(".timeline.json").is_none());
    // Names may not escape the target directory.
    assert!(timeline_file_name("scenes/nested").is_none());
    assert!(timeline_file_name("../escape").is_none());
    assert!(timeline_file_name("/absolute").is_none());
    // A bare ".." is not traversal once suffixed — it is just an odd filename.
    assert_eq!(timeline_file_name("..").unwrap(), "...timeline.json");
}

#[test]
fn rejects_creating_a_timeline_outside_a_directory_or_over_an_existing_file() {
    let root = temporary_project_root();
    fs::create_dir_all(&root).unwrap();

    assert!(create(&root, Path::new("missing"), "intro").is_err());
    create(&root, Path::new(""), "intro").unwrap();
    assert!(create(&root, Path::new(""), "intro").is_err());

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn preferred_timeline_is_loaded() {
    let root = temporary_project_root();
    fs::create_dir_all(&root).unwrap();
    create(&root, Path::new(""), "timeline-0").unwrap();
    let (second_path, mut second) = create(&root, Path::new(""), "timeline-1").unwrap();
    second.settings.width = 1280;
    second.save(&root.join(&second_path)).unwrap();

    let (loaded_path, loaded) = load_existing(&root, Some(&second_path)).unwrap().unwrap();
    assert_eq!(loaded_path, second_path);
    assert_eq!(loaded.settings.width, 1280);
    fs::remove_dir_all(root).unwrap();
}
