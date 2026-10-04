//! The progress bar shared by the player views. Views attach their own event handlers.

use gpui::{
    Bounds, CursorStyle, Div, ElementId, Pixels, Stateful, canvas, div, prelude::*, px, relative,
    rgb,
};
use std::{cell::Cell, rc::Rc, time::Duration};

/// A track filled to `progress`. Records its own window bounds into `bounds` while prepainting.
pub(crate) fn progress_bar(
    id: impl Into<ElementId>,
    progress: f32,
    bounds: Rc<Cell<Bounds<Pixels>>>,
) -> Stateful<Div> {
    div()
        .id(id)
        .relative()
        .w_full()
        .h(px(20.0))
        .flex_shrink_0()
        .bg(rgb(0x303030))
        .cursor(CursorStyle::PointingHand)
        .child(div().h_full().w(relative(progress)).bg(rgb(0xdba34b)))
        .child(
            canvas(
                move |seek_bounds, _, _| bounds.set(seek_bounds),
                |_, _, _, _| {},
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full(),
        )
}

/// Played fraction of `duration`; zero for empty media.
pub(crate) fn progress(position: Duration, duration: Duration) -> f32 {
    if duration.is_zero() {
        return 0.0;
    }
    (position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
}

/// The media position under `pointer_x`, clamped to the ends of the bar.
pub(crate) fn seek_position(
    pointer_x: Pixels,
    bounds: Bounds<Pixels>,
    duration: Duration,
) -> Duration {
    let width = f32::from(bounds.size.width).max(1.0);
    let fraction = (f32::from(pointer_x - bounds.left()) / width).clamp(0.0, 1.0);
    duration.mul_f64(f64::from(fraction))
}
