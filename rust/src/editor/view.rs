use super::*;

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_video_transform_inputs(cx);
        let viewport = window.viewport_size();
        let editor_width =
            (f32::from(viewport.width) - crate::gpui_inspector::docked_width(window)).max(0.0);
        let editor_viewport = gpui::size(px(editor_width), viewport.height);
        let preview_width =
            (editor_width - MEDIA_PANEL_WIDTH - self.properties.width).max(MIN_PREVIEW_WIDTH);
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
                    editor_width,
                    f32::from(viewport.height),
                    cx,
                ));
        }
        let file_menu = self
            .explorer
            .context_menu
            .as_ref()
            .map(|menu| self.file_menu_overlay(menu, editor_viewport, cx));
        let clip_menu = self
            .timeline
            .context_menu
            .as_ref()
            .map(|menu| self.timeline_clip_menu_overlay(menu, editor_viewport, cx));
        let rename_dialog = self
            .explorer
            .rename_dialog
            .as_ref()
            .map(|_| self.rename_dialog(cx));
        let new_timeline_dialog = self
            .explorer
            .new_timeline_dialog
            .as_ref()
            .map(|_| self.new_timeline_dialog(cx));

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
            .on_action(cx.listener(Self::action_copy_selected_clips))
            .on_action(cx.listener(Self::action_cut_selected_clips))
            .on_action(cx.listener(Self::action_paste_clips))
            .on_action(cx.listener(Self::action_select_all_unlocked_clips))
            .on_action(cx.listener(Self::action_activate_selection_tool))
            .on_action(cx.listener(Self::action_activate_blade_tool))
            .on_action(cx.listener(Self::action_activate_trim_tool))
            .on_action(cx.listener(Self::action_toggle_fullscreen))
            .on_action(cx.listener(Self::action_exit_fullscreen))
            .on_action(cx.listener(Self::action_toggle_inspector))
            .on_action(cx.listener(Self::action_reveal_in_finder))
            .on_action(cx.listener(Self::action_open_in_default_app))
            .on_drag_move::<PropertiesPanelResizeDrag>(
                cx.listener(Self::resize_properties_panel_drag),
            )
            .on_mouse_move(cx.listener(Self::update_video_opacity_drag))
            .capture_any_mouse_up(cx.listener(Self::finish_properties_panel_resize))
            .capture_any_mouse_up(cx.listener(Self::finish_video_opacity_drag))
            .capture_any_mouse_down(cx.listener(|editor, event: &MouseDownEvent, window, cx| {
                if event.button == MouseButton::Left {
                    editor.focus_handle.focus(window);
                    if editor.preview.volume_open {
                        editor.preview.volume_open = false;
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
            .when_some(clip_menu, |this, menu| this.child(menu))
            .when_some(rename_dialog, |this, dialog| this.child(dialog))
            .when_some(new_timeline_dialog, |this, dialog| this.child(dialog))
            .when(self.settings_open, |this| {
                this.child(self.settings_modal(cx))
            })
            .when(self.export.dialog.is_some(), |this| {
                this.child(self.export_dialog(cx))
            })
    }
}

impl Editor {
    fn topbar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let undo_enabled = !self.timeline.undo_stack.is_empty();
        let redo_enabled = !self.timeline.redo_stack.is_empty();
        let export_enabled = self.timeline.active_timeline.is_some()
            && !self.project.clips.is_empty()
            && !self.export.running;
        let has_error = self.error.is_some();
        let message = self
            .error
            .as_deref()
            .or(self.status.as_deref())
            .unwrap_or("Ready")
            .to_string();
        let timeline_name = self
            .timeline
            .active_timeline
            .as_deref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "No timeline".to_string());

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
                            .child(format!("EDITOR · {timeline_name} · AUTOSAVED")),
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
                    .child(toolbar_button("New Timeline", true).on_click(cx.listener(
                        |editor, _, window, cx| {
                            editor.begin_create_timeline(PathBuf::new(), window, cx);
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
                            if self.export.running {
                                "Exporting…"
                            } else {
                                "Export MP4"
                            },
                            export_enabled,
                        )
                        .bg(rgb(ACCENT))
                        .text_color(rgb(0x17120a))
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.open_export_dialog(cx);
                            cx.notify();
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
