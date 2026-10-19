use crate::ui_components::{HorizontalSplitState, VerticalSplitState};
use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Serialize)]
#[serde(default)]
pub(crate) struct ProjectLocalSettings {
    pub(crate) active_timeline: Option<PathBuf>,
    pub(crate) upper_space_split_state: HorizontalSplitState,
    pub(crate) workspace_split_state: VerticalSplitState, // 上半区与时间线之间的分割；只保存时间线高度。
}

impl Default for ProjectLocalSettings {
    fn default() -> Self {
        Self {
            active_timeline: None,
            upper_space_split_state: HorizontalSplitState::new(
                crate::layout::DEFAULT_MEDIA_PANEL_WIDTH,
                crate::layout::DEFAULT_PROPERTIES_PANEL_WIDTH,
            ),
            workspace_split_state: VerticalSplitState::new(crate::layout::DEFAULT_TIMELINE_HEIGHT),
        }
    }
}

// This function does IO, avoid it during render pass / render tree construction
pub(crate) fn load_project_local_settings(project_root: &Path) -> ProjectLocalSettings {
    let Ok(contents) = fs::read_to_string(project_local_settings_path(project_root)) else {
        return ProjectLocalSettings::default();
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

pub(crate) fn save_project_local_settings(
    project_root: &Path,
    settings: &ProjectLocalSettings,
) -> Result<()> {
    let path = project_local_settings_path(project_root);
    let Some(directory) = path.parent() else {
        bail!(
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
