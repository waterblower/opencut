use crate::asset::IconName;
use crate::theme::{BORDER, MUTED, PANEL, TEXT};
use gpui::{FontWeight, IntoElement, ParentElement, Styled, div, px, rgb, svg};

pub(super) fn empty_panel_view() -> gpui::AnyElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .px(px(16.0))
        .bg(rgb(PANEL))
        .text_color(rgb(TEXT))
        .child(
            div()
                .size(px(48.0))
                .rounded(px(12.0))
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(0x131316))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    svg()
                        .path(IconName::Pointer.path())
                        .size(px(24.0))
                        .text_color(rgb(MUTED)),
                ),
        )
        .child(
            div()
                .mt(px(14.0))
                .text_size(px(14.0))
                .line_height(px(20.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child("Nothing selected"),
        )
        .child(
            div()
                .mt(px(5.0))
                .text_center()
                .text_size(px(12.0))
                .line_height(px(17.0))
                .text_color(rgb(MUTED))
                .child("Select a clip on the timeline or a file in the Explorer."),
        )
        .into_any_element()
}
