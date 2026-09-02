use std::path::PathBuf;

use gpui::{
    Context, CursorStyle, InteractiveElement, IntoElement, MouseButton, MouseDownEvent,
    ParentElement, StatefulInteractiveElement, Styled, div, prelude::FluentBuilder, px, rgb,
};

use crate::editor::{Editor, MUTED, PANEL, SURFACE_HOVER};

impl Editor {
    pub(super) fn explorer_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let project_name = self
            .global_settings
            .project_root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.global_settings.project_root.display().to_string());
        let filter_query = self.explorer.filter.read(cx).query().to_string();
        let filter = filter_query.trim().to_lowercase();
        let show_root_contents = self.explorer.root_expanded || !filter.is_empty();

        let root_context_path = PathBuf::new();
        let root_row = div()
            .id("project-root")
            .h(px(38.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .cursor(CursorStyle::PointingHand)
            .hover(|style| style.bg(rgb(SURFACE_HOVER)))
            .on_click(cx.listener(|editor, _, _, cx| {
                editor.explorer.root_expanded = !editor.explorer.root_expanded;
                if let Err(error) = editor.save_explorer_expansion() {
                    eprintln!("Could not save explorer expansion: {error}");
                }
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                    editor.show_file_context_menu(root_context_path.clone(), true, event, cx);
                }),
            )
            .child(
                div()
                    .w(px(14.0))
                    .flex_shrink_0()
                    .text_color(rgb(MUTED))
                    .child(if show_root_contents { "▾" } else { "▸" }),
            )
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .font_family("monospace")
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_ellipsis()
                    .child(project_name),
            );

        let entries = {
            let visible_entries = if filter.is_empty() {
                &self.explorer.file_tree
            } else {
                &self.explorer.search_results
            };

            visible_entries
                .iter()
                .enumerate()
                .filter(|_| show_root_contents)
                .map(|(index, entry)| self.explorer_file_entry(index, entry, cx))
                .collect::<Vec<_>>()
        };

        let d = div()
            .id("editor-media-panel")
            .w_full()
            .h_full()
            .bg(rgb(PANEL))
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .bg(rgb(PANEL))
                    .child(self.explorer.filter.clone().into_any_element())
                    .child(
                        div()
                            .id("editor-media-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.explorer.scroll)
                            .flex()
                            .flex_col()
                            .py_2()
                            .child(root_row)
                            .when(show_root_contents && entries.is_empty(), |this| {
                                this.child(div().p_4().text_sm().text_color(rgb(MUTED)).child(
                                    if filter.is_empty() {
                                        "This project folder is empty.".to_string()
                                    } else if self.explorer.search_pending {
                                        "Searching project…".to_string()
                                    } else {
                                        format!("No files match “{filter_query}”.")
                                    },
                                ))
                            })
                            .children(entries),
                    ),
            )
            .into_any_element();
        return d;
    }
}
