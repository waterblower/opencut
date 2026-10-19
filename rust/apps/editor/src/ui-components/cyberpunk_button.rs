//! Style D · Cyberpunk from design/pages/00-design-system.svg.
//! Callers attach `.on_click` only when the button is enabled.
//!
//! gpui boxes only have rounded corners, so the cut-corner outline is painted on a
//! `canvas`. A canvas cannot use hover/active styles, so each button keeps its own
//! pointer state with `window.use_keyed_state`.

use gpui::prelude::*;
use gpui::{
    App, Bounds, Div, ElementId, Entity, FontWeight, MouseButton, PathBuilder, Pixels, Point, Rgba,
    Stateful, Window, canvas, div, fill, point, px, rgb, size,
};

/// Neon yellow with cut top-left and bottom-right corners and hazard slashes.
/// Hover brightens the border and glow; pressing turns the button inside out.
pub(crate) fn button_cyberpunk_primary(
    id: impl Into<ElementId>,
    label: &str,
    enabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let element_id = id.into();
    let pointer = window.use_keyed_state(element_id.clone(), cx, |_, _| PointerState::default());
    let look = current_look(enabled, *pointer.read(cx));
    let text_color = match look {
        Look::Default | Look::Hover => DARK,
        Look::Pressed => YELLOW,
        Look::Disabled => OLIVE_TEXT,
    };
    let button = div()
        .id(element_id)
        .relative()
        .h_10()
        .pl_6()
        .pr(px(PRIMARY_RIGHT_PADDING))
        .flex()
        .items_center()
        .font_family(FONT_FAMILY)
        .font_weight(FontWeight::BOLD)
        .text_size(px(13.0))
        .whitespace_nowrap()
        .text_color(rgb(text_color))
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, (), window, _| paint_primary(bounds, look, window),
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .child(label.to_uppercase())
        .when(look == Look::Pressed, |this| {
            this.top(px(PRESS_OFFSET)).left(px(PRESS_OFFSET))
        });
    track_pointer(button, pointer, enabled)
}

/// Cyan outline with the same cut corners as the primary button.
/// Hover brightens it; pressing fills it solid over a magenta offset.
pub(crate) fn button_cyberpunk_secondary(
    id: impl Into<ElementId>,
    label: &str,
    enabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> Stateful<Div> {
    let element_id = id.into();
    let pointer = window.use_keyed_state(element_id.clone(), cx, |_, _| PointerState::default());
    let look = current_look(enabled, *pointer.read(cx));
    let text_color = match look {
        Look::Default => CYAN,
        Look::Hover => CYAN_HOVER_TEXT,
        Look::Pressed => DARK,
        Look::Disabled => TEAL_TEXT,
    };
    let button = div()
        .id(element_id)
        .relative()
        .h_10()
        .px_8()
        .flex()
        .items_center()
        .justify_center()
        .font_family(FONT_FAMILY)
        .font_weight(FontWeight::BOLD)
        .text_size(px(13.0))
        .whitespace_nowrap()
        .text_color(rgb(text_color))
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, (), window, _| paint_secondary(bounds, look, window),
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
        .child(label.to_uppercase());
    track_pointer(button, pointer, enabled)
}

const FONT_FAMILY: &str = "Menlo";
const CUT: f32 = 10.0; // 切角的边长
const PRIMARY_RIGHT_PADDING: f32 = 44.0; // 给右侧三条斜纹留出的空间
const PRESS_OFFSET: f32 = 2.0; // 按下时整个按钮向右下移动
const MAGENTA_OFFSET: f32 = 3.0; // 次按钮按下时洋红色底影的偏移
const SCANLINE_SPACING: f32 = 3.0;
const GLOW_LAYERS: usize = 6;
const GLOW_STEP: f32 = 1.5; // 每层光晕描边加宽的像素

const YELLOW: u32 = 0xfcee0a;
const CYAN: u32 = 0x00f0ff;
const MAGENTA: u32 = 0xff2a6d;
const DARK: u32 = 0x0a0a0b;
const OLIVE: u32 = 0x3a3926;
const OLIVE_TEXT: u32 = 0x57563f;
const CYAN_HOVER_STROKE: u32 = 0x7ffbff;
const CYAN_HOVER_TEXT: u32 = 0xb5fcff;
const YELLOW_HOVER_BORDER: u32 = 0xfff9b8;
const TEAL: u32 = 0x1f3337;
const TEAL_TEXT: u32 = 0x34494d;
const WHITE: u32 = 0xffffff;

#[derive(Clone, Copy, Default)]
struct PointerState {
    hovered: bool,
    pressed: bool, // 左键在按钮上按下且尚未松开
}

#[derive(Clone, Copy, PartialEq)]
enum Look {
    Default,
    Hover,
    Pressed,
    Disabled,
}

fn current_look(enabled: bool, pointer: PointerState) -> Look {
    if !enabled {
        Look::Disabled
    } else if pointer.pressed {
        Look::Pressed
    } else if pointer.hovered {
        Look::Hover
    } else {
        Look::Default
    }
}

/// Pointer tracking stays attached while disabled so the state is current when re-enabled.
fn track_pointer(
    button: Stateful<Div>,
    pointer: Entity<PointerState>,
    enabled: bool,
) -> Stateful<Div> {
    let hover_pointer = pointer.clone();
    let down_pointer = pointer.clone();
    let up_pointer = pointer.clone();
    button
        .when(enabled, |this| this.cursor_pointer())
        .on_hover(move |hovered, _, cx| {
            hover_pointer.update(cx, |state, cx| {
                state.hovered = *hovered;
                cx.notify();
            })
        })
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            down_pointer.update(cx, |state, cx| {
                state.pressed = true;
                cx.notify();
            })
        })
        .on_mouse_up(MouseButton::Left, move |_, _, cx| {
            up_pointer.update(cx, |state, cx| {
                state.pressed = false;
                cx.notify();
            })
        })
        .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
            pointer.update(cx, |state, cx| {
                state.pressed = false;
                cx.notify();
            })
        })
}

fn paint_primary(bounds: Bounds<Pixels>, look: Look, window: &mut Window) {
    let shape = outline(bounds);
    match look {
        Look::Default => {
            paint_glow(&shape, YELLOW, 0.45, window);
            paint_shape(PathBuilder::fill(), &shape, rgb(YELLOW), window);
            paint_slashes(bounds, rgb(DARK), window);
        }
        Look::Hover => {
            paint_glow(&shape, YELLOW, 0.75, window);
            paint_shape(PathBuilder::fill(), &shape, rgb(YELLOW), window);
            paint_shape(
                PathBuilder::stroke(px(1.5)),
                &shape,
                rgb(YELLOW_HOVER_BORDER),
                window,
            );
            paint_slashes(bounds, rgb(DARK), window);
        }
        Look::Pressed => {
            paint_glow(&shape, YELLOW, 0.75, window);
            paint_shape(PathBuilder::fill(), &shape, rgb(DARK), window);
            paint_scanlines(bounds, color(YELLOW, 0.2), window);
            paint_shape(PathBuilder::stroke(px(1.5)), &shape, rgb(YELLOW), window);
            paint_slashes(bounds, rgb(YELLOW), window);
        }
        Look::Disabled => {
            paint_scanlines(bounds, color(WHITE, 0.05), window);
            paint_shape(PathBuilder::stroke(px(1.0)), &shape, rgb(OLIVE), window);
            paint_slashes(bounds, rgb(OLIVE), window);
        }
    }
}

fn paint_secondary(bounds: Bounds<Pixels>, look: Look, window: &mut Window) {
    let shape = outline(bounds);
    match look {
        Look::Default => {
            paint_glow(&shape, CYAN, 0.35, window);
            paint_shape(PathBuilder::fill(), &shape, color(CYAN, 0.06), window);
            paint_shape(PathBuilder::stroke(px(1.0)), &shape, rgb(CYAN), window);
        }
        Look::Hover => {
            paint_glow(&shape, CYAN, 0.7, window);
            paint_shape(PathBuilder::fill(), &shape, color(CYAN, 0.16), window);
            paint_shape(
                PathBuilder::stroke(px(1.0)),
                &shape,
                rgb(CYAN_HOVER_STROKE),
                window,
            );
            let bar = Bounds::new(
                at(bounds, 0.0, CUT),
                size(px(4.0), bounds.size.height - px(CUT)),
            );
            window.paint_quad(fill(bar, rgb(CYAN)));
        }
        Look::Pressed => {
            let magenta_shape = outline(shifted(bounds, MAGENTA_OFFSET, MAGENTA_OFFSET));
            paint_shape(PathBuilder::fill(), &magenta_shape, rgb(MAGENTA), window);
            paint_glow(&shape, CYAN, 0.7, window);
            paint_shape(PathBuilder::fill(), &shape, rgb(CYAN), window);
        }
        Look::Disabled => {
            paint_scanlines(bounds, color(WHITE, 0.05), window);
            paint_shape(PathBuilder::stroke(px(1.0)), &shape, rgb(TEAL), window);
        }
    }
}

/// Both button kinds cut the top-left and bottom-right corners.
fn outline(bounds: Bounds<Pixels>) -> [Point<Pixels>; 6] {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    [
        at(bounds, CUT, 0.0),
        at(bounds, width, 0.0),
        at(bounds, width, height - CUT),
        at(bounds, width - CUT, height),
        at(bounds, 0.0, height),
        at(bounds, 0.0, CUT),
    ]
}

/// Fills or strokes a closed polygon, depending on `builder`.
fn paint_shape(
    mut builder: PathBuilder,
    points: &[Point<Pixels>],
    color: Rgba,
    window: &mut Window,
) {
    builder.add_polygon(points, true);
    match builder.build() {
        Ok(path) => window.paint_path(path, color),
        Err(error) => log::error!("Could not build button shape: {error:?}"),
    }
}

/// Stacked strokes of growing width approximate a blurred neon glow that follows the cut corners.
fn paint_glow(points: &[Point<Pixels>], hex: u32, strength: f32, window: &mut Window) {
    let layer_color = color(hex, strength / GLOW_LAYERS as f32);
    for layer in (1..=GLOW_LAYERS).rev() {
        let width = px(layer as f32 * GLOW_STEP);
        paint_shape(PathBuilder::stroke(width), points, layer_color, window);
    }
}

/// One-pixel lines every few pixels, clipped to the cut-corner outline.
fn paint_scanlines(bounds: Bounds<Pixels>, color: Rgba, window: &mut Window) {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    let mut y = 0.0;
    while y < height {
        let left_inset = (CUT - y).max(0.0);
        let right_inset = (y + 1.0 - (height - CUT)).max(0.0);
        let line = Bounds::new(
            at(bounds, left_inset, y),
            size(px(width - left_inset - right_inset), px(1.0)),
        );
        window.paint_quad(fill(line, color));
        y += SCANLINE_SPACING;
    }
}

/// Three hazard slashes near the right edge.
fn paint_slashes(bounds: Bounds<Pixels>, color: Rgba, window: &mut Window) {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    for index in 0..3 {
        let left = width - 30.0 + index as f32 * 7.0;
        let slash = [
            at(bounds, left, height - 6.0),
            at(bounds, left + 8.0, 6.0),
            at(bounds, left + 11.0, 6.0),
            at(bounds, left + 3.0, height - 6.0),
        ];
        paint_shape(PathBuilder::fill(), &slash, color, window);
    }
}

fn at(bounds: Bounds<Pixels>, x: f32, y: f32) -> Point<Pixels> {
    point(bounds.origin.x + px(x), bounds.origin.y + px(y))
}

fn shifted(bounds: Bounds<Pixels>, x: f32, y: f32) -> Bounds<Pixels> {
    Bounds::new(at(bounds, x, y), bounds.size)
}

fn color(hex: u32, alpha: f32) -> Rgba {
    Rgba {
        a: alpha,
        ..rgb(hex)
    }
}
