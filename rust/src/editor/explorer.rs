use super::*;

#[derive(Clone)]
pub(super) struct FileContextMenu {
    relative_path: PathBuf,
    x: f32,
    y: f32,
}

impl Editor {
    pub(super) fn media_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let project_name = self
            .project_root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.project_root.display().to_string());
        let entries = self
            .file_tree
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let path = entry.relative_path.clone();
                let selection_path = path.clone();
                let action_path = path.clone();
                let context_path = path.clone();
                let selected = self.selected_file.as_ref() == Some(&path);
                let is_directory = entry.is_directory;
                let is_video = entry.is_video;
                let is_image = entry.is_image;
                let is_audio = entry.is_audio;
                let is_media = is_video || is_image || is_audio;
                let thumbnail_path = is_image.then(|| self.project_root.join(&path));
                let icon = if is_directory {
                    if entry.expanded { "▾" } else { "▸" }
                } else if is_video {
                    "▶"
                } else if is_audio {
                    "♪"
                } else {
                    "·"
                };
                div()
                    .id(("project-file", index))
                    .h(px(34.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded_md()
                    .pr_2()
                    .pl(px(8.0 + entry.depth as f32 * 16.0))
                    .bg(rgb(if selected { 0x25221c } else { PANEL }))
                    .cursor(CursorStyle::PointingHand)
                    .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                    .on_click(cx.listener(move |editor, _, _, cx| {
                        if is_directory {
                            editor.toggle_directory(selection_path.clone());
                        } else {
                            editor.select_file(selection_path.clone(), cx);
                        }
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                            editor.show_file_context_menu(context_path.clone(), event, cx);
                        }),
                    )
                    .child(
                        div()
                            .size(px(18.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .overflow_hidden()
                            .rounded_sm()
                            .text_color(rgb(if is_media { ACCENT } else { MUTED }))
                            .when_some(thumbnail_path, |this, path| {
                                this.child(img(path).size_full().object_fit(ObjectFit::Cover))
                            })
                            .when(!is_image, |this| this.child(icon)),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .text_sm()
                            .text_ellipsis()
                            .text_color(rgb(if is_media || is_directory {
                                TEXT
                            } else {
                                MUTED
                            }))
                            .child(entry.name.clone()),
                    )
                    .when(is_media, |this| {
                        this.child(
                            div()
                                .id(("add-project-file", index))
                                .size_6()
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .occlude()
                                .bg(rgb(0x25252a))
                                .hover(|style| style.bg(rgb(ACCENT)).text_color(rgb(0x17120a)))
                                .child("+")
                                .on_click(cx.listener(move |editor, _, _, cx| {
                                    editor.add_file_to_timeline(action_path.clone(), cx);
                                })),
                        )
                    })
            })
            .collect::<Vec<_>>();

        div()
            .id("editor-media-panel")
            .w(px(MEDIA_PANEL_WIDTH))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(rgb(BORDER))
            .bg(rgb(PANEL))
            .child(
                div()
                    .h(px(62.0))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap_1()
                    .justify_between()
                    .px_4()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .w_full()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_ellipsis()
                            .child(project_name),
                    )
                    .child(
                        div()
                            .w_full()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(rgb(MUTED))
                            .text_ellipsis()
                            .child(self.project_root.display().to_string()),
                    ),
            )
            .child(
                div()
                    .id("editor-media-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.timeline_vertical_scroll)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .when(entries.is_empty(), |this| {
                        this.child(
                            div()
                                .p_3()
                                .text_sm()
                                .text_color(rgb(MUTED))
                                .child("This project folder is empty."),
                        )
                    })
                    .children(entries),
            )
            .into_any_element()
    }

    pub(super) fn file_menu_overlay(
        &self,
        menu: &FileContextMenu,
        viewport: gpui::Size<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let width = 268.0;
        let height = 92.0;
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
                    ),
            )
            .into_any_element()
    }

    pub(super) fn refresh_file_tree(&mut self) {
        self.last_tree_scan = Instant::now();
        match visible_tree(&self.project_root, &self.expanded_directories) {
            Ok(entries) => self.file_tree = entries,
            Err(error) => self.error = Some(error),
        }
    }

    fn toggle_directory(&mut self, relative_path: PathBuf) {
        if !self.expanded_directories.remove(&relative_path) {
            self.expanded_directories.insert(relative_path);
        }
        self.refresh_file_tree();
    }

    fn show_file_context_menu(
        &mut self,
        relative_path: PathBuf,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        self.selected_file = Some(relative_path.clone());
        self.file_context_menu = Some(FileContextMenu {
            relative_path,
            x: event.position.x.into(),
            y: event.position.y.into(),
        });
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn dismiss_file_context_menu(&mut self) {
        self.file_context_menu = None;
    }

    fn reveal_selected_file(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.file_action_path() else {
            return;
        };
        self.file_context_menu = None;
        cx.reveal_path(&path);
    }

    fn open_selected_file_in_default_app(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.file_action_path() else {
            return;
        };
        self.file_context_menu = None;
        cx.open_with_system(&path);
    }

    fn file_action_path(&self) -> Option<PathBuf> {
        let relative_path = self
            .file_context_menu
            .as_ref()
            .map(|menu| &menu.relative_path)
            .or(self.selected_file.as_ref())?;
        Some(self.project_root.join(relative_path))
    }

    fn add_file_to_timeline(&mut self, relative_path: PathBuf, cx: &mut Context<Self>) {
        if let Some(asset_id) = self
            .project
            .assets
            .iter()
            .find(|asset| asset.path == relative_path)
            .map(|asset| asset.id)
        {
            self.append_asset_clip(asset_id);
            cx.notify();
            return;
        }

        let project_root = self.project_root.clone();
        let absolute_path = project_root.join(&relative_path);
        let is_image = workspace::is_image_path(&relative_path);
        let is_audio = workspace::is_audio_path(&relative_path);
        self.status = Some(format!("Inspecting {}…", relative_path.display()));
        cx.spawn(async move |editor, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    if is_image {
                        probe_image(&absolute_path, 0)
                    } else if is_audio {
                        probe_audio(&absolute_path, 0)
                    } else {
                        probe_media(&absolute_path, 0)
                    }
                })
                .await;
            editor
                .update(cx, |editor, cx| {
                    if editor.project_root != project_root {
                        return;
                    }
                    match result {
                        Ok(mut asset) => {
                            if let Some(asset_id) = editor
                                .project
                                .assets
                                .iter()
                                .find(|asset| asset.path == relative_path)
                                .map(|asset| asset.id)
                            {
                                editor.append_asset_clip(asset_id);
                                editor.status = Some("Added media to timeline.".to_string());
                                cx.notify();
                                return;
                            }
                            editor.checkpoint();
                            asset.id = editor.take_id();
                            asset.path = relative_path.clone();
                            let asset_id = asset.id;
                            editor.project.assets.push(asset);
                            editor.append_asset_clip_without_checkpoint(asset_id);
                            editor.selected_file = Some(relative_path);
                            editor.save_project();
                            editor.status = Some("Added media to timeline.".to_string());
                            editor.error = None;
                        }
                        Err(error) => editor.error = Some(error),
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    pub(super) fn action_reveal_in_finder(
        &mut self,
        _: &RevealInFinder,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.reveal_selected_file(cx);
        cx.notify();
    }

    pub(super) fn action_open_in_default_app(
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
