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

pub(super) fn create(project_root: &Path) -> Result<(PathBuf, Project), String> {
    let existing = timeline_files(project_root)?;
    let relative_path = (1usize..)
        .map(|index| PathBuf::from(format!("timeline-{index}{TIMELINE_SUFFIX}")))
        .find(|candidate| !existing.contains(candidate))
        .ok_or_else(|| "could not choose a timeline filename".to_string())?;
    let project = Project::default();
    project.save(&project_root.join(&relative_path))?;
    Ok((relative_path, project))
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
        let (second_path, _) = create(&root).unwrap();

        assert_eq!(main_path, Path::new("main.timeline.json"));
        assert_eq!(second_path, Path::new("timeline-1.timeline.json"));
        assert_eq!(timeline_files(&root).unwrap(), vec![main_path, second_path]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn preferred_timeline_is_loaded() {
        let root = temporary_project_root();
        fs::create_dir_all(&root).unwrap();
        load_or_create(&root, None).unwrap();
        let (second_path, mut second) = create(&root).unwrap();
        second.settings.width = 1280;
        second.save(&root.join(&second_path)).unwrap();

        let (loaded_path, loaded) = load_or_create(&root, Some(&second_path)).unwrap();
        assert_eq!(loaded_path, second_path);
        assert_eq!(loaded.settings.width, 1280);
        fs::remove_dir_all(root).unwrap();
    }
}
