use super::*;

impl Editor {
    /// Opens the new-timeline dialog for `relative_directory`, pre-filled with the next
    /// unused default name so the user can accept it without typing.
    pub(crate) fn begin_create_timeline(
        &mut self,
        relative_directory: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dismiss_context_menu();
        let default_name = timeline_document::default_timeline_name(
            &self.global_settings.project_root,
            &relative_directory,
        );
        let input = cx.new(|cx| {
            ExplorerFilter::new_field(
                "new-timeline-name",
                default_name,
                "Timeline name",
                self.focus_handle.clone(),
                cx,
            )
        });
        input.update(cx, |input, cx| input.focus_and_select_all(window, cx));
        self.explorer.new_timeline_dialog = Some(NewTimelineDialogState {
            relative_directory,
            input,
        });
        cx.notify();
    }

    pub(crate) fn finish_create_timeline(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let Some(state) = self.explorer.new_timeline_dialog.as_ref() else {
            return Ok(());
        };
        let relative_directory = state.relative_directory.clone();
        let name = state.input.read(cx).query().trim().to_string();
        let (relative_path, timeline) = timeline_document::create(
            &self.global_settings.project_root,
            &relative_directory,
            &name,
        )
        .map_err(|error| format!("Could not create timeline: {error}"))?;
        self.explorer.new_timeline_dialog = None;
        self.activate_created_timeline(relative_directory, relative_path, timeline, cx)
    }

    pub(crate) fn begin_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let ContextMenu::File(menu) = &self.context_menu else {
            return;
        };
        let relative_path = menu.relative_path.clone();
        self.dismiss_context_menu();
        let Some(name) = relative_path
            .file_name()
            .map(|name: &std::ffi::OsStr| name.to_string_lossy().into_owned())
        else {
            eprintln!("The project folder cannot be renamed here.");
            return;
        };
        let input = cx.new(|cx| {
            ExplorerFilter::new_field(
                "rename-project-entry",
                name,
                "New name",
                self.focus_handle.clone(),
                cx,
            )
        });
        input.update(cx, |input, cx| input.focus_and_select_all(window, cx));
        self.explorer.rename_dialog = Some(RenameDialogState {
            relative_path,
            input,
        });
        cx.notify();
    }

    pub(crate) fn finish_rename(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let Some(state) = self.explorer.rename_dialog.as_ref() else {
            return Ok(());
        };
        let old_relative = state.relative_path.clone();
        let new_name = state.input.read(cx).query().trim().to_string();
        let Some(new_relative) = renamed_relative_path(&old_relative, &new_name) else {
            return Err("Enter a single non-empty file or folder name.".to_string());
        };
        if new_relative == old_relative {
            self.explorer.rename_dialog = None;
            return Ok(());
        }

        let old_path = self.global_settings.project_root.join(&old_relative);
        let new_path = self.global_settings.project_root.join(&new_relative);
        if new_path.exists() {
            return Err(format!(
                "Cannot rename: {} already exists.",
                new_relative.display()
            ));
        }
        std::fs::rename(&old_path, &new_path)
            .map_err(|error| format!("Could not rename {}: {error}", old_relative.display()))?;

        if let Some(timeline) = self.timeline.as_mut() {
            let paths = timeline
                .data
                .assets
                .iter()
                .filter_map(|asset| {
                    remap_relative_path(&asset.path, &old_relative, &new_relative)
                        .map(|path| (asset.id, path))
                })
                .collect();
            edit_and_rebuild_timeline(
                &mut self.preview,
                &self.global_settings.project_root,
                timeline,
                EditAction::UpdateAssetPaths { paths },
            )
            .expect("updating asset paths cannot be rejected");
            for snapshot in timeline
                .undo_stack
                .iter_mut()
                .chain(timeline.redo_stack.iter_mut())
            {
                for asset in &mut snapshot.assets {
                    if let Some(path) =
                        remap_relative_path(&asset.path, &old_relative, &new_relative)
                    {
                        asset.path = path;
                    }
                }
            }
        }
        self.explorer.expanded_directories = self
            .explorer
            .expanded_directories
            .iter()
            .map(|path| {
                remap_relative_path(path, &old_relative, &new_relative)
                    .unwrap_or_else(|| path.clone())
            })
            .collect();
        if let Some(selected) = self.explorer.selected_file.as_mut()
            && let Some(path) = remap_relative_path(selected, &old_relative, &new_relative)
        {
            *selected = path;
        }
        match &mut self.preview.target {
            PreviewTarget::VideoFile(path, _)
            | PreviewTarget::AudioFile(path, _)
            | PreviewTarget::ImageFile(path) => {
                if let Some(new_path) = remap_relative_path(path, &old_relative, &new_relative) {
                    *path = new_path;
                }
            }
            PreviewTarget::None | PreviewTarget::Timeline(_) => {}
        }

        let renamed_active_timeline = self
            .timeline
            .as_ref()
            .and_then(|timeline| remap_relative_path(&timeline.path, &old_relative, &new_relative));
        if let Some(renamed_active_timeline) = renamed_active_timeline
            && let Some(timeline) = self.timeline.as_mut()
        {
            timeline.path = renamed_active_timeline;
        }
        if let Some(timeline) = self.timeline.as_ref() {
            timeline.save(&self.global_settings.project_root);
        }
        save_project_local_settings(
            &self.global_settings.project_root,
            self.timeline
                .as_ref()
                .map(|timeline| timeline.path.as_path()),
        )?;

        self.explorer.rename_dialog = None;
        self.explorer.search_query = None;
        self.explorer.search_results.clear();
        self.explorer.search_pending = false;
        self.explorer
            .refresh_file_tree(&self.global_settings.project_root)?;
        self.save_explorer_expansion()?;
        self.schedule_explorer_search(cx);
        self.status = Some(format!(
            "Renamed {} to {}.",
            old_relative.display(),
            new_relative.display()
        ));
        Ok(())
    }

    pub(in crate::editor) fn reveal_selected_file(&mut self, cx: &mut Context<Self>) {
        let Some(path) = file_action_path(
            match &self.context_menu {
                ContextMenu::File(menu) => Some(menu),
                ContextMenu::None | ContextMenu::TimelineClip(_) | ContextMenu::TextTrack(_) => {
                    None
                }
            },
            self.explorer.selected_file.as_deref(),
            &self.global_settings.project_root,
        ) else {
            return;
        };
        self.dismiss_context_menu();
        cx.reveal_path(&path);
    }

    pub(in crate::editor) fn open_selected_file_in_default_app(&mut self, cx: &mut Context<Self>) {
        let Some(path) = file_action_path(
            match &self.context_menu {
                ContextMenu::File(menu) => Some(menu),
                ContextMenu::None | ContextMenu::TimelineClip(_) | ContextMenu::TextTrack(_) => {
                    None
                }
            },
            self.explorer.selected_file.as_deref(),
            &self.global_settings.project_root,
        ) else {
            return;
        };
        self.dismiss_context_menu();
        cx.open_with_system(&path);
    }

    pub(in crate::editor) fn trash_selected_file(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let ContextMenu::File(menu) = &self.context_menu else {
            return Ok(());
        };
        let relative_path = menu.relative_path.clone();
        self.dismiss_context_menu();

        // The project root is the workspace itself, not an entry within it.
        if relative_path.as_os_str().is_empty() {
            return Err("The project folder cannot be moved to Trash here.".to_string());
        }
        if self
            .timeline
            .as_ref()
            .is_some_and(|timeline| timeline.path.starts_with(&relative_path))
        {
            return Err("The active timeline cannot be moved to Trash.".to_string());
        }

        let path = self.global_settings.project_root.join(&relative_path);
        let display_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| relative_path.display().to_string());
        if let Err(error) = move_path_to_trash(&path) {
            self.status = None;
            return Err(format!("Could not move {display_name} to Trash: {error}"));
        }
        self.explorer
            .expanded_directories
            .retain(|directory| !directory.starts_with(&relative_path));
        self.explorer.selected_file = None;
        self.explorer.search_query = None;
        self.explorer.search_results.clear();
        self.explorer.search_pending = false;
        self.explorer
            .refresh_file_tree(&self.global_settings.project_root)?;
        self.save_explorer_expansion()?;
        self.schedule_explorer_search(cx);
        self.status = Some(format!("Moved {display_name} to Trash."));
        Ok(())
    }

    pub(crate) fn action_reveal_in_finder(
        &mut self,
        _: &RevealInFinder,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reveal_selected_file(cx);
        cx.notify();
    }

    pub(crate) fn action_open_in_default_app(
        &mut self,
        _: &OpenInDefaultApp,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_selected_file_in_default_app(cx);
        cx.notify();
    }
}

fn file_action_path(
    context_menu: Option<&FileContextMenu>,
    selected_file: Option<&Path>,
    project_root: &Path,
) -> Option<PathBuf> {
    let relative_path = context_menu
        .map(|menu| menu.relative_path.as_path())
        .or(selected_file)?;
    Some(project_root.join(relative_path))
}
