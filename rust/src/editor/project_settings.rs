use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Default, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct ProjectLocalSettings {
    pub(super) active_timeline: Option<PathBuf>,
}

pub(super) fn load_project_local_settings(project_root: &Path) -> ProjectLocalSettings {
    let Ok(contents) = fs::read_to_string(project_local_settings_path(project_root)) else {
        return ProjectLocalSettings::default();
    };
    serde_json::from_str(&contents).unwrap_or_default()
}

pub(super) fn save_project_local_settings(
    project_root: &Path,
    active_timeline: Option<&Path>,
) -> anyhow::Result<()> {
    let path = project_local_settings_path(project_root);
    let Some(directory) = path.parent() else {
        anyhow::bail!("project-local settings path had no parent directory");
    };
    fs::create_dir_all(directory)
        .map_err(|error| anyhow::anyhow!("could not create {}: {error}", directory.display()))?;
    let json = serde_json::to_string_pretty(&ProjectLocalSettings {
        active_timeline: active_timeline.map(Path::to_path_buf),
    })
    .map_err(|error| anyhow::anyhow!("could not serialize project-local settings: {error}"))?;
    fs::write(&path, format!("{json}\n"))
        .map_err(|error| anyhow::anyhow!("could not write {}: {error}", path.display()))
}

fn project_local_settings_path(project_root: &Path) -> PathBuf {
    project_root.join(".opencut/project.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_timeline_round_trips_in_project_local_settings() {
        let project_root = std::env::temp_dir().join(format!(
            "opencut-project-settings-{}",
            ulid::Ulid::generate()
        ));
        let active_timeline = Path::new("edits/intro.timeline.json");

        save_project_local_settings(&project_root, Some(active_timeline)).unwrap();
        let settings = load_project_local_settings(&project_root);

        assert_eq!(settings.active_timeline.as_deref(), Some(active_timeline));
        fs::remove_dir_all(project_root).unwrap();
    }
}
