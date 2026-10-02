use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Default, Deserialize, Serialize)]
pub struct GlobalEditorSettings {
    #[serde(default)]
    pub project_root: PathBuf,
    #[serde(default)]
    pub minimax_api_key: String,
}

impl GlobalEditorSettings {
    pub fn load() -> Result<Self> {
        load_settings(&settings_path())
    }

    pub fn save(&self) -> Result<()> {
        save_settings(&settings_path(), self)
    }
}

fn load_settings(path: &std::path::Path) -> Result<GlobalEditorSettings> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(GlobalEditorSettings::default());
        }
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "Could not load editor settings from {} ({}:{})",
                    path.display(),
                    file!(),
                    line!()
                )
            });
        }
    };
    let settings: GlobalEditorSettings = serde_json::from_str(&contents)
        .with_context(|| format!("Parsing settings {}", path.display()))?;
    Ok(settings)
}

fn save_settings(path: &std::path::Path, settings: &GlobalEditorSettings) -> Result<()> {
    if let Some(directory) = path.parent() {
        if let Err(error) = fs::create_dir_all(directory) {
            bail!(
                "could not create {}: {error} at {}:{}",
                directory.display(),
                file!(),
                line!()
            );
        }
    }
    let json = match serde_json::to_string_pretty(settings) {
        Ok(json) => json,
        Err(error) => bail!(
            "could not serialize global settings: {error} at {}:{}",
            file!(),
            line!()
        ),
    };
    if let Err(error) = fs::write(&path, format!("{json}\n")) {
        bail!(
            "could not write {}: {error} at {}:{}",
            path.display(),
            file!(),
            line!()
        );
    }
    Ok(())
}

fn settings_path() -> PathBuf {
    std::env::home_dir()
        .expect("Could not determine the user's home directory")
        .join(".opencut")
        .join("editor-settings.json")
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
        let mut settings = load_settings(&path).unwrap();
        assert!(settings.minimax_api_key.is_empty());
        settings.minimax_api_key = "test-key".into();
        save_settings(&path, &settings).unwrap();
        let mut settings = load_settings(&path).unwrap();
        settings.project_root = directory.join("missing-project");
        save_settings(&path, &settings).unwrap();
        assert_eq!(
            load_settings(&path).unwrap().project_root,
            directory.join("missing-project")
        );
        let next_project = directory.join("next");
        fs::create_dir(&next_project).unwrap();
        settings.project_root = next_project.clone();
        save_settings(&path, &settings).unwrap();
        let mut settings = load_settings(&path).unwrap();
        assert_eq!(settings.project_root, next_project);
        assert_eq!(settings.minimax_api_key, "test-key");
        settings.minimax_api_key.clear();
        save_settings(&path, &settings).unwrap();
        assert!(load_settings(&path).unwrap().minimax_api_key.is_empty());
        fs::remove_dir_all(directory).unwrap();
    }
}
