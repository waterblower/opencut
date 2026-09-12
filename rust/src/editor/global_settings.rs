use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

#[derive(Deserialize, Serialize)]
pub struct GlobalEditorSettings {
    pub project_root: PathBuf,
    #[serde(default)]
    pub minimax_api_key: String,
}

impl GlobalEditorSettings {
    pub fn load() -> Self {
        load_settings(&settings_path(), &PathBuf::from(env!("CARGO_MANIFEST_DIR")))
    }

    pub fn save(&self) -> anyhow::Result<()> {
        save_settings(&settings_path(), self)
    }
}

fn load_settings(path: &std::path::Path, default_root: &std::path::Path) -> GlobalEditorSettings {
    if let Ok(contents) = fs::read_to_string(path)
        && let Ok(mut settings) = serde_json::from_str::<GlobalEditorSettings>(&contents)
    {
        if !settings.project_root.is_dir() {
            settings.project_root = default_root.to_path_buf();
        }
        return settings;
    }

    GlobalEditorSettings {
        project_root: default_root.to_path_buf(),
        minimax_api_key: String::new(),
    }
}

fn save_settings(path: &std::path::Path, settings: &GlobalEditorSettings) -> anyhow::Result<()> {
    if let Some(directory) = path.parent() {
        if let Err(error) = fs::create_dir_all(directory) {
            anyhow::bail!(
                "could not create {}: {error} at {}:{}",
                directory.display(),
                file!(),
                line!()
            );
        }
    }
    let json = match serde_json::to_string_pretty(settings) {
        Ok(json) => json,
        Err(error) => anyhow::bail!(
            "could not serialize global settings: {error} at {}:{}",
            file!(),
            line!()
        ),
    };
    if let Err(error) = fs::write(&path, format!("{json}\n")) {
        anyhow::bail!(
            "could not write {}: {error} at {}:{}",
            path.display(),
            file!(),
            line!()
        );
    }
    Ok(())
}

fn settings_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/editor-settings.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_settings_key_changes_and_project_switches_preserve_configuration() {
        let directory =
            std::env::temp_dir().join(format!("opencut-settings-test-{}", ulid::Ulid::generate()));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("settings.json");
        fs::write(
            &path,
            serde_json::json!({"project_root": directory}).to_string(),
        )
        .unwrap();
        let mut settings = load_settings(&path, &directory);
        assert!(settings.minimax_api_key.is_empty());
        settings.minimax_api_key = "test-key".into();
        save_settings(&path, &settings).unwrap();
        let mut settings = load_settings(&path, &directory);
        settings.project_root = directory.join("missing-project");
        save_settings(&path, &settings).unwrap();
        let mut settings = load_settings(&path, &directory);
        assert_eq!(settings.project_root, directory);
        assert_eq!(settings.minimax_api_key, "test-key");
        let next_project = directory.join("next");
        fs::create_dir(&next_project).unwrap();
        settings.project_root = next_project.clone();
        save_settings(&path, &settings).unwrap();
        let mut settings = load_settings(&path, &directory);
        assert_eq!(settings.project_root, next_project);
        assert_eq!(settings.minimax_api_key, "test-key");
        settings.minimax_api_key.clear();
        save_settings(&path, &settings).unwrap();
        assert!(load_settings(&path, &directory).minimax_api_key.is_empty());
        fs::remove_dir_all(directory).unwrap();
    }
}
