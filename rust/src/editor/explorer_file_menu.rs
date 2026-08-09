use super::*;

#[derive(Clone)]
pub(crate) struct FileContextMenu {
    relative_path: PathBuf,
    is_directory: bool,
    x: f32,
    y: f32,
}

impl Editor {
    pub(crate) fn file_menu_overlay(
        &self,
        menu: &FileContextMenu,
        viewport: gpui::Size<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let width = 268.0;
        let can_create_timeline = menu.is_directory;
        let can_rename = !menu.relative_path.as_os_str().is_empty();
        let can_trash = can_rename && self.timeline_path.as_ref() != Some(&menu.relative_path);
        let height = 92.0
            + if can_create_timeline { 40.0 } else { 0.0 }
            + if can_rename { 40.0 } else { 0.0 }
            + if can_trash { 40.0 } else { 0.0 };
        let left = menu
            .x
            .clamp(8.0, (f32::from(viewport.width) - width - 8.0).max(8.0));
        let top = menu
            .y
            .clamp(8.0, (f32::from(viewport.height) - height - 8.0).max(8.0));

        div()
            .id("file-context-menu-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _, _, cx| {
                    editor.dismiss_file_context_menu();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|editor, _, _, cx| {
                    editor.dismiss_file_context_menu();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("file-context-menu")
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .w(px(width))
                    .p_1()
                    .flex()
                    .flex_col()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(0x1b1b1e))
                    .shadow_lg()
                    .occlude()
                    .when(can_create_timeline, |this| {
                        let directory = menu.relative_path.clone();
                        this.child(
                            file_menu_item("Create New Timeline", "").on_click(cx.listener(
                                move |editor, _, window, cx| {
                                    editor.begin_create_timeline(directory.clone(), window, cx);
                                },
                            )),
                        )
                    })
                    .child(
                        file_menu_item("Reveal in Finder", "⌥⌘R").on_click(cx.listener(
                            |editor, _, _, cx| {
                                editor.reveal_selected_file(cx);
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        file_menu_item("Open in Default App", "⌃⇧↵").on_click(cx.listener(
                            |editor, _, _, cx| {
                                editor.open_selected_file_in_default_app(cx);
                                cx.notify();
                            },
                        )),
                    )
                    .when(can_rename, |this| {
                        this.child(file_menu_item("Rename", "").on_click(cx.listener(
                            |editor, _, window, cx| {
                                editor.begin_rename(window, cx);
                            },
                        )))
                    })
                    .when(can_trash, |this| {
                        this.child(
                            file_menu_item("Move to Trash", "")
                                .text_color(rgb(ERROR))
                                .on_click(cx.listener(|editor, _, _, cx| {
                                    editor.trash_selected_file(cx);
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .into_any_element()
    }

    pub(crate) fn show_file_context_menu(
        &mut self,
        relative_path: PathBuf,
        is_directory: bool,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        self.explorer.selected_file = Some(relative_path.clone());
        self.explorer.context_menu = Some(FileContextMenu {
            relative_path,
            is_directory,
            x: event.position.x.into(),
            y: event.position.y.into(),
        });
        cx.stop_propagation();
        cx.notify();
    }

    pub(crate) fn dismiss_file_context_menu(&mut self) {
        self.explorer.context_menu = None;
    }

    /// Opens the new-timeline dialog for `relative_directory`, pre-filled with the next
    /// unused default name so the user can accept it without typing.
    pub(crate) fn begin_create_timeline(
        &mut self,
        relative_directory: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.explorer.context_menu = None;
        let default_name =
            timeline_document::default_timeline_name(&self.project_root, &relative_directory);
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
        self.error = None;
        cx.notify();
    }

    pub(crate) fn finish_create_timeline(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.explorer.new_timeline_dialog.as_ref() else {
            return;
        };
        let relative_directory = state.relative_directory.clone();
        let name = state.input.read(cx).query().trim().to_string();
        let (relative_path, project) =
            match timeline_document::create(&self.project_root, &relative_directory, &name) {
                Ok(timeline) => timeline,
                Err(error) => {
                    // Keep the dialog open so the name can be corrected.
                    self.error = Some(format!("Could not create timeline: {error}"));
                    return;
                }
            };
        self.explorer.new_timeline_dialog = None;
        self.activate_created_timeline(relative_directory, relative_path, project, cx);
    }

    pub(crate) fn begin_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(relative_path): Option<PathBuf> = self
            .explorer
            .context_menu
            .as_ref()
            .map(|menu| menu.relative_path.clone())
        else {
            return;
        };
        self.explorer.context_menu = None;
        let Some(name) = relative_path
            .file_name()
            .map(|name: &std::ffi::OsStr| name.to_string_lossy().into_owned())
        else {
            self.error = Some("The project folder cannot be renamed here.".to_string());
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
        self.error = None;
        cx.notify();
    }

    pub(crate) fn finish_rename(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.explorer.rename_dialog.as_ref() else {
            return;
        };
        let old_relative = state.relative_path.clone();
        let new_name = state.input.read(cx).query().trim().to_string();
        let Some(new_relative) = renamed_relative_path(&old_relative, &new_name) else {
            self.error = Some("Enter a single non-empty file or folder name.".to_string());
            return;
        };
        if new_relative == old_relative {
            self.explorer.rename_dialog = None;
            return;
        }

        let old_path = self.project_root.join(&old_relative);
        let new_path = self.project_root.join(&new_relative);
        if new_path.exists() {
            self.error = Some(format!(
                "Cannot rename: {} already exists.",
                new_relative.display()
            ));
            return;
        }
        if let Err(error) = std::fs::rename(&old_path, &new_path) {
            self.error = Some(format!(
                "Could not rename {}: {error}",
                old_relative.display()
            ));
            return;
        }

        for project in std::iter::once(&mut self.project)
            .chain(self.timeline.undo_stack.iter_mut())
            .chain(self.timeline.redo_stack.iter_mut())
        {
            for asset in &mut project.assets {
                if let Some(path) = remap_relative_path(&asset.path, &old_relative, &new_relative) {
                    asset.path = path;
                }
            }
        }
        self.preview.timeline_needs_rebuild = true;
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
            PreviewTarget::VideoFile(path)
            | PreviewTarget::AudioFile(path)
            | PreviewTarget::ImageFile(path) => {
                if let Some(new_path) = remap_relative_path(path, &old_relative, &new_relative) {
                    *path = new_path;
                }
            }
            PreviewTarget::Timeline => {}
        }

        self.error = None;
        if self.timeline_path.as_ref() == Some(&old_relative) {
            self.timeline_path = Some(new_relative.clone());
            if let Err(error) = save_active_timeline(&self.project_root, &new_relative) {
                self.error = Some(error);
            }
            if let Some(view_state) = self.current_timeline_view_state()
                && let Err(error) = save_timeline_view(&view_state)
            {
                self.error = Some(error);
            }
        }
        self.save_project();
        self.explorer.rename_dialog = None;
        self.explorer.search_query = None;
        self.explorer.search_results.clear();
        self.explorer.search_pending = false;
        self.refresh_file_tree();
        self.schedule_explorer_search(cx);
        self.status = Some(format!(
            "Renamed {} to {}.",
            old_relative.display(),
            new_relative.display()
        ));
    }

    fn reveal_selected_file(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.file_action_path() else {
            return;
        };
        self.explorer.context_menu = None;
        cx.reveal_path(&path);
    }

    fn open_selected_file_in_default_app(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.file_action_path() else {
            return;
        };
        self.explorer.context_menu = None;
        cx.open_with_system(&path);
    }

    fn trash_selected_file(&mut self, cx: &mut Context<Self>) {
        let Some(relative_path): Option<PathBuf> = self
            .explorer
            .context_menu
            .as_ref()
            .map(|menu| menu.relative_path.clone())
        else {
            return;
        };
        self.explorer.context_menu = None;

        // The project root is the workspace itself, not an entry within it.
        if relative_path.as_os_str().is_empty() {
            self.error = Some("The project folder cannot be moved to Trash here.".to_string());
            return;
        }

        let path = self.project_root.join(&relative_path);
        let display_name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| relative_path.display().to_string());
        match move_path_to_trash(&path) {
            Ok(()) => {
                self.explorer
                    .expanded_directories
                    .retain(|directory| !directory.starts_with(&relative_path));
                self.explorer.selected_file = None;
                self.explorer.search_query = None;
                self.explorer.search_results.clear();
                self.explorer.search_pending = false;
                self.refresh_file_tree();
                self.schedule_explorer_search(cx);
                self.status = Some(format!("Moved {display_name} to Trash."));
                self.error = None;
            }
            Err(error) => {
                self.status = None;
                self.error = Some(format!("Could not move {display_name} to Trash: {error}"));
            }
        }
    }

    fn file_action_path(&self) -> Option<PathBuf> {
        let relative_path = self
            .explorer
            .context_menu
            .as_ref()
            .map(|menu| &menu.relative_path)
            .or(self.explorer.selected_file.as_ref())?;
        Some(self.project_root.join(relative_path))
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

fn file_menu_item(label: &'static str, shortcut: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(label)
        .h(px(40.0))
        .px_3()
        .flex()
        .items_center()
        .justify_between()
        .rounded_md()
        .cursor(CursorStyle::PointingHand)
        .hover(|style| style.bg(rgb(0x34343a)))
        .child(div().text_sm().child(label))
        .child(
            div()
                .font_family("monospace")
                .text_sm()
                .text_color(rgb(MUTED))
                .child(shortcut),
        )
}
