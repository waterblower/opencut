use crate::player::Player;
use gpui::{
    Context, CursorStyle, ObjectFit, Render, Window, div, img, prelude::*, px, relative, rgb,
};
use std::time::Duration;

impl Render for Player {
    fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let width = f32::from(window.viewport_size().width).max(1.0);
        let height = (f32::from(window.viewport_size().height) - 100.0).max(1.0);
        let duration = self.video_backend.metadata.duration.unwrap_or_default();
        let position = self.position;
        let progress = if duration.is_zero() {
            0.0
        } else {
            (position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
        };

        div()
            .id("player")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x080808))
            .text_color(rgb(0xeeeeee))
            .child(
                div()
                    .relative()
                    .w_full()
                    .h(px(height))
                    .overflow_hidden()
                    .when_some(self.displayed.as_ref(), |this, frame| {
                        this.child(
                            img(frame.clone())
                                .object_fit(ObjectFit::Contain)
                                .w(px(width))
                                .h(px(height)),
                        )
                    }),
            )
            .child(
                div()
                    .id("seek")
                    .w_full()
                    .h(px(20.0))
                    .flex_shrink_0()
                    .bg(rgb(0x303030))
                    .cursor(CursorStyle::PointingHand)
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
                            .child("Play"),
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

fn format_time(time: Duration) -> String {
    let seconds = time.as_secs();
    format!("{}:{:02}", seconds / 60, seconds % 60)
}
