//! The progress bar shared by the player views. Views attach their own event handlers.

use gpui::{
    Bounds, CursorStyle, Div, ElementId, Empty, Pixels, Stateful, canvas, div, prelude::*, px,
    relative, rgb,
};
use std::{cell::Cell, rc::Rc, time::Duration};

pub(crate) const PROGRESS_BAR_HEIGHT: f32 = 20.0;

/// Drag payload of a progress bar;
/// views seek on `on_drag_move::<ProgressBarDrag>`.
pub(crate) struct ProgressBarDrag;

/// A thin track filled to `progress`, inside a taller hit area. Records the track's window
/// bounds into `bounds` while prepainting, and starts a [`ProgressBarDrag`] when the pointer
/// is dragged from it. On hover the track thickens and shows a knob at `progress`.
pub(crate) fn progress_bar(
    id: impl Into<ElementId>,
    progress: f32,
    bounds: Rc<Cell<Bounds<Pixels>>>,
) -> Stateful<Div> {
    div()
        .id(id)
        .group("progress-bar")
        .w_full()
        .h(px(PROGRESS_BAR_HEIGHT))
        .flex_shrink_0()
        .px_4()
        .flex()
        .items_center()
        .cursor(CursorStyle::PointingHand)
        .on_drag(ProgressBarDrag, |_, _, _, cx| cx.new(|_| Empty)) // 拖动时不显示跟随鼠标的预览。
        .child(
            div()
                .relative()
                .w_full()
                .h(px(4.0))
                .rounded_full()
                .bg(rgb(0x2b2b31))
                .group_hover("progress-bar", |style| style.h(px(6.0)))
                .child(
                    div()
                        .h_full()
                        .w(relative(progress))
                        .rounded_full()
                        .bg(rgb(0xf0b75e)),
                )
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .bottom_0()
                        .left(relative(progress))
                        .w_0()
                        .flex()
                        .items_center()
                        .justify_center() // 零宽锚点在进度处；圆点在其上居中，不依赖轨道高度。
                        .child(
                            div()
                                .flex_shrink_0()
                                .size(px(12.0))
                                .rounded_full()
                                .border_2()
                                .border_color(rgb(0x0d0d0f))
                                .bg(rgb(0xf2f2f4))
                                .invisible()
                                .group_hover("progress-bar", |style| style.visible()),
                        ),
                )
                .child(
                    canvas(
                        move |seek_bounds, _, _| bounds.set(seek_bounds), // 只记录细轨道的范围，两端留白点击时夹到首尾。
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .top_0()
                    .left_0()
                    .size_full(),
                ),
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
