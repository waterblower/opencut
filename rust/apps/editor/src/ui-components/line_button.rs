//! Style C · Line from design/pages/00-design-system.svg.
//! Callers attach `.on_click` only when the button is enabled.

use gpui::prelude::*;
use gpui::{Div, ElementId, FontWeight, Stateful, div, px, rgb, rgba};

/// A frame drawn by corner brackets. Hover opens the brackets; pressing fills the button solid.
pub(crate) fn button_line_primary(
    id: impl Into<ElementId>,
    label: &str,
    enabled: bool,
) -> Stateful<Div> {
    let (text_color, bracket_color) = if enabled {
        (LIME, LIME)
    } else {
        (DISABLED_TEXT, DISABLED_BRACKET)
    };
    let button = div()
        .id(id)
        .group(PRIMARY_GROUP)
        .relative()
        .h_10()
        .px_6()
        .flex()
        .items_center()
        .justify_center()
        .gap_2()
        .font_family(FONT_FAMILY)
        .font_weight(FontWeight::BOLD)
        .text_size(px(13.0))
        .text_color(rgb(text_color))
        .child(label.to_uppercase())
        .child(div().relative().child("→").when(enabled, |arrow| {
            arrow.group_hover(PRIMARY_GROUP, |style| style.left(px(ARROW_SHIFT)))
        }))
        .child(bracket_view(Corner::TopLeft, bracket_color, enabled))
        .child(bracket_view(Corner::TopRight, bracket_color, enabled))
        .child(bracket_view(Corner::BottomLeft, bracket_color, enabled))
        .child(bracket_view(Corner::BottomRight, bracket_color, enabled));
    if enabled {
        button
            .cursor_pointer()
            .bg(rgba(LIME_FILL))
            .hover(|style| style.bg(rgba(LIME_HOVER_FILL)).text_color(rgb(LIME_HOVER)))
            .active(|style| style.bg(rgb(LIME)).text_color(rgb(PRESSED_TEXT)))
    } else {
        button
    }
}

/// Underlined text. Hover widens the underline; pressing tints the background.
pub(crate) fn button_line_secondary(
    id: impl Into<ElementId>,
    label: &str,
    enabled: bool,
) -> Stateful<Div> {
    let button = div()
        .id(id)
        .group(SECONDARY_GROUP)
        .h_10()
        .px_8()
        .flex()
        .items_center()
        .justify_center()
        .font_family(FONT_FAMILY)
        .font_weight(FontWeight::BOLD)
        .text_size(px(13.0));
    if enabled {
        button
            .cursor_pointer()
            .text_color(rgb(SECONDARY_TEXT))
            .hover(|style| style.text_color(rgb(WHITE)))
            .active(|style| style.bg(rgba(WHITE_PRESSED_FILL)))
            .child(
                div().relative().child(label.to_uppercase()).child(
                    div()
                        .id("underline")
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom(px(-5.0))
                        .h(px(1.0))
                        .bg(rgb(UNDERLINE))
                        .group_hover(SECONDARY_GROUP, |style| {
                            style.left(px(-16.0)).right(px(-16.0)).bg(rgb(WHITE))
                        })
                        .group_active(SECONDARY_GROUP, |style| style.h(px(2.0))),
                ),
            )
    } else {
        button
            .text_color(rgb(DISABLED_SECONDARY_TEXT))
            .child(label.to_uppercase())
    }
}

const FONT_FAMILY: &str = "Menlo";
const PRIMARY_GROUP: &str = "line-primary-button";
const SECONDARY_GROUP: &str = "line-secondary-button";
const BRACKET_LENGTH: f32 = 8.0;
const BRACKET_OPEN: f32 = 3.0; // 悬停时括角向外移动的距离
const ARROW_SHIFT: f32 = 4.0; // 悬停和按下时箭头向右移动的距离

const LIME: u32 = 0xc8f25a;
const LIME_HOVER: u32 = 0xd9ff7a;
const LIME_FILL: u32 = 0xc8f25a0d;
const LIME_HOVER_FILL: u32 = 0xc8f25a1f;
const PRESSED_TEXT: u32 = 0x0b0b0d;
const DISABLED_TEXT: u32 = 0x4f4f56;
const DISABLED_BRACKET: u32 = 0x3a3a40;
const SECONDARY_TEXT: u32 = 0xa0a0a8;
const DISABLED_SECONDARY_TEXT: u32 = 0x46464c;
const UNDERLINE: u32 = 0x4a4a52;
const WHITE: u32 = 0xf2f2f4;
const WHITE_PRESSED_FILL: u32 = 0xffffff12;

#[derive(Clone, Copy)]
enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

fn bracket_view(corner: Corner, color: u32, enabled: bool) -> Stateful<Div> {
    let bracket = div()
        .absolute()
        .size(px(BRACKET_LENGTH))
        .border_color(rgb(color));
    let open = px(-BRACKET_OPEN);
    match corner {
        Corner::TopLeft => bracket
            .id("bracket-top-left")
            .top_0()
            .left_0()
            .border_t_2()
            .border_l_2()
            .when(enabled, |this| {
                this.group_hover(PRIMARY_GROUP, |style| style.top(open).left(open))
                    .group_active(PRIMARY_GROUP, |style| style.top_0().left_0())
            }),
        Corner::TopRight => bracket
            .id("bracket-top-right")
            .top_0()
            .right_0()
            .border_t_2()
            .border_r_2()
            .when(enabled, |this| {
                this.group_hover(PRIMARY_GROUP, |style| style.top(open).right(open))
                    .group_active(PRIMARY_GROUP, |style| style.top_0().right_0())
            }),
        Corner::BottomLeft => bracket
            .id("bracket-bottom-left")
            .bottom_0()
            .left_0()
            .border_b_2()
            .border_l_2()
            .when(enabled, |this| {
                this.group_hover(PRIMARY_GROUP, |style| style.bottom(open).left(open))
                    .group_active(PRIMARY_GROUP, |style| style.bottom_0().left_0())
            }),
        Corner::BottomRight => bracket
            .id("bracket-bottom-right")
            .bottom_0()
            .right_0()
            .border_b_2()
            .border_r_2()
            .when(enabled, |this| {
                this.group_hover(PRIMARY_GROUP, |style| style.bottom(open).right(open))
                    .group_active(PRIMARY_GROUP, |style| style.bottom_0().right_0())
            }),
    }
}
