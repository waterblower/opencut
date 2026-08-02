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
        let filter = self.explorer_filter_query.trim().to_lowercase();
        let entries = self
            .file_tree
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                filter.is_empty()
                    || entry
                        .relative_path
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&filter)
            })
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
                let metadata = explorer_metadata(
                    entry,
                    self.project.assets.iter().find(|asset| asset.path == path),
                );
                div()
                    .id(("project-file", index))
                    .relative()
                    .h(px(38.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .pr_2()
                    .pl(px(10.0 + entry.depth as f32 * 16.0))
                    .bg(rgb(if selected { 0x1e1b13 } else { PANEL }))
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
                    .when(selected, |this| {
                        this.child(
                            div()
                                .absolute()
                                .left_0()
                                .top_0()
                                .bottom_0()
                                .w(px(2.0))
                                .bg(rgb(ACCENT)),
                        )
                    })
                    .child(
                        div()
                            .w(px(if is_directory { 14.0 } else { 38.0 }))
                            .h(px(20.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(is_directory, |this| {
                                this.text_color(rgb(MUTED)).child(if entry.expanded {
                                    "▾"
                                } else {
                                    "▸"
                                })
                            })
                            .when(!is_directory, |this| this.child(explorer_file_badge(entry))),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .text_sm()
                            .font_family("monospace")
                            .text_ellipsis()
                            .text_color(rgb(if is_media || is_directory {
                                TEXT
                            } else {
                                MUTED
                            }))
                            .child(entry.name.clone()),
                    )
                    .when(is_media && selected, |this| {
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
                                .text_color(rgb(MUTED))
                                .hover(|style| style.bg(rgb(ACCENT)).text_color(rgb(0x17120a)))
                                .child("+")
                                .on_click(cx.listener(move |editor, _, _, cx| {
                                    editor.add_file_to_timeline(action_path.clone(), cx);
                                    cx.stop_propagation();
                                })),
                        )
                    })
                    .when_some(metadata, |this, metadata| {
                        this.child(
                            div()
                                .max_w(px(58.0))
                                .flex_shrink_0()
                                .font_family("monospace")
                                .text_xs()
                                .text_ellipsis()
                                .text_color(rgb(0x55555e))
                                .child(metadata),
                        )
                    })
            })
            .collect::<Vec<_>>();

        div()
            .id("editor-media-panel")
            .w(px(MEDIA_PANEL_WIDTH))
            .h_full()
            .flex_shrink_0()
            .border_r_1()
            .border_color(rgb(BORDER))
            .bg(rgb(PANEL))
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .bg(rgb(PANEL))
                    .child(
                        div()
                            .h(px(52.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .border_b_1()
                            .border_color(rgb(BORDER))
                            .child(div().text_color(rgb(MUTED)).child("▾"))
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .font_family("monospace")
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_ellipsis()
                                    .child(project_name),
                            )
                            .child(
                                explorer_header_button("refresh-project-tree", "↻").on_click(
                                    cx.listener(|editor, _, _, cx| {
                                        editor.refresh_file_tree();
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                explorer_header_button("collapse-project-tree", "↤").on_click(
                                    cx.listener(|editor, _, _, cx| {
                                        editor.expanded_directories.clear();
                                        editor.refresh_file_tree();
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(explorer_header_button("project-tree-menu", "•••").on_click(
                                cx.listener(|editor, _, _, cx| {
                                    editor.open_project_folder(cx);
                                }),
                            )),
                    )
                    .child(self.explorer_filter(cx))
                    .child(
                        div()
                            .id("editor-media-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.explorer_scroll)
                            .flex()
                            .flex_col()
                            .py_2()
                            .when(entries.is_empty(), |this| {
                                this.child(div().p_4().text_sm().text_color(rgb(MUTED)).child(
                                    if self.explorer_filter_query.is_empty() {
                                        "This project folder is empty.".to_string()
                                    } else {
                                        format!("No files match “{}”.", self.explorer_filter_query)
                                    },
                                ))
                            })
                            .children(entries),
                    ),
            )
            .into_any_element()
    }

    fn explorer_filter(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        div()
            .h(px(58.0))
            .flex_shrink_0()
            .p_2()
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .id("explorer-filter")
                    .h_full()
                    .w_full()
                    .key_context("ExplorerFilter")
                    .track_focus(&self.explorer_filter_focus)
                    .flex()
                    .items_center()
                    .px_3()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(SURFACE))
                    .cursor(CursorStyle::IBeam)
                    .focus(|style| style.border_color(rgb(0x52779a)))
                    .on_click(cx.listener(|editor, _, window, cx| {
                        editor.explorer_filter_focus.focus(window);
                        cx.stop_propagation();
                    }))
                    .on_key_down(cx.listener(Self::handle_explorer_filter_key))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .font_family("monospace")
                            .text_sm()
                            .text_ellipsis()
                            .text_color(rgb(if self.explorer_filter_query.is_empty() {
                                MUTED
                            } else {
                                TEXT
                            }))
                            .child(if self.explorer_filter_query.is_empty() {
                                "Filter files…".to_string()
                            } else {
                                self.explorer_filter_query.clone()
                            }),
                    )
                    .when(!self.explorer_filter_query.is_empty(), |this| {
                        this.child(
                            div()
                                .id("clear-explorer-filter")
                                .size_5()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .cursor(CursorStyle::PointingHand)
                                .text_color(rgb(MUTED))
                                .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                                .child("×")
                                .on_click(cx.listener(|editor, _, _, cx| {
                                    editor.explorer_filter_query.clear();
                                    cx.stop_propagation();
                                    cx.notify();
                                })),
                        )
                    }),
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

    fn handle_explorer_filter_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event.keystroke.key.as_str() {
            "escape" | "enter" => {
                if event.keystroke.key == "escape" {
                    self.explorer_filter_query.clear();
                }
                self.focus_handle.focus(window);
            }
            "backspace" => {
                if event.keystroke.modifiers.platform {
                    self.explorer_filter_query.clear();
                } else {
                    self.explorer_filter_query.pop();
                }
            }
            _ if !event.keystroke.modifiers.control
                && !event.keystroke.modifiers.platform
                && !event.keystroke.modifiers.function =>
            {
                if let Some(text) = event.keystroke.key_char.as_deref()
                    && !text.chars().any(char::is_control)
                {
                    self.explorer_filter_query.push_str(text);
                }
            }
            _ => return,
        }
        cx.stop_propagation();
        cx.notify();
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

fn explorer_header_button(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h_6()
        .min_w(px(22.0))
        .px_1()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .cursor(CursorStyle::PointingHand)
        .font_family("monospace")
        .text_xs()
        .text_color(rgb(MUTED))
        .hover(|style| style.bg(rgb(SURFACE_HOVER)).text_color(rgb(TEXT)))
        .child(label)
}

fn explorer_file_badge(entry: &FileTreeEntry) -> gpui::Div {
    let extension = entry
        .relative_path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            extension
                .chars()
                .take(4)
                .collect::<String>()
                .to_ascii_uppercase()
        })
        .filter(|extension| !extension.is_empty())
        .unwrap_or_else(|| "FILE".to_string());
    let (text, border) = if entry.is_video {
        (0x8fb9dd, 0x355b78)
    } else if entry.is_audio {
        (0x7fd0ae, 0x32725a)
    } else if entry.is_image {
        (0xc3a9e8, 0x665184)
    } else {
        (0x8b8b94, 0x46464e)
    };

    div()
        .w_full()
        .h(px(18.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .border_1()
        .border_color(rgb(border))
        .font_family("monospace")
        .text_xs()
        .text_color(rgb(text))
        .child(extension)
}

fn explorer_metadata(entry: &FileTreeEntry, asset: Option<&MediaAsset>) -> Option<String> {
    if let Some(asset) = asset
        && asset.kind != MediaKind::Image
        && asset.duration.is_finite()
        && asset.duration > 0.0
    {
        return Some(format_explorer_duration(asset.duration));
    }
    entry.size_bytes.map(format_file_size)
}

fn format_explorer_duration(seconds: f64) -> String {
    let total = seconds.round().max(0.0) as u64;
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

fn format_file_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.1} GB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.1} MB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.0} KB", bytes / KIB)
    } else {
        format!("{bytes:.0} B")
    }
}
