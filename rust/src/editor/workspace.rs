use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

use super::Editor;

#[derive(Deserialize, Serialize)]
pub(super) struct GlobalEditorSettings {
    pub(super) project_root: PathBuf,
    #[serde(default)]
    pub(super) expanded_directories: Vec<PathBuf>,
    #[serde(default = "root_expanded_by_default")]
    pub(super) root_expanded: bool,
}

pub(super) fn load_global_editor_settings() -> GlobalEditorSettings {
    if let Ok(contents) = fs::read_to_string(settings_path())
        && let Ok(settings) = serde_json::from_str::<GlobalEditorSettings>(&contents)
        && settings.project_root.is_dir()
    {
        return settings;
    }

    GlobalEditorSettings {
        project_root: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
        expanded_directories: Vec::new(),
        root_expanded: true,
    }
}

fn settings_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/editor-settings.json")
}

pub(super) fn save_global_editor_settings(settings: &GlobalEditorSettings) -> Result<(), String> {
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

impl Editor {
    pub(super) fn save_explorer_expansion(&mut self) -> Result<(), String> {
        self.global_settings.expanded_directories =
            self.explorer.expanded_directories.iter().cloned().collect();
        self.global_settings.expanded_directories.sort();
        self.global_settings.root_expanded = self.explorer.root_expanded;
        save_global_editor_settings(&self.global_settings)
    }
}

fn root_expanded_by_default() -> bool {
    true
}

#[cfg(test)]
#[path = "workspace.test.rs"]
mod tests;
