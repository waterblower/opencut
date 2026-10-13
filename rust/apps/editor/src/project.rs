use crate::editor::Editor;
use crate::event_bus::AppEvent;
use crate::explorer_file_entry::visible_tree;
use crate::preview::{PreviewTarget, timeline_preview_target};
use crate::project_settings::{load_project_local_settings, save_project_local_settings};
use crate::timeline::TimelineRuntimeState;
use ::timeline::TimelineSerialization;
use anyhow::{Context as _, Result};
use gpui::PathPromptOptions;
use gpui::prelude::*;
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
                timeline.set_playhead(timeline.playhead());
                if !matches!(self.preview.target, PreviewTarget::Timeline { .. }) {
                    self.preview.target =
                        timeline_preview_target(timeline, &self.project_root, cx)?;
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
        let res = (|| -> Result<()> {
            // 先完成所有可能失败的工作，全部成功后才修改 self；失败时编辑器保持原状。
            let mut new_timeline = TimelineRuntimeState::from_serialize(
                active_timeline,
                self.project_root.join(timeline_path),
            )?;
            new_timeline.set_playhead(new_timeline.playhead());
            let new_preview_target =
                timeline_preview_target(&new_timeline, &self.project_root, cx)?;
            let new_relative_timeline_path = new_timeline
                .path
                .strip_prefix(&self.project_root)
                .ok()
                .map(Path::to_path_buf);
            let new_file_tree =
                visible_tree(&self.project_root, &self.explorer.expanded_directories)
                    .context("scanning project file tree")?;
            let mut new_settings = load_project_local_settings(&self.project_root);
            new_settings.active_timeline = new_relative_timeline_path.clone();
            save_project_local_settings(&self.project_root, &new_settings)?;

            self.properties.transform_input_clip_id = None;
            self.properties.text_input_clip_id = None;
            self.timeline = Some(new_timeline);
            self.preview.target = new_preview_target;
            self.explorer.search_query = None;
            self.explorer.search_results.clear();
            self.explorer.search_pending = false;
            self.explorer
                .filter
                .update(cx, |filter, cx| filter.clear(cx));
            self.explorer.selected_file = new_relative_timeline_path;
            self.dismiss_context_menu();
            self.explorer.file_tree = new_file_tree;
            self.explorer.last_tree_scan = Instant::now();
            self.schedule_active_timeline_waveforms(cx);
            Ok(())
        })();
        res.context("activate_timeline failed")
    }
}
