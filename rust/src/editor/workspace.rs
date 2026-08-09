use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, PartialEq)]
pub(super) struct FileTreeEntry {
    pub relative_path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub is_directory: bool,
    pub is_video: bool,
    pub is_image: bool,
    pub is_audio: bool,
    pub is_timeline: bool,
    pub size_bytes: Option<u64>,
    pub expanded: bool,
}

#[derive(Deserialize, Serialize)]
struct WorkspaceSettings {
    project_root: PathBuf,
    #[serde(default)]
    active_timeline: Option<PathBuf>,
}

pub(super) fn load_project_root() -> PathBuf {
    let settings = settings_path();
    if let Ok(contents) = fs::read_to_string(settings)
        && let Ok(settings) = serde_json::from_str::<WorkspaceSettings>(&contents)
        && settings.project_root.is_dir()
    {
        return settings.project_root;
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub(super) fn save_project_root(project_root: &Path) -> Result<(), String> {
    let mut settings = load_settings().unwrap_or_else(|| default_settings(project_root));
    if settings.project_root != project_root {
        settings.active_timeline = None;
    }
    settings.project_root = project_root.to_path_buf();
    save_settings(&settings)
}

pub(super) fn load_active_timeline(project_root: &Path) -> Option<PathBuf> {
    load_settings().and_then(|settings| {
        (settings.project_root == project_root)
            .then_some(settings.active_timeline)
            .flatten()
    })
}

pub(super) fn save_active_timeline(
    project_root: &Path,
    timeline_path: &Path,
) -> Result<(), String> {
    let mut settings = load_settings().unwrap_or_else(|| default_settings(project_root));
    settings.project_root = project_root.to_path_buf();
    settings.active_timeline = Some(timeline_path.to_path_buf());
    save_settings(&settings)
}

pub(super) fn visible_tree(
    project_root: &Path,
    expanded_directories: &HashSet<PathBuf>,
) -> Result<Vec<FileTreeEntry>, String> {
    let mut entries = Vec::new();
    read_directory(
        project_root,
        Path::new(""),
        0,
        expanded_directories,
        &mut entries,
    )?;
    Ok(entries)
}

/// Searches the complete project tree, independently of which folders are expanded.
/// Matching ancestor directories are included so results retain their hierarchy.
pub(super) fn search_tree(project_root: &Path, query: &str) -> Result<Vec<FileTreeEntry>, String> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    search_directory(project_root, Path::new(""), 0, &query, true)
}

fn search_directory(
    project_root: &Path,
    relative_directory: &Path,
    depth: usize,
    query: &str,
    is_root: bool,
) -> Result<Vec<FileTreeEntry>, String> {
    let directory = project_root.join(relative_directory);
    let children = match directory_children(&directory) {
        Ok(children) => children,
        Err(error) if !is_root => {
            eprintln!("{error}");
            return Ok(Vec::new());
        }
        Err(error) => return Err(error),
    };
    let mut matches = Vec::new();

    for (name, is_directory, size_bytes) in children {
        let relative_path = relative_directory.join(&name);
        if is_directory {
            let descendants =
                search_directory(project_root, &relative_path, depth + 1, query, false)?;
            let directory_matches = relative_path
                .to_string_lossy()
                .to_lowercase()
                .contains(query);
            if directory_matches || !descendants.is_empty() {
                matches.push(file_tree_entry(
                    relative_path,
                    name,
                    depth,
                    true,
                    None,
                    !descendants.is_empty(),
                ));
                matches.extend(descendants);
            }
        } else if relative_path
            .to_string_lossy()
            .to_lowercase()
            .contains(query)
        {
            matches.push(file_tree_entry(
                relative_path,
                name,
                depth,
                false,
                size_bytes,
                false,
            ));
        }
    }

    Ok(matches)
}

fn read_directory(
    project_root: &Path,
    relative_directory: &Path,
    depth: usize,
    expanded_directories: &HashSet<PathBuf>,
    entries: &mut Vec<FileTreeEntry>,
) -> Result<(), String> {
    let directory = project_root.join(relative_directory);
    let children = directory_children(&directory)?;

    for (name, is_directory, size_bytes) in children {
        let relative_path = relative_directory.join(&name);
        let expanded = is_directory && expanded_directories.contains(&relative_path);
        entries.push(file_tree_entry(
            relative_path.clone(),
            name,
            depth,
            is_directory,
            size_bytes,
            expanded,
        ));
        if expanded {
            read_directory(
                project_root,
                &relative_path,
                depth + 1,
                expanded_directories,
                entries,
            )?;
        }
    }
    Ok(())
}

fn directory_children(directory: &Path) -> Result<Vec<(String, bool, Option<u64>)>, String> {
    let mut children = fs::read_dir(directory)
        .map_err(|error| format!("could not read {}: {error}", directory.display()))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            let size_bytes = (!file_type.is_dir())
                .then(|| entry.metadata().ok().map(|metadata| metadata.len()))
                .flatten();
            let name = entry.file_name().to_string_lossy().into_owned();
            if matches!(name.as_str(), ".DS_Store" | ".git" | ".opencut") {
                return None;
            }
            Some((name, file_type.is_dir(), size_bytes))
        })
        .collect::<Vec<_>>();
    children.sort_by(|(left_name, left_dir, _), (right_name, right_dir, _)| {
        right_dir
            .cmp(left_dir)
            .then_with(|| left_name.to_lowercase().cmp(&right_name.to_lowercase()))
    });
    Ok(children)
}

fn file_tree_entry(
    relative_path: PathBuf,
    name: String,
    depth: usize,
    is_directory: bool,
    size_bytes: Option<u64>,
    expanded: bool,
) -> FileTreeEntry {
    FileTreeEntry {
        is_video: !is_directory && is_video_path(&relative_path),
        is_image: !is_directory && is_image_path(&relative_path),
        is_audio: !is_directory && is_audio_path(&relative_path),
        is_timeline: !is_directory && super::timeline_document::is_timeline_path(&relative_path),
        relative_path,
        name,
        depth,
        is_directory,
        size_bytes,
        expanded,
    }
}

pub(super) fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png"
            )
        })
}

pub(super) fn is_video_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "mp4" | "mov" | "m4v" | "mkv" | "webm" | "avi"
            )
        })
}

pub(super) fn is_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "aac" | "flac" | "m4a" | "mp3" | "ogg" | "wav"
            )
        })
}

fn settings_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/editor-settings.json")
}

fn load_settings() -> Option<WorkspaceSettings> {
    let contents = fs::read_to_string(settings_path()).ok()?;
    serde_json::from_str(&contents).ok()
}

fn default_settings(project_root: &Path) -> WorkspaceSettings {
    WorkspaceSettings {
        project_root: project_root.to_path_buf(),
        active_timeline: None,
    }
}

fn save_settings(settings: &WorkspaceSettings) -> Result<(), String> {
    let path = settings_path();
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)
            .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
    }
    let json = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("could not serialize workspace settings: {error}"))?;
    fs::write(&path, format!("{json}\n"))
        .map_err(|error| format!("could not write {}: {error}", path.display()))
}

#[cfg(test)]
#[path = "workspace.test.rs"]
mod tests;
