use super::*;

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let preview_width =
            (f32::from(viewport.width) - MEDIA_PANEL_WIDTH - INSPECTOR_WIDTH).max(320.0);
        let preview_height =
            (f32::from(viewport.height) - TOPBAR_HEIGHT - TIMELINE_HEIGHT).max(240.0);

        if window.is_fullscreen() {
            return div()
                .id("editor-fullscreen-preview")
                .track_focus(&self.focus_handle)
                .on_action(cx.listener(Self::action_toggle_playback))
                .on_action(cx.listener(Self::action_toggle_fullscreen))
                .on_action(cx.listener(Self::action_exit_fullscreen))
                .on_action(cx.listener(Self::action_toggle_inspector))
                .size_full()
                .overflow_hidden()
                .bg(rgb(0x000000))
                .text_color(rgb(TEXT))
                .child(self.preview_player(
                    0.0,
                    0.0,
                    f32::from(viewport.width),
                    f32::from(viewport.height),
                    cx,
                ));
        }
        let file_menu = self
            .file_context_menu
            .as_ref()
            .map(|menu| self.file_menu_overlay(menu, viewport, cx));

        div()
            .id("editor-root")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::action_toggle_playback))
            .on_action(cx.listener(Self::action_delete_selected))
            .on_action(cx.listener(Self::action_split_clip))
            .on_action(cx.listener(Self::action_undo))
            .on_action(cx.listener(Self::action_redo))
            .on_action(cx.listener(Self::action_duplicate_selected))
            .on_action(cx.listener(Self::action_add_marker))
            .on_action(cx.listener(Self::action_toggle_fullscreen))
            .on_action(cx.listener(Self::action_exit_fullscreen))
            .on_action(cx.listener(Self::action_toggle_inspector))
            .on_action(cx.listener(Self::action_reveal_in_finder))
            .on_action(cx.listener(Self::action_open_in_default_app))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _, _, cx| {
                    if editor.preview_volume_open {
                        editor.preview_volume_open = false;
                        cx.notify();
                    }
                }),
            )
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgb(BACKGROUND))
            .text_color(rgb(TEXT))
            .child(self.topbar(cx))
            .child(
                div()
                    .id("editor-workspace")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("editor-upper-workspace")
                            .min_w_0()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .child(self.media_panel(cx))
                            .child(self.preview_player(
                                MEDIA_PANEL_WIDTH,
                                TOPBAR_HEIGHT,
                                preview_width,
                                preview_height,
                                cx,
                            ))
                            .child(self.inspector(cx)),
                    )
                    .child(self.timeline(cx)),
            )
            .when_some(file_menu, |this, menu| this.child(menu))
    }
}

impl Editor {
    fn topbar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let undo_enabled = !self.undo_stack.is_empty();
        let redo_enabled = !self.redo_stack.is_empty();
        let export_enabled = !self.project.clips.is_empty() && !self.exporting;
        let has_error = self.error.is_some();
        let message = self
            .error
            .as_deref()
            .or(self.status.as_deref())
            .unwrap_or("Ready")
            .to_string();

        div()
            .id("editor-topbar")
            .h(px(TOPBAR_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px_5()
            .border_b_1()
            .border_color(rgb(BORDER))
            .bg(rgb(PANEL))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("OpenCut"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(rgb(MUTED))
                            .child("EDITOR · AUTOSAVED"),
                    ),
            )
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .px_5()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .text_sm()
                            .text_ellipsis()
                            .text_color(if has_error { rgb(ERROR) } else { rgb(MUTED) })
                            .child(message),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(toolbar_button("Undo", undo_enabled).on_click(cx.listener(
                        |editor, _, _, cx| {
                            editor.undo();
                            cx.notify();
                        },
                    )))
                    .child(toolbar_button("Redo", redo_enabled).on_click(cx.listener(
                        |editor, _, _, cx| {
                            editor.redo();
                            cx.notify();
                        },
                    )))
                    .child(toolbar_button("Open Folder", true).on_click(cx.listener(
                        |editor, _, _, cx| {
                            editor.open_project_folder(cx);
                        },
                    )))
                    .child(
                        toolbar_button(
                            if self.exporting {
                                "Exporting…"
                            } else {
                                "Export MP4"
                            },
                            export_enabled,
                        )
                        .bg(rgb(ACCENT))
                        .text_color(rgb(0x17120a))
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.export(cx);
                        })),
                    ),
            )
            .into_any_element()
    }

    fn media_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
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

    fn file_menu_overlay(
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

    fn inspector(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let selected = self.selected_clip_id.and_then(|id| {
            let index = self.project.clip_index(id)?;
            let clip = &self.project.clips[index];
            let asset = clip
                .asset_id
                .and_then(|asset_id| self.project.asset(asset_id));
            let track = self.project.track(clip.track_id)?;
            Some((clip, asset, track))
        });

        div()
            .id("editor-inspector")
            .w(px(INSPECTOR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(rgb(BORDER))
            .bg(rgb(PANEL))
            .child(
                div()
                    .h(px(52.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px_4()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Inspector"),
            )
            .child(
                div()
                    .id("editor-inspector-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .when_some(selected, |this, (clip, asset, track)| {
                        let title = asset
                            .map(|asset| asset.name.clone())
                            .unwrap_or_else(|| "Missing media".to_string());
                        this.flex()
                            .flex_col()
                            .gap_4()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(title),
                            )
                            .child(inspector_value(
                                "Timeline start",
                                format_time(clip.timeline_start),
                            ))
                            .child(inspector_value("Source in", format_time(clip.source_in)))
                            .child(inspector_value("Source out", format_time(clip.source_out)))
                            .child(inspector_value(
                                "Clip duration",
                                format_time(clip.duration()),
                            ))
                            .child(inspector_value("Track", track.name.clone()))
                            .when_some(asset, |this, asset| {
                                this.child(inspector_value("Source", asset_description(asset)))
                            })
                            .child(
                                div()
                                    .mt_2()
                                    .flex()
                                    .gap_2()
                                    .child(panel_button("Nudge left").on_click(cx.listener(
                                        |editor, _, _, cx| {
                                            editor.move_selected(-1);
                                            cx.notify();
                                        },
                                    )))
                                    .child(panel_button("Nudge right").on_click(cx.listener(
                                        |editor, _, _, cx| {
                                            editor.move_selected(1);
                                            cx.notify();
                                        },
                                    ))),
                            )
                            .child(panel_button("Split at playhead").on_click(cx.listener(
                                |editor, _, _, cx| {
                                    editor.split_selected();
                                    cx.notify();
                                },
                            )))
                            .child(panel_button("Duplicate clip").on_click(cx.listener(
                                |editor, _, _, cx| {
                                    editor.duplicate_selected();
                                    cx.notify();
                                },
                            )))
                            .child(panel_button("Delete clip").text_color(rgb(ERROR)).on_click(
                                cx.listener(|editor, _, _, cx| {
                                    editor.delete_selected();
                                    cx.notify();
                                }),
                            ))
                    })
                    .when(selected.is_none(), |this| {
                        this.text_sm()
                            .text_color(rgb(MUTED))
                            .child("Select a timeline clip to inspect it.")
                    }),
            )
            .into_any_element()
    }
}

fn toolbar_button(label: &'static str, enabled: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(label)
        .h_9()
        .px_3()
        .flex()
        .items_center()
        .justify_center()
        .rounded_lg()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(SURFACE))
        .cursor(if enabled {
            CursorStyle::PointingHand
        } else {
            CursorStyle::Arrow
        })
        .text_sm()
        .text_color(rgb(if enabled { TEXT } else { 0x55555d }))
        .when(enabled, |this| {
            this.hover(|style| style.bg(rgb(SURFACE_HOVER)))
        })
        .child(label.to_string())
}

fn panel_button(label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(label)
        .h_9()
        .px_3()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .rounded_lg()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(SURFACE))
        .cursor(CursorStyle::PointingHand)
        .text_sm()
        .hover(|style| style.bg(rgb(SURFACE_HOVER)))
        .child(label.to_string())
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

fn inspector_value(label: &str, value: String) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(rgb(MUTED))
                .child(label.to_string()),
        )
        .child(div().text_sm().child(value))
}

fn asset_description(asset: &MediaAsset) -> String {
    match asset.kind {
        MediaKind::Image => format!("{} image · {}×{}", asset.codec, asset.width, asset.height),
        MediaKind::Audio => format!("{} audio", asset.codec),
        MediaKind::Video => format!(
            "{} · {}×{} · {:.2} fps",
            asset.codec, asset.width, asset.height, asset.framerate
        ),
    }
}
