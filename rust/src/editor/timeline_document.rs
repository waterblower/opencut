use super::model::Project;
use std::{
    fs,
    path::{Path, PathBuf},
};

const DEFAULT_TIMELINE_FILE: &str = "main.timeline.json";
const TIMELINE_SUFFIX: &str = ".timeline.json";

pub(super) fn is_timeline_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(TIMELINE_SUFFIX))
}

pub(super) fn timeline_files(project_root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = fs::read_dir(project_root)
        .map_err(|error| format!("could not read {}: {error}", project_root.display()))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            (path.is_file() && is_timeline_path(&path))
                .then(|| path.strip_prefix(project_root).ok().map(Path::to_path_buf))
                .flatten()
        })
        .collect::<Vec<_>>();
    paths.sort_by_key(|path| path.to_string_lossy().to_lowercase());
    Ok(paths)
}

pub(super) fn load_or_create(
    project_root: &Path,
    preferred: Option<&Path>,
) -> Result<(PathBuf, Project), String> {
    let timelines = timeline_files(project_root)?;
    let relative_path = preferred
        .filter(|path| timelines.iter().any(|timeline| timeline == *path))
        .map(Path::to_path_buf)
        .or_else(|| timelines.first().cloned())
        .unwrap_or_else(|| PathBuf::from(DEFAULT_TIMELINE_FILE));
    let path = project_root.join(&relative_path);
    let project = if path.is_file() {
        Project::load(&path)?
    } else {
        let project = Project::default();
        project.save(&path)?;
        project
    };
    Ok((relative_path, project))
}

/// Turns a user-entered name into a timeline filename, appending the extension the
/// user is not expected to type. An already-suffixed name is accepted unchanged.
///
/// Returns `None` when the name is empty or is anything other than a single path
/// component, so `a/b` and `..` are rejected rather than escaping the directory.
pub(super) fn timeline_file_name(name: &str) -> Option<String> {
    let name = name.trim();
    let stem = name.strip_suffix(TIMELINE_SUFFIX).unwrap_or(name).trim();
    if stem.is_empty() {
        return None;
    }
    let file_name = format!("{stem}{TIMELINE_SUFFIX}");
    let mut components = Path::new(&file_name).components();
    let component = components.next()?;
    if components.next().is_some() || !matches!(component, std::path::Component::Normal(_)) {
        return None;
    }
    Some(file_name)
}

/// The next unused `timeline-N` name for `relative_directory`, without the extension,
/// suitable for pre-filling the create dialog.
pub(super) fn default_timeline_name(project_root: &Path, relative_directory: &Path) -> String {
    let existing = timeline_file_names(&project_root.join(relative_directory)).unwrap_or_default();
    (1usize..)
        .map(|index| format!("timeline-{index}"))
        .find(|candidate| !existing.contains(&format!("{candidate}{TIMELINE_SUFFIX}")))
        .unwrap_or_else(|| "timeline".to_string())
}

/// Creates an empty timeline named `name` inside `relative_directory`, which is relative
/// to the project root. An empty directory places the timeline at the root. The caller
/// does not need to include the `.timeline.json` extension.
///
/// The returned path is relative to the project root, so it can be handed straight to
/// the file tree and to `Project::load`.
pub(super) fn create(
    project_root: &Path,
    relative_directory: &Path,
    name: &str,
) -> Result<(PathBuf, Project), String> {
    let directory = project_root.join(relative_directory);
    if !directory.is_dir() {
        return Err(format!("{} is not a directory", directory.display()));
    }
    let file_name = timeline_file_name(name)
        .ok_or_else(|| "Enter a single non-empty timeline name.".to_string())?;
    let relative_path = relative_directory.join(&file_name);
    let path = project_root.join(&relative_path);
    if path.exists() {
        return Err(format!("{} already exists.", relative_path.display()));
    }
    let project = Project::default();
    project.save(&path)?;
    Ok((relative_path, project))
}

fn timeline_file_names(directory: &Path) -> Result<Vec<String>, String> {
    Ok(fs::read_dir(directory)
        .map_err(|error| format!("could not read {}: {error}", directory.display()))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            (path.is_file() && is_timeline_path(&path))
                .then(|| path.file_name()?.to_str().map(str::to_owned))
                .flatten()
        })
        .collect())
}

#[cfg(test)]
mod tests {
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
    fn creates_and_discovers_multiple_root_timeline_files() {
        let root = temporary_project_root();
        fs::create_dir_all(&root).unwrap();

        let (main_path, _) = load_or_create(&root, None).unwrap();
        let (second_path, _) = create(&root, Path::new(""), "timeline-1").unwrap();

        assert_eq!(main_path, Path::new("main.timeline.json"));
        assert_eq!(second_path, Path::new("timeline-1.timeline.json"));
        assert_eq!(timeline_files(&root).unwrap(), vec![main_path, second_path]);
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
        load_or_create(&root, None).unwrap();
        let (second_path, mut second) = create(&root, Path::new(""), "timeline-1").unwrap();
        second.settings.width = 1280;
        second.save(&root.join(&second_path)).unwrap();

        let (loaded_path, loaded) = load_or_create(&root, Some(&second_path)).unwrap();
        assert_eq!(loaded_path, second_path);
        assert_eq!(loaded.settings.width, 1280);
        fs::remove_dir_all(root).unwrap();
    }
}
