use crate::actions::EDITOR_KEY_CONTEXT;
use crate::asset::IconName;
use crate::dialog_missing_media::missing_media_dialog_view;
use crate::editor::Editor;
use crate::event_bus::{AppEvent, EventBus};
use crate::layout::{
    MIN_MEDIA_PANEL_WIDTH, MIN_PREVIEW_WIDTH, MIN_PROPERTY_PANEL_WIDTH, MIN_TIMELINE_HEIGHT,
    MIN_UPPER_WORKSPACE_HEIGHT, TOPBAR_HEIGHT,
};
use crate::project_picker;
use crate::property_panel::property_panel_view;
use crate::settings::settings_modal_view;
use crate::theme::{BACKGROUND, BORDER, MUTED, PANEL, TEXT};
use crate::ui_components::cyberpunk_button::{
    button_cyberpunk_primary, button_cyberpunk_secondary,
};
use crate::ui_components::{
    HorizontalSplit, HorizontalSplitConstraints, VerticalSplit, VerticalSplitConstraints,
};
use gpui::prelude::*;
use gpui::{App, CursorStyle, MouseButton, MouseDownEvent, Window, canvas, div, img, px, rgb};

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.sync_video_transform_inputs(cx);

        let viewport = window.viewport_size();
        let editor_width =
            (f32::from(viewport.width) - crate::gpui_inspector::docked_width(window)).max(0.0);
        let editor_viewport = gpui::size(px(editor_width), viewport.height);
        let workspace_height = (f32::from(viewport.height) - TOPBAR_HEIGHT).max(0.0); // 顶栏以下、上半区与时间线共用的高度。
        let workspace_heights = self
            .workspace_split_state
            .read(cx)
            .heights(workspace_height, WORKSPACE_SPLIT_CONSTRAINTS);

        if self.preview.fullscreen {
            return div()
                .id("editor-fullscreen-preview")
                .key_context(EDITOR_KEY_CONTEXT)
                .on_mouse_move(cx.listener(Self::move_preview_text))
                .on_mouse_move(cx.listener(Self::move_clip_trim))
                .on_mouse_up(MouseButton::Left, cx.listener(Self::release_clip_trim))
                .on_mouse_up_out(MouseButton::Left, cx.listener(Self::release_clip_trim))
                .on_mouse_up(MouseButton::Left, cx.listener(Self::release_preview_text))
                .on_mouse_up_out(MouseButton::Left, cx.listener(Self::release_preview_text))
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
                .child(self.preview_player(editor_width, f32::from(viewport.height), cx));
        }
        let placement_cursor = self
            .timeline
            .as_ref()
            .and_then(|timeline| timeline.interaction.clip_move_drag.as_ref())
            .filter(|drag| drag.changed)
            .map(|drag| {
                if drag.invalid_reason.is_some() {
                    CursorStyle::OperationNotAllowed
                } else {
                    CursorStyle::ClosedHand
                }
            });
        let context_menu = self.context_menu_overlay(editor_viewport, cx);
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
        let missing_media_dialog = self
            .missing_media_dialog
            .as_ref()
            .map(|dialog| missing_media_dialog_view(dialog, self.event_bus.clone()));
        let settings_modal = if self.settings_open
            && let Some(timeline) = self.timeline.as_ref()
        {
            Some(settings_modal_view(&timeline.editing_state, cx))
        } else {
            None
        };

        div()
            .id("editor-root")
            .key_context(EDITOR_KEY_CONTEXT)
            .on_mouse_move(cx.listener(Self::move_preview_text))
            .on_mouse_move(cx.listener(Self::move_clip_trim))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::release_clip_trim))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::release_clip_trim))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::release_preview_text))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::release_preview_text))
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
            .on_action(cx.listener(Self::action_select_clips_right_of_pointer))
            .on_action(cx.listener(Self::action_select_clips_left_of_pointer))
            .on_action(cx.listener(Self::action_activate_selection_tool))
            .on_action(cx.listener(Self::action_activate_blade_tool))
            .on_action(cx.listener(Self::action_toggle_fullscreen))
            .on_action(cx.listener(Self::action_exit_fullscreen))
            .on_action(cx.listener(Self::action_toggle_inspector))
            .on_action(cx.listener(Self::action_reveal_in_finder))
            .on_action(cx.listener(Self::action_open_in_default_app))
            .capture_any_mouse_down(cx.listener(|editor, event: &MouseDownEvent, window, cx| {
                if event.button == MouseButton::Left {
                    editor.focus_handle.focus(window, cx);
                }
            }))
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgb(BACKGROUND))
            .text_color(rgb(TEXT))
            .child(self.topbar(window, cx))
            .child(VerticalSplit::new(
                "editor-workspace",
                self.workspace_split_state.clone(),
                self.event_bus.clone(),
                workspace_height,
                WORKSPACE_SPLIT_CONSTRAINTS,
                self.upper_workspace(editor_width, workspace_heights.top, cx),
                self.timeline_view(editor_width, cx),
            ))
            .when_some(context_menu, |this, menu| this.child(menu))
            .when_some(rename_dialog, |this, dialog| this.child(dialog))
            .when_some(new_timeline_dialog, |this, dialog| this.child(dialog))
            .when(self.explorer.trash_timeline_dialog_open, |this| {
                this.child(self.trash_timeline_dialog_view(cx))
            })
            .when_some(missing_media_dialog, |this, dialog| this.child(dialog))
            .when_some(settings_modal, |this, modal| this.child(modal))
            .when(self.global_settings_input.is_some(), |this| {
                this.child(self.global_settings_dialog(cx))
            })
            .when_some(placement_cursor, |this, cursor| {
                this.child(
                    canvas(
                        |_, _, _| {},
                        move |_, (), window, _| {
                            window.set_window_cursor_style(cursor);
                        },
                    )
                    .absolute()
                    .inset_0()
                    .size_full(),
                )
            })
    }
}

// Public APIs only
impl Editor {
    /// The height of the timeline panel at the bottom of the window, as laid out by `render`.
    pub(super) fn timeline_height(&self, window: &Window, cx: &App) -> f32 {
        let workspace_height = (f32::from(window.viewport_size().height) - TOPBAR_HEIGHT).max(0.0);
        self.workspace_split_state
            .read(cx)
            .heights(workspace_height, WORKSPACE_SPLIT_CONSTRAINTS)
            .bottom
    }
}

impl Editor {
    fn upper_workspace(
        &self,
        width: f32,
        preview_height: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let constraints = HorizontalSplitConstraints {
            min_left: MIN_MEDIA_PANEL_WIDTH,
            min_center: MIN_PREVIEW_WIDTH,
            min_right: MIN_PROPERTY_PANEL_WIDTH,
        };
        let widths = self.upper_split_state.read(cx).widths(width, constraints);
        let right_panel_view = property_panel_view(&self.property_panel);
        HorizontalSplit::new(
            "editor-upper-workspace",
            self.upper_split_state.clone(),
            self.event_bus.clone(),
            width,
            constraints,
            self.explorer_panel(cx),
            self.preview_player(widths.center, preview_height, cx),
            right_panel_view,
        )
        .into_any_element()
    }

    fn topbar(&self, window: &mut Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let timeline_name = self
            .timeline
            .as_ref()
            .map(|timeline| timeline.path.as_path())
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "No timeline".to_string());

        let export_enabled = self.timeline.is_some();

        div()
            .id("editor-topbar")
            .h(px(TOPBAR_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .px_5()
            .border_b_1()
            .border_color(rgb(BORDER))
            .bg(rgb(PANEL))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_shrink_0()
                            .items_center()
                            .gap_1()
                            .child(img(IconName::App.path()).size(px(40.0)).flex_shrink_0())
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child("OpenCut"),
                            ),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .text_ellipsis()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(rgb(MUTED))
                            .child(format!("EDITOR · {timeline_name} · AUTOSAVED")),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .gap_3()
                    .child(
                        button_cyberpunk_secondary("topbar-settings", "Settings", true, window, cx)
                            .w(px(128.0))
                            .px_0()
                            .on_click(cx.listener(|editor, _, window, cx| {
                                editor.open_global_settings(window, cx)
                            })),
                    )
                    .child(
                        button_cyberpunk_secondary(
                            "topbar-open-folder",
                            "Open Folder",
                            true,
                            window,
                            cx,
                        )
                        .w(px(152.0))
                        .px_0()
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.open_project_folder(cx);
                        })),
                    )
                    .child(
                        button_cyberpunk_secondary("topbar-projects", "Projects", true, window, cx)
                            .w(px(128.0))
                            .px_0()
                            .on_click(cx.listener(|_, _, _, cx| {
                                let event_bus = cx.new(|_| EventBus {});
                                if let Err(error) = project_picker::open(cx, event_bus, None) {
                                    log::error!("Could not open project picker: {error:?}");
                                }
                            })),
                    )
                    .child(
                        button_cyberpunk_primary(
                            "topbar-export",
                            "Export",
                            export_enabled,
                            window,
                            cx,
                        )
                        .w(px(128.0))
                        .pl(px(20.0))
                        .pr(px(34.0))
                        .justify_center()
                        .when(export_enabled, |button| {
                            button.on_click(cx.listener(|editor, _, _, cx| {
                                let Some(timeline) = editor.timeline.as_ref() else {
                                    return;
                                };
                                let timeline_path = timeline.path.clone();
                                editor.emit_event(cx, AppEvent::OpenExportWindow { timeline_path });
                            }))
                        }),
                    ),
            )
            .into_any_element()
    }
}

const WORKSPACE_SPLIT_CONSTRAINTS: VerticalSplitConstraints = VerticalSplitConstraints {
    min_top: MIN_UPPER_WORKSPACE_HEIGHT,
    min_bottom: MIN_TIMELINE_HEIGHT,
};
