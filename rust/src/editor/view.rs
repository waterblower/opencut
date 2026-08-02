use super::*;

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let preview_width =
            (f32::from(viewport.width) - MEDIA_PANEL_WIDTH - PROPERTIES_PANEL_WIDTH).max(320.0);
        let preview_height =
            (f32::from(viewport.height) - TOPBAR_HEIGHT - TIMELINE_HEIGHT).max(240.0);

        if window.is_fullscreen() {
            return div()
                .id("editor-fullscreen-preview")
                .key_context(EDITOR_KEY_CONTEXT)
                .track_focus(&self.focus_handle)
                .on_action(cx.listener(Self::action_toggle_playback))
                .on_action(cx.listener(Self::action_step_backward_frame))
                .on_action(cx.listener(Self::action_step_forward_frame))
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
            .key_context(EDITOR_KEY_CONTEXT)
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::action_toggle_playback))
            .on_action(cx.listener(Self::action_step_backward_frame))
            .on_action(cx.listener(Self::action_step_forward_frame))
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
            .capture_any_mouse_down(cx.listener(|editor, event: &MouseDownEvent, window, cx| {
                if event.button == MouseButton::Left {
                    editor.focus_handle.focus(window);
                    if editor.preview_volume_open {
                        editor.preview_volume_open = false;
                        cx.notify();
                    }
                }
            }))
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
                            .child(self.properties_panel(cx)),
                    )
                    .child(self.timeline(cx)),
            )
            .when_some(file_menu, |this, menu| this.child(menu))
            .when(self.settings_open, |this| {
                this.child(self.settings_modal(cx))
            })
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
                    .child(toolbar_button("Settings", true).on_click(cx.listener(
                        |editor, _, _, cx| {
                            editor.settings_open = true;
                            cx.notify();
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
