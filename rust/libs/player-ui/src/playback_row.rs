//! The playback row shared by the video and timeline players: playback button, time, and title.

use crate::format_time;
use gpui::{
    CursorStyle, Div, ElementId, PathBuilder, Pixels, Point, Stateful, canvas, div, point,
    prelude::*, px, rgb,
};
use std::time::Duration;

pub const PLAYBACK_ROW_HEIGHT: f32 = 44.0;

pub(crate) enum PlaybackIcon {
    Play,
    Pause,
    Replay,
}

/// The round playback button. Views attach their own click handler.
pub(crate) fn playback_button_view(id: impl Into<ElementId>, icon: PlaybackIcon) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(24.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(rgb(0xf2f2f4))
        .cursor(CursorStyle::PointingHand)
        .hover(|style| style.bg(rgb(0xffffff)))
        .child(playback_icon_view(icon))
}

/// Button, `position / duration`, and the title truncated on the right.
pub(crate) fn playback_row_view(
    button: Stateful<Div>,
    position: Duration,
    duration: Duration,
    title: String,
) -> Div {
    div()
        .h(px(PLAYBACK_ROW_HEIGHT))
        .flex_shrink_0()
        .px_4()
        .flex()
        .items_center()
        .gap(px(10.0))
        .child(button)
        .child(
            div()
                .flex()
                .flex_shrink_0()
                .font_family("monospace")
                .text_size(px(13.0))
                .text_color(rgb(0xf2f2f4))
                .child(format_time(position))
                .child(
                    div()
                        .text_color(rgb(0x777780))
                        .child(format!(" / {}", format_time(duration))),
                ),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .ml(px(14.0))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .text_right()
                .text_size(px(12.0))
                .text_color(rgb(0x777780))
                .child(title),
        )
}

fn playback_icon_view(icon: PlaybackIcon) -> gpui::AnyElement {
    let color = rgb(0x0d0d0f);
    match icon {
        PlaybackIcon::Pause => {
            let bar = || div().w(px(2.2)).h(px(8.0)).rounded(px(0.8)).bg(color);
            div()
                .flex()
                .gap(px(1.6))
                .child(bar())
                .child(bar())
                .into_any_element()
        }
        PlaybackIcon::Play => canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let center = bounds.center();
                let mut builder = PathBuilder::fill();
                builder.move_to(center + point(px(-2.5), px(-4.0)));
                builder.line_to(center + point(px(4.0), px(0.0)));
                builder.line_to(center + point(px(-2.5), px(4.0)));
                builder.close();
                match builder.build() {
                    Ok(path) => window.paint_path(path, color),
                    Err(error) => panic!("could not build the play icon: {error:?}"),
                }
            },
        )
        .size(px(10.0))
        .into_any_element(),
        PlaybackIcon::Replay => canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                // lucide rotate-ccw, drawn from its 24-unit grid at 10px.
                let center = bounds.center();
                let scale = 10.0 / 24.0;
                let at = |x: f32, y: f32| -> Point<Pixels> {
                    center + point(px((x - 12.0) * scale), px((y - 12.0) * scale))
                };
                let mut builder = PathBuilder::stroke(px(1.35));
                builder.move_to(at(3.0, 12.0));
                builder.arc_to(
                    point(px(9.0 * scale), px(9.0 * scale)),
                    px(0.0),
                    true,
                    false,
                    at(12.0, 3.0),
                );
                builder.arc_to(
                    point(px(9.75 * scale), px(9.75 * scale)),
                    px(0.0),
                    false,
                    false,
                    at(5.26, 5.74),
                );
                builder.line_to(at(3.0, 8.0));
                builder.move_to(at(3.0, 3.0));
                builder.line_to(at(3.0, 8.0));
                builder.line_to(at(8.0, 8.0));
                match builder.build() {
                    Ok(path) => window.paint_path(path, color),
                    Err(error) => panic!("could not build the replay icon: {error:?}"),
                }
            },
        )
        .size(px(10.0))
        .into_any_element(),
    }
}
