use crate::timeline_player::TimelinePlayer;
use crate::{Seeker, format_time};
use engine::timeline_backend::TimelineBackend;
use gpui::{
    AnyElement, AvailableSpace, Bounds, ClickEvent, Context, CursorStyle, Window, canvas, div,
    prelude::*, px, relative, rgb,
};
use std::{cell::Cell, rc::Rc, sync::Arc, time::Duration};

/// Standalone view: picture, seek bar, and transport controls.
impl Render for TimelinePlayer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let seek_bounds = Rc::new(Cell::new(Bounds::default()));
        let duration = self.backend.duration();
        let ended = self.backend.is_ended();
        let position = if ended {
            duration
        } else {
            self.backend.clock_position() // 帧起点会把进度条拉到点击位置左侧，最多一帧。
        };
        let playback_label = match (ended, self.backend.is_playing()) {
            (true, _) => "Ended",
            (_, true) => "Pause",
            (_, false) => "Play",
        };
        let progress = if duration.is_zero() {
            0.0
        } else {
            (position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
        };

        div()
            .on_children_prepainted({
                let seek_bounds = seek_bounds.clone();
                move |bounds, _, _| {
                    seek_bounds.set(bounds[1]); // 子元素依次为画面、进度条、控制栏；使用进度条的窗口坐标。
                }
            })
            .id("timeline-player")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x080808))
            .text_color(rgb(0xeeeeee))
            .child(
                div()
                    .w_full()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(timeline_backend_picture(&self.backend)),
            )
            .child(
                div()
                    .id("seek")
                    .w_full()
                    .h(px(20.0))
                    .flex_shrink_0()
                    .bg(rgb(0x303030))
                    .cursor(CursorStyle::PointingHand)
                    .on_click(cx.listener({
                        let seek_bounds = seek_bounds.clone();
                        move |player, event: &ClickEvent, _, cx| {
                            let bounds = seek_bounds.get();
                            let width = f32::from(bounds.size.width).max(1.0);
                            let fraction = f32::from(event.position().x - bounds.left()) / width;
                            let position = player
                                .backend
                                .duration()
                                .mul_f64(f64::from(fraction.clamp(0.0, 1.0)));
                            match player.seek(position) {
                                Ok(()) => cx.notify(),
                                Err(error) => player.fail(error, cx),
                            }
                        }
                    }))
                    .child(div().h_full().w(relative(progress)).bg(rgb(0xdba34b))),
            )
            .child(
                div()
                    .h(px(80.0))
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .id("play-pause")
                            .cursor(CursorStyle::PointingHand)
                            .p_2()
                            .on_click(cx.listener(|player, _, _, cx| {
                                if let Err(error) = player.toggle_playback(cx) {
                                    player.fail(error, cx);
                                }
                            }))
                            .child(playback_label),
                    )
                    .child(format!(
                        "{} / {}",
                        format_time(position),
                        format_time(duration)
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .when_some(self.error.clone(), |this, error| {
                                this.text_color(rgb(0xff6b6b)).child(error)
                            })
                            .when(self.error.is_none(), |this| this.child(self.title.clone())),
                    ),
            )
    }
}

/// The composited picture alone, filling its parent.
pub fn timeline_backend_picture(backend: &TimelineBackend) -> AnyElement {
    let frame = backend.preview_frame();
    canvas(
        move |bounds, window, cx| {
            let mut element =
                frame.render_frame(bounds.size.width.into(), bounds.size.height.into());
            element.prepaint_as_root(
                bounds.origin,
                bounds.size.map(AvailableSpace::Definite),
                window,
                cx,
            );
            element
        },
        |_, mut element, window, cx| element.paint(window, cx),
    )
    .size_full()
    .into_any_element()
}
