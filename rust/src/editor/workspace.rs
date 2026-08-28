use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Deserialize, Serialize)]
pub(super) struct GlobalEditorSettings {
    pub(super) project_root: PathBuf,
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
    }
}

fn settings_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/editor-settings.json")
}

pub(super) fn save_global_editor_settings(settings: &GlobalEditorSettings) -> anyhow::Result<()> {
    let path = settings_path();
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory).map_err(|error| {
            anyhow::anyhow!("could not create {}: {error}", directory.display())
        })?;
    }
    let json = serde_json::to_string_pretty(settings)
        .map_err(|error| anyhow::anyhow!("could not serialize workspace settings: {error}"))?;
    fs::write(&path, format!("{json}\n"))
        .map_err(|error| anyhow::anyhow!("could not write {}: {error}", path.display()))
}
