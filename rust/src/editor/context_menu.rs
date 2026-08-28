use super::*;

pub(super) enum ContextMenu {
    None,
    File(FileContextMenu),
    TimelineClip(TimelineClipContextMenu),
    TextTrack(TextTrackContextMenu),
}

#[derive(Clone)]
pub(crate) struct FileContextMenu {
    pub(super) relative_path: PathBuf,
    is_directory: bool,
    x: f32,
    y: f32,
}

#[derive(Clone, Copy)]
pub(super) struct TimelineClipContextMenu {
    pub(super) clip_id: Ulid,
    x: f32,
    y: f32,
}

impl Editor {
    pub(super) fn context_menu_overlay(
        &self,
        viewport: gpui::Size<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        match &self.context_menu {
            ContextMenu::None => None,
            ContextMenu::File(menu) => Some(self.file_menu_overlay(menu, viewport, cx)),
            ContextMenu::TimelineClip(menu) => {
                Some(self.timeline_clip_menu_overlay(menu, viewport, cx))
            }
            ContextMenu::TextTrack(menu) => Some(self.text_track_menu_overlay(menu, viewport, cx)),
        }
    }

    pub(crate) fn file_menu_overlay(
        &self,
        menu: &FileContextMenu,
        viewport: gpui::Size<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let width = 268.0;
        let can_create_timeline = menu.is_directory;
        let can_open_timeline_settings =
            !menu.is_directory && timeline_document::is_timeline_path(&menu.relative_path);
        let can_rename = !menu.relative_path.as_os_str().is_empty();
        let can_trash = can_rename
            && !self
                .timeline
                .as_ref()
                .is_some_and(|timeline| timeline.path.starts_with(&menu.relative_path));
        let height = 92.0
            + if can_create_timeline { 40.0 } else { 0.0 }
            + if can_open_timeline_settings {
                40.0
            } else {
                0.0
            }
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
                    editor.dismiss_context_menu();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|editor, _, _, cx| {
                    editor.dismiss_context_menu();
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
                    .when(can_open_timeline_settings, |this| {
                        let timeline_path = menu.relative_path.clone();
                        this.child(file_menu_item("Settings", "").on_click(cx.listener(
                            move |editor, _, _, cx| {
                                editor.dismiss_context_menu();
                                if let Err(error) = editor.open_timeline(timeline_path.clone(), cx)
                                {
                                    eprintln!("{error}");
                                    return;
                                }
                                if editor
                                    .timeline
                                    .as_ref()
                                    .is_some_and(|timeline| timeline.path == timeline_path)
                                {
                                    editor.settings_open = true;
                                }
                                cx.notify();
                            },
                        )))
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
                                    if let Err(error) = editor.trash_selected_file(cx) {
                                        eprintln!("{error}");
                                    }
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
        self.context_menu = ContextMenu::File(FileContextMenu {
            relative_path,
            is_directory,
            x: event.position.x.into(),
            y: event.position.y.into(),
        });
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn dismiss_context_menu(&mut self) {
        self.context_menu = ContextMenu::None;
    }

    pub(super) fn take_context_menu(&mut self) -> ContextMenu {
        std::mem::replace(&mut self.context_menu, ContextMenu::None)
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

impl Editor {
    pub(super) fn timeline_clip_menu_overlay(
        &self,
        menu: &TimelineClipContextMenu,
        viewport: gpui::Size<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let width = 292.0;
        let height = 56.0;
        let left = menu
            .x
            .clamp(8.0, (f32::from(viewport.width) - width - 8.0).max(8.0));
        let top = menu
            .y
            .clamp(8.0, (f32::from(viewport.height) - height - 8.0).max(8.0));
        let enabled = self
            .timeline
            .as_ref()
            .and_then(|timeline| transform_targets(&timeline.data, menu.clip_id))
            .is_some_and(|(_, targets)| !targets.is_empty());

        div()
            .id("timeline-clip-context-menu-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _, _, cx| {
                    editor.dismiss_context_menu();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|editor, _, _, cx| {
                    editor.dismiss_context_menu();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("timeline-clip-context-menu")
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .w(px(width))
                    .p_1()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(0x1b1b1e))
                    .shadow_lg()
                    .occlude()
                    .child(
                        div()
                            .id("apply-transform-to-track-clips")
                            .h(px(40.0))
                            .px_3()
                            .flex()
                            .items_center()
                            .rounded_md()
                            .cursor(if enabled {
                                CursorStyle::PointingHand
                            } else {
                                CursorStyle::Arrow
                            })
                            .text_color(rgb(if enabled { TEXT } else { MUTED }))
                            .when(enabled, |this| {
                                this.hover(|style| style.bg(rgb(0x34343a)))
                                    .on_click(cx.listener(|editor, _, _, cx| {
                                        editor.apply_transform_to_track_clips();
                                        cx.notify();
                                    }))
                            })
                            .child(div().text_sm().child("Apply Transforms to All Other Clips")),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn show_timeline_clip_context_menu(
        &mut self,
        clip_id: Ulid,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        self.explorer.selected_file = None;
        self.select_only_clip(Some(clip_id));
        let Some(_) = self.timeline.as_ref() else {
            return;
        };
        self.context_menu = ContextMenu::TimelineClip(TimelineClipContextMenu {
            clip_id,
            x: event.position.x.into(),
            y: event.position.y.into(),
        });
        cx.stop_propagation();
        cx.notify();
    }
}

pub(super) struct TextTrackContextMenu {
    pub(super) track_id: Ulid,
    pub(super) position: TimelineTime,
    pub(super) x: f32,
    pub(super) y: f32,
}

impl Editor {
    pub(super) fn text_track_menu_overlay(
        &self,
        menu: &TextTrackContextMenu,
        viewport: gpui::Size<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let width = 180.0;
        let height = 56.0;
        let left = menu
            .x
            .clamp(8.0, (f32::from(viewport.width) - width - 8.0).max(8.0));
        let top = menu
            .y
            .clamp(8.0, (f32::from(viewport.height) - height - 8.0).max(8.0));
        let track_id = menu.track_id;
        let position = menu.position;

        div()
            .id("text-track-context-menu-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _, _, cx| {
                    editor.dismiss_context_menu();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|editor, _, _, cx| {
                    editor.dismiss_context_menu();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("text-track-context-menu")
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .w(px(width))
                    .p_1()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(0x1b1b1e))
                    .shadow_lg()
                    .occlude()
                    .child(
                        div()
                            .id(gpui::SharedString::from(format!(
                                "add-text-to-track-{}",
                                menu.track_id
                            )))
                            .h(px(40.0))
                            .px_3()
                            .flex()
                            .items_center()
                            .rounded_md()
                            .cursor(CursorStyle::PointingHand)
                            .text_color(rgb(TEXT))
                            .hover(|style| style.bg(rgb(0x34343a)))
                            .on_click(cx.listener(move |editor, _, _, cx| {
                                editor.add_text(track_id, position, cx);
                            }))
                            .child(div().text_sm().child("Add text")),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn show_text_track_context_menu(
        &mut self,
        track_id: Ulid,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        if !timeline
            .data
            .track(track_id)
            .is_some_and(|track| track.kind == TrackKind::Text)
        {
            return;
        }
        let scroll_x: f32 = timeline.h_scroll.offset().x.into();
        let content_x =
            f32::from(event.position.x) - TRACK_HEADER_WIDTH - scroll_x - TIMELINE_PADDING;
        let position = timeline
            .data
            .nearest_time(content_x as f64 / timeline.data.view.pixels_per_second as f64)
            .max(TimelineTime::ZERO);
        self.context_menu = ContextMenu::TextTrack(TextTrackContextMenu {
            track_id,
            position,
            x: event.position.x.into(),
            y: event.position.y.into(),
        });
        cx.stop_propagation();
        cx.notify();
    }
}
