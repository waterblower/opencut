use anyhow::Context as _;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::editor::generic_containers::HorizontalSplitState;

#[derive(Deserialize, Serialize)]
#[serde(default)]
pub struct ProjectLocalSettings {
    pub active_timeline: Option<PathBuf>,
    pub upper_space_split_state: HorizontalSplitState,
}

impl Default for ProjectLocalSettings {
    fn default() -> Self {
        Self {
            active_timeline: None,
            upper_space_split_state: HorizontalSplitState::new(
                super::DEFAULT_MEDIA_PANEL_WIDTH,
                super::DEFAULT_PROPERTIES_PANEL_WIDTH,
            ),
        }
    }
}

pub fn load_project_local_settings(project_root: &Path) -> ProjectLocalSettings {
    let Ok(contents) = fs::read_to_string(project_local_settings_path(project_root)) else {
        return ProjectLocalSettings::default();
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

pub fn save_project_local_settings(
    project_root: &Path,
    settings: &ProjectLocalSettings,
) -> anyhow::Result<()> {
    let path = project_local_settings_path(project_root);
    let Some(directory) = path.parent() else {
        anyhow::bail!(
            "project-local settings path had no parent directory at {}:{}",
            file!(),
            line!()
        );
    };
    fs::create_dir_all(directory).with_context(|| {
        format!(
            "could not create {} at {}:{}",
            directory.display(),
            file!(),
            line!()
        )
    })?;
    let json = serde_json::to_string_pretty(settings).with_context(|| {
        format!(
            "could not serialize project-local settings at {}:{}",
            file!(),
            line!()
        )
    })?;
    fs::write(&path, format!("{json}\n")).with_context(|| {
        format!(
            "could not write {} at {}:{}",
            path.display(),
            file!(),
            line!()
        )
    })
}

fn project_local_settings_path(project_root: &Path) -> PathBuf {
    project_root.join(".opencut/project.json")
}

#[cfg(test)]
#[path = "tests/project_settings.test.rs"]
mod tests;
