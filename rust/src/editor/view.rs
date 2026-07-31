use super::*;

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let preview_width =
            (f32::from(viewport.width) - MEDIA_PANEL_WIDTH - INSPECTOR_WIDTH).max(320.0);
        let preview_height =
            (f32::from(viewport.height) - TOPBAR_HEIGHT - TIMELINE_HEIGHT).max(240.0);
        let file_menu = self
            .file_context_menu
            .as_ref()
            .map(|menu| self.file_menu_overlay(menu, viewport, cx));
        let selected_image_path = self
            .selected_file
            .as_ref()
            .filter(|path| workspace::is_image_path(path))
            .map(|path| self.project_root.join(path));
        let timeline_image_path = self.loaded_clip_id.and_then(|clip_id| {
            let clip = self.project.clip(clip_id)?;
            let asset = self.project.asset(clip.asset_id)?;
            (asset.kind == MediaKind::Image).then(|| self.project_root.join(&asset.path))
        });
        let preview = if let Some(image_path) = selected_image_path {
            img(image_path)
                .id("editor-selected-image-preview")
                .w(px(preview_width))
                .h(px(preview_height))
                .object_fit(ObjectFit::Contain)
                .into_any_element()
        } else if let Some(video_handle) = &self.video {
            video(video_handle.clone())
                .id("editor-preview-video")
                .size(px(preview_width), px(preview_height))
                .buffer_capacity(3)
                .into_any_element()
        } else if let Some(image_path) = timeline_image_path {
            img(image_path)
                .id("editor-preview-image")
                .w(px(preview_width))
                .h(px(preview_height))
                .object_fit(ObjectFit::Contain)
                .into_any_element()
        } else {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(MUTED))
                .child("Choose a video from the project folder to begin")
                .into_any_element()
        };

        div()
            .id("editor-root")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::action_toggle_playback))
            .on_action(cx.listener(Self::action_delete_selected))
            .on_action(cx.listener(Self::action_split_clip))
            .on_action(cx.listener(Self::action_undo))
            .on_action(cx.listener(Self::action_redo))
            .on_action(cx.listener(Self::action_reveal_in_finder))
            .on_action(cx.listener(Self::action_open_in_default_app))
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
                    .id("editor-media-scroll")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.media_panel(cx))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .h_full()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .id("editor-preview")
                                    .min_h_0()
                                    .flex_1()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .overflow_hidden()
                                    .bg(rgb(0x000000))
                                    .cursor(CursorStyle::PointingHand)
                                    .on_click(cx.listener(|editor, _, _, cx| {
                                        editor.toggle_playback();
                                        cx.notify();
                                    }))
                                    .child(preview),
                            )
                            .child(self.timeline(cx)),
                    )
                    .child(self.inspector(cx)),
            )
            .when_some(file_menu, |this, menu| this.child(menu))
    }
}

impl Editor {
    fn topbar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let undo_enabled = !self.undo_stack.is_empty();
        let redo_enabled = !self.redo_stack.is_empty();
        let export_enabled = !self.project.timeline.is_empty() && !self.exporting;
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
                let is_media = is_video || is_image;
                let thumbnail_path = is_image.then(|| self.project_root.join(&path));
                let icon = if is_directory {
                    if entry.expanded { "▾" } else { "▸" }
                } else if is_video {
                    "▶"
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
                            editor.select_file(selection_path.clone());
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
            let clip = &self.project.timeline[index];
            let asset = self.project.asset(clip.asset_id)?;
            Some((index, clip, asset))
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
                    .when_some(selected, |this, (index, clip, asset)| {
                        this.flex()
                            .flex_col()
                            .gap_4()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(asset.name.clone()),
                            )
                            .child(inspector_value(
                                "Timeline start",
                                format_time(self.project.timeline_start(index)),
                            ))
                            .child(inspector_value("Source in", format_time(clip.source_in)))
                            .child(inspector_value("Source out", format_time(clip.source_out)))
                            .child(inspector_value(
                                "Clip duration",
                                format_time(clip.duration()),
                            ))
                            .child(inspector_value("Source", asset_description(asset)))
                            .child(
                                div()
                                    .mt_2()
                                    .flex()
                                    .gap_2()
                                    .child(panel_button("Move left").on_click(cx.listener(
                                        |editor, _, _, cx| {
                                            editor.move_selected(-1);
                                            cx.notify();
                                        },
                                    )))
                                    .child(panel_button("Move right").on_click(cx.listener(
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

    fn timeline(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let timeline_width = (self.project.timeline_duration() as f32 * self.pixels_per_second
            + TIMELINE_PADDING * 2.0)
            .max(640.0);
        let clip_elements = self
            .project
            .timeline
            .iter()
            .enumerate()
            .map(|(index, clip)| {
                let clip_id = clip.id;
                let selected = self.selected_clip_id == Some(clip_id);
                let asset_name = self
                    .project
                    .asset(clip.asset_id)
                    .map(|asset| asset.name.clone())
                    .unwrap_or_else(|| "Missing media".to_string());
                let width = (clip.duration() as f32 * self.pixels_per_second).max(2.0);
                div()
                    .id(("timeline-clip", index))
                    .relative()
                    .w(px(width))
                    .h(px(86.0))
                    .flex_shrink_0()
                    .overflow_hidden()
                    .rounded_lg()
                    .border_2()
                    .border_color(rgb(if selected { ACCENT } else { 0x3a6695 }))
                    .bg(rgb(CLIP_BLUE))
                    .cursor(CursorStyle::PointingHand)
                    .on_click(cx.listener(move |editor, _, _, cx| {
                        editor.select_clip(clip_id);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .flex_col()
                            .justify_between()
                            .p_3()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_ellipsis()
                                    .child(asset_name),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_family("monospace")
                                    .text_color(rgb(0xb9cee5))
                                    .child(format!(
                                        "{} – {}",
                                        format_time(clip.source_in),
                                        format_time(clip.source_out)
                                    )),
                            ),
                    )
                    .child(trim_handle(("left-trim", index), true).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                            editor.begin_trim(clip_id, TrimEdge::Left, event.position.x.into());
                            cx.notify();
                        }),
                    ))
                    .child(trim_handle(("right-trim", index), false).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                            editor.begin_trim(clip_id, TrimEdge::Right, event.position.x.into());
                            cx.notify();
                        }),
                    ))
            })
            .collect::<Vec<_>>();
        let playhead_left = TIMELINE_PADDING + self.playhead as f32 * self.pixels_per_second;

        div()
            .id("editor-timeline")
            .h(px(TIMELINE_HEIGHT))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(rgb(BORDER))
            .bg(rgb(0x0a0a0c))
            .on_mouse_move(cx.listener(Self::update_trim))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_trim))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_trim))
            .child(
                div()
                    .h(px(TIMELINE_HEADER_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .id("timeline-play")
                                    .size_8()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .bg(rgb(SURFACE))
                                    .cursor(CursorStyle::PointingHand)
                                    .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                                    .child(if self.playing { "Ⅱ" } else { "▶" })
                                    .on_click(cx.listener(|editor, _, _, cx| {
                                        editor.toggle_playback();
                                        cx.notify();
                                    })),
                            )
                            .child(div().font_family("monospace").text_sm().child(format!(
                                "{} / {}",
                                format_time(self.playhead),
                                format_time(self.project.timeline_duration())
                            )))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MUTED))
                                    .child("Video, audio, and still images"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(panel_button("−").on_click(cx.listener(|editor, _, _, cx| {
                                editor.zoom(0.8);
                                cx.notify();
                            })))
                            .child(
                                div()
                                    .w(px(58.0))
                                    .text_center()
                                    .font_family("monospace")
                                    .text_xs()
                                    .text_color(rgb(MUTED))
                                    .child(format!("{:.0}px/s", self.pixels_per_second)),
                            )
                            .child(panel_button("+").on_click(cx.listener(|editor, _, _, cx| {
                                editor.zoom(1.25);
                                cx.notify();
                            }))),
                    ),
            )
            .child(
                div()
                    .id("editor-timeline-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_x_scroll()
                    .track_scroll(&self.timeline_scroll)
                    .child(
                        div()
                            .relative()
                            .w(px(timeline_width))
                            .h_full()
                            .pt_8()
                            .px(px(TIMELINE_PADDING))
                            .child(
                                div()
                                    .id("timeline-seek-ruler")
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .w_full()
                                    .h(px(24.0))
                                    .cursor(CursorStyle::PointingHand)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|editor, event: &MouseDownEvent, _, cx| {
                                            editor.seek_from_timeline_x(event.position.x.into());
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(div().h(px(86.0)).flex().gap_0().children(clip_elements))
                            .child(
                                div()
                                    .absolute()
                                    .top_3()
                                    .bottom_3()
                                    .left(px(playhead_left))
                                    .w(px(2.0))
                                    .bg(rgb(ACCENT))
                                    .child(
                                        div()
                                            .absolute()
                                            .top_0()
                                            .left(px(-5.0))
                                            .w(px(12.0))
                                            .h(px(8.0))
                                            .bg(rgb(ACCENT)),
                                    ),
                            ),
                    ),
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
    if asset.kind == MediaKind::Image {
        format!("{} image · {}×{}", asset.codec, asset.width, asset.height)
    } else {
        format!(
            "{} · {}×{} · {:.2} fps",
            asset.codec, asset.width, asset.height, asset.framerate
        )
    }
}

fn trim_handle(id: impl Into<gpui::ElementId>, left: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .absolute()
        .top_0()
        .bottom_0()
        .when(left, |this| this.left_0())
        .when(!left, |this| this.right_0())
        .w(px(10.0))
        .bg(rgb(ACCENT))
        .opacity(0.72)
        .cursor(CursorStyle::ResizeLeftRight)
        .occlude()
        .hover(|style| style.opacity(1.0))
}
