use crate::seek_bar::{progress, seek_bar, seek_position};
use crate::timeline_player::TimelinePlayer;
use crate::{Seeker, format_time};
use engine::timeline_backend::TimelineBackend;
use gpui::{
    AnyElement, AvailableSpace, Bounds, ClickEvent, Context, CursorStyle, Window, canvas, div,
    prelude::*, px, rgb,
};
use std::{cell::Cell, rc::Rc};

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

        div()
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
                seek_bar("seek", progress(position, duration), seek_bounds.clone()).on_click(
                    cx.listener(move |player, event: &ClickEvent, _, cx| {
                        let target = seek_position(event.position().x, seek_bounds.get(), duration);
                        match player.seek(target) {
                            Ok(()) => cx.notify(),
                            Err(error) => player.fail(error, cx),
                        }
                    }),
                ),
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
                            .child(self.title.clone()),
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
