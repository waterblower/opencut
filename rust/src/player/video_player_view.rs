use crate::video_player::{DisplayedFrame, PlaybackState, TogglePlayback, VideoPlayer};
#[cfg(target_os = "macos")]
use gpui::surface;
use gpui::{
    ClickEvent, Context, CursorStyle, ObjectFit, Render, Window, div, img, prelude::*, px,
    relative, rgb,
};
use std::time::Duration;

impl Render for VideoPlayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = f32::from(window.viewport_size().width).max(1.0);
        let height = (f32::from(window.viewport_size().height) - 100.0).max(1.0);
        let duration = self.duration();
        let position = match (&self.playback_state, &self.displayed) {
            (PlaybackState::Ended, _) => duration,
            (_, Some((_, pts, _))) => (*pts).min(duration), // 播放中和暂停时，UI 进度使用当前展示帧的 PTS。
            (_, None) => Duration::ZERO,
        };
        let playback_label = match self.playback_state {
            PlaybackState::Playing => "Pause",
            PlaybackState::Paused => "Play",
            PlaybackState::Ended => "Ended",
        };
        let progress = if duration.is_zero() {
            0.0
        } else {
            (position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
        };

        div()
            .id("player")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(|player, _: &TogglePlayback, _, cx| {
                player.toggle_playback(cx);
            }))
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
                    .when_some(self.displayed.as_ref(), |this, (frame, _, _)| {
                        let content = match frame {
                            #[cfg(target_os = "macos")]
                            DisplayedFrame::Surface(buffer) => surface(buffer.clone())
                                .object_fit(ObjectFit::Contain)
                                .w(px(width))
                                .h(px(height))
                                .into_any_element(),
                            DisplayedFrame::Image(image) => img(image.clone())
                                .object_fit(ObjectFit::Contain)
                                .w(px(width))
                                .h(px(height))
                                .into_any_element(),
                        };
                        this.child(content)
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
                    .on_click(cx.listener(move |player, event: &ClickEvent, window, cx| {
                        let fraction = f32::from(event.position().x) / width;
                        if let Err(error) = player.seek(fraction, window, cx) {
                            eprintln!("Player seek failed: {error:?}");
                            std::process::exit(1);
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

fn format_time(time: Duration) -> String {
    let seconds = time.as_secs();
    format!("{}:{:02}", seconds / 60, seconds % 60)
}
