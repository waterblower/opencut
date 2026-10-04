use crate::editor::Editor;
use crate::preview::PreviewTarget;
use anyhow::Result;
use gpui::prelude::*;
use gpui::{App, AsyncApp, Entity};
use player_ui::audio_player::AudioPlayer;
use player_ui::video_player::VideoPlayer;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub enum PreviewEvent {
    SelectFile(PathBuf),
    TogglePlayback,
}

impl Editor {
    pub async fn open_file_preview(
        editor: Entity<Self>,
        project_root: PathBuf,
        relative_path: PathBuf,
        audio_only: bool,
        cx: &mut AsyncApp,
    ) -> Result<()> {
        let source = project_root.join(&relative_path);
        editor.update(cx, |editor, cx| -> Result<()> {
            if !file_preview_requested(
                &editor.project_root,
                editor.explorer.selected_file.as_deref(),
                &editor.preview.target,
                &project_root,
                &relative_path,
            ) {
                return Ok(());
            }
            // media_backend is not Send, so the player opens on the UI thread.
            editor.preview.target = if audio_only {
                let player = AudioPlayer::new(source)?;
                let player = cx.new(move |_| player);
                let task = player.update(cx, |player, cx| player.start(cx));
                PreviewTarget::AudioFile {
                    _task: task,
                    path: relative_path.clone(),
                    player,
                }
            } else {
                let player = VideoPlayer::new(source)?;
                let player = cx.new(move |_| player);
                let task = player.update(cx, |player, cx| player.start(cx));
                PreviewTarget::VideoFile {
                    _task: task,
                    path: relative_path.clone(),
                    player,
                }
            };
            cx.notify();
            Ok(())
        })?;
        Ok(())
    }
}

pub fn file_preview_requested(
    project_root: &Path,
    selected_file: Option<&Path>,
    target: &PreviewTarget,
    requested_root: &Path,
    requested_file: &Path,
) -> bool {
    project_root == requested_root
        && selected_file == Some(requested_file)
        && matches!(target, PreviewTarget::None)
}

impl Editor {
    pub fn toggle_preview_playback(&mut self, cx: &mut Context<Self>) -> Result<()> {
        match &self.preview.target {
            PreviewTarget::VideoFile { player, .. } => {
                player.update(cx, |player, cx| player.toggle_playback(cx));
                Ok(())
            }
            PreviewTarget::AudioFile { player, .. } => {
                player.update(cx, |player, cx| player.toggle_playback(cx))
            }
            PreviewTarget::Timeline { player, .. } => {
                player.update(cx, |player, cx| player.toggle_playback(cx))
            }
            PreviewTarget::None | PreviewTarget::ImageFile(_) => Ok(()),
        }
    }
}

#[cfg(test)]
#[path = "tests/preview_events.test.rs"]
mod tests;
