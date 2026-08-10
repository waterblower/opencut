use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

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
