use crate::editor::Editor;
use crate::event_bus::AppEvent;
use crate::preview::PreviewTarget;
use crate::project_settings::{load_project_local_settings, save_project_local_settings};
use crate::timeline::TimelineRuntimeState;
use ::timeline::TimelineSerialization;
use anyhow::{Context as _, Result};
use gpui::PathPromptOptions;
use gpui::prelude::*;
use player_ui::timeline_player::TimelinePlayer;
use std::path::{Path, PathBuf};
use std::time::Instant;

impl Editor {
    pub(crate) fn open_project_folder(&mut self, cx: &mut Context<Self>) {
        let selection = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Open project folder".into()),
        });
        cx.spawn(async move |editor, cx| {
            let paths = match selection.await {
                Ok(Ok(Some(paths))) => paths,
                Ok(Ok(None)) => return,
                result => {
                    log::error!(
                        "Folder dialog failed: {result:?} at {}:{}",
                        file!(),
                        line!()
                    );
                    return;
                }
            };
            let Some(project_path) = paths.into_iter().next() else {
                return;
            };
            let _ = editor.update(cx, |editor, cx| {
                editor.emit_event(cx, AppEvent::SwitchProject { project_path });
            });
        })
        .detach();
    }

    pub fn prepare_project_switch(&mut self) -> Result<()> {
        if let Some(timeline) = self.timeline.as_ref() {
            timeline.save()?;
        }
        Ok(())
    }

    pub(super) fn open_timeline(
        &mut self,
        relative_path: PathBuf,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        (|| -> Result<()> {
            if self
                .timeline
                .as_ref()
                .is_some_and(|timeline| timeline.path == self.project_root.join(&relative_path))
            {
                self.select_only_clip(None);
                let timeline = self.timeline.as_mut().expect("timeline was checked above");
                timeline.seek_frame(timeline.playhead());
                if !matches!(self.preview.target, PreviewTarget::Timeline { .. }) {
                    self.preview.target = self.create_timeline_preview(cx)?;
                }
                self.explorer.selected_file = Some(relative_path);
                cx.notify();
                return Ok(());
            }
            let path = self.project_root.join(&relative_path);
            let timeline = TimelineSerialization::load(&path)?;
            if let Some(timeline) = self.timeline.as_ref() {
                timeline.save()?;
            }
            self.activate_timeline(relative_path.clone(), timeline, cx)?;
            self.select_only_clip(None);
            self.explorer.selected_file = Some(relative_path.clone());
            Ok(())
        })()
        .context("open_timeline failed")
    }

    /// Saves the current timeline, switches to a freshly created one, and reveals it in
    /// the file tree. `relative_directory` is only used to expand the containing folder.
    pub(crate) fn activate_created_timeline(
        &mut self,
        relative_directory: PathBuf,
        relative_path: PathBuf,
        timeline: TimelineSerialization,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        if let Some(active_timeline) = self.timeline.as_ref() {
            active_timeline.save()?;
        }

        // Expand the target folder so the new timeline is visible in the tree.
        if !relative_directory.as_os_str().is_empty() {
            self.explorer
                .expanded_directories
                .insert(relative_directory);
        }
        self.activate_timeline(relative_path.clone(), timeline, cx)?;
        self.explorer.refresh_file_tree(&self.project_root)?;
        self.save_explorer_expansion()?;
        Ok(())
    }

    fn activate_timeline(
        &mut self,
        timeline_path: PathBuf,
        active_timeline: TimelineSerialization,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let t = Instant::now();
        let res = (|| -> Result<()> {
            self.properties.transform_input_clip_id = None;
            self.properties.text_input_clip_id = None;
            self.timeline = Some(TimelineRuntimeState::from_serialize(
                active_timeline,
                self.project_root.join(timeline_path),
            )?);
            let mut settings = load_project_local_settings(&self.project_root);
            settings.active_timeline = self.timeline.as_ref().and_then(|timeline| {
                timeline
                    .path
                    .strip_prefix(&self.project_root)
                    .ok()
                    .map(Path::to_path_buf)
            });
            save_project_local_settings(&self.project_root, &settings)?;
            self.explorer.search_query = None;
            self.explorer.search_results.clear();
            self.explorer.search_pending = false;
            self.explorer
                .filter
                .update(cx, |filter, cx| filter.clear(cx));
            self.explorer.selected_file = self.timeline.as_ref().and_then(|timeline| {
                timeline
                    .path
                    .strip_prefix(&self.project_root)
                    .ok()
                    .map(Path::to_path_buf)
            });
            self.dismiss_context_menu();
            self.explorer
                .refresh_file_tree(&self.project_root)
                .context("refresh_file_tree failed")?;
            if let Some(timeline) = self.timeline.as_mut() {
                timeline.seek_frame(timeline.playhead());
            }
            self.preview.target = self.create_timeline_preview(cx)?;
            self.schedule_active_timeline_waveforms(cx);
            Ok(())
        })();
        log::debug!("activate_timeline: {}", t.elapsed().as_millis());
        res.context("activate_timeline failed")
    }
}

impl Editor {
    /// Builds a standalone preview player on a snapshot of the active timeline.
    /// It is deliberately decoupled: later edits and playhead moves in the editing area
    /// do not reach it.
    pub fn create_timeline_preview(&self, cx: &mut Context<Self>) -> Result<PreviewTarget> {
        let Some(timeline) = self.timeline.as_ref() else {
            return Ok(PreviewTarget::None);
        };
        let relative_path = match timeline.path.strip_prefix(&self.project_root) {
            Ok(relative_path) => relative_path.to_path_buf(),
            Err(_) => timeline.path.clone(),
        };
        let mut player = TimelinePlayer::new(timeline.editing_state.clone(), &self.project_root)?;
        player.title = relative_path.display().to_string();
        let player = cx.new(move |_| player);
        let task = player.update(cx, |player, cx| player.start(cx));
        Ok(PreviewTarget::Timeline {
            _task: task,
            path: relative_path,
            player,
        })
    }
}
