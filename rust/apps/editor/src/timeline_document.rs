use ::timeline::TimelineSerialization;
use anyhow::{Context as _, Result, anyhow};
use std::fs;
use std::path::{Component, Path, PathBuf};

const TIMELINE_SUFFIX: &str = ".timeline";
const LEGACY_TIMELINE_SUFFIX: &str = ".timeline.json";

pub(super) fn is_timeline_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            name.ends_with(TIMELINE_SUFFIX) || name.ends_with(LEGACY_TIMELINE_SUFFIX)
        })
}

pub(super) fn project_timeline_files(project_root: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    collect_timeline_files(project_root, project_root, &mut paths)?;
    paths.sort_by_key(|path| path.to_string_lossy().to_lowercase());
    Ok(paths)
}

fn collect_timeline_files(
    project_root: &Path,
    directory: &Path,
    paths: &mut Vec<PathBuf>,
) -> Result<()> {
    for entry in fs::read_dir(directory)
        .map_err(|error| anyhow!("could not read {}: {error}", directory.display()))?
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path.is_dir() {
            if !matches!(
                entry.file_name().to_str(),
                Some(".git" | ".opencut" | "target")
            ) {
                collect_timeline_files(project_root, &path, paths)?;
            }
        } else if is_timeline_path(&path)
            && let Ok(relative) = path.strip_prefix(project_root)
        {
            paths.push(relative.to_path_buf());
        }
    }
    Ok(())
}

/// Turns a user-entered name into a timeline filename, appending the extension the
/// user is not expected to type. An already-suffixed name is accepted unchanged.
///
/// Returns `None` when the name is empty or is anything other than a single path
/// component, so `a/b` and `..` are rejected rather than escaping the directory.
pub(super) fn timeline_file_name(name: &str) -> Option<String> {
    let name = name.trim();
    let suffix = if name.ends_with(LEGACY_TIMELINE_SUFFIX) {
        LEGACY_TIMELINE_SUFFIX
    } else {
        TIMELINE_SUFFIX
    };
    let stem = name.strip_suffix(suffix).unwrap_or(name).trim();
    if stem.is_empty() {
        return None;
    }
    let file_name = format!("{stem}{suffix}");
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
        .find(|candidate| {
            !existing.contains(&format!("{candidate}{TIMELINE_SUFFIX}"))
                && !existing.contains(&format!("{candidate}{LEGACY_TIMELINE_SUFFIX}"))
        })
        .unwrap_or_else(|| "timeline".to_string())
}

/// Creates an empty timeline named `name` inside `relative_directory`, which is relative
/// to the project root. An empty directory places the timeline at the root. The caller
/// does not need to include the `.timeline` extension. The file contents are JSON.
///
/// The returned path is relative to the project root, so it can be handed straight to
/// the file tree and to `Timeline::load`.
pub(super) fn create(
    project_root: &Path,
    relative_directory: &Path,
    name: &str,
) -> Result<(PathBuf, TimelineSerialization)> {
    let directory = project_root.join(relative_directory);
    if !directory.is_dir() {
        return Err(anyhow!("{} is not a directory", directory.display()));
    }
    let file_name = timeline_file_name(name)
        .ok_or_else(|| anyhow!("Enter a single non-empty timeline name."))?;
    let relative_path = relative_directory.join(&file_name);
    let path = project_root.join(&relative_path);
    if path.exists() {
        return Err(anyhow!("{} already exists.", relative_path.display()));
    }
    let timeline = TimelineSerialization::default();
    timeline.save(&path)?;
    Ok((relative_path, timeline))
}

/// The media file for `asset_path`, which a timeline stores relative to its own directory.
/// `..` components are resolved lexically, without touching the file system.
pub fn resolve_asset_path(timeline_directory: &Path, asset_path: &Path) -> PathBuf {
    let mut resolved = PathBuf::new();
    for component in timeline_directory.join(asset_path).components() {
        match component {
            Component::ParentDir => {
                resolved.pop();
            }
            Component::CurDir => {}
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                resolved.push(component);
            }
        }
    }
    resolved
}

/// The asset path a timeline in `timeline_directory` stores for `media_path`; climbs with `..`
/// when the media lies outside that directory. Both paths must be absolute.
/// Paths on different Windows volumes remain absolute.
pub fn relative_asset_path(timeline_directory: &Path, media_path: &Path) -> PathBuf {
    let directory = timeline_directory.components().collect::<Vec<_>>();
    let media = media_path.components().collect::<Vec<_>>();
    if directory.first() != media.first() {
        return media_path.to_path_buf();
    }
    let shared = directory
        .iter()
        .zip(&media)
        .take_while(|(directory_component, media_component)| directory_component == media_component)
        .count();
    let mut relative = PathBuf::new();
    for _ in shared..directory.len() {
        relative.push("..");
    }
    for component in &media[shared..] {
        relative.push(component);
    }
    relative
}

fn timeline_file_names(directory: &Path) -> Result<Vec<String>> {
    Ok(fs::read_dir(directory)
        .map_err(|error| anyhow!("could not read {}: {error}", directory.display()))?
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
#[path = "tests/timeline_document.test.rs"]
mod tests;
