use crate::progress_bar::{progress, progress_bar, seek_position};
use crate::video_player::{PlaybackState, VideoPlayer};
use crate::{Seeker, format_time};
#[cfg(target_os = "macos")]
use gpui::surface;
use gpui::{
    Bounds, ClickEvent, Context, CursorStyle, ObjectFit, Render, Window, div, prelude::*, px, rgb,
};
use std::{cell::Cell, rc::Rc, time::Duration};

impl Render for VideoPlayer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let seek_bounds = Rc::new(Cell::new(Bounds::default()));
        let duration = self.duration();
        let ended = match self.is_ended() {
            Ok(ended) => ended,
            Err(error) => {
                eprintln!("Reading player completion failed: {error:?}");
                std::process::exit(1);
            }
        };
        let position = match (ended, &self.displayed) {
            (true, _) => duration,
            (_, Some((_, pts, _))) => (*pts).min(duration), // 播放中和暂停时，UI 进度使用当前展示帧的 PTS。
            (_, None) => Duration::ZERO,
        };
        let playback_label = match (ended, &self.playback_state) {
            (true, _) => "Ended",
            (_, PlaybackState::Playing) => "Pause",
            (_, PlaybackState::Paused) => "Play",
        };

        div()
            .id("player")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x080808))
            .text_color(rgb(0xeeeeee))
            .child(
                div()
                    .relative()
                    .w_full()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .when_some(self.displayed.as_ref(), |this, (frame, _, _)| {
                        #[cfg(target_os = "macos")]
                        let content = surface(frame.clone())
                            .object_fit(ObjectFit::Contain)
                            .size_full()
                            .into_any_element();
                        #[cfg(not(target_os = "macos"))]
                        let content: gpui::AnyElement = match *frame {};
                        this.child(content)
                    }),
            )
            .child(
                progress_bar("progress_bar", progress(position, duration), seek_bounds.clone()).on_click(
                    cx.listener(move |player, event: &ClickEvent, _, cx| {
                        let target = seek_position(event.position().x, seek_bounds.get(), duration);
                        match player.seek(target) {
                            Ok(()) => cx.notify(),
                            Err(error) => {
                                eprintln!("Player seek failed: {error:?}");
                                std::process::exit(1);
                            }
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
                                player.toggle_playback(cx);
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
