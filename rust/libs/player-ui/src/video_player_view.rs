use crate::video_player::{DisplayedFrame, PlaybackState, VideoPlayer};
#[cfg(target_os = "macos")]
use gpui::surface;
use gpui::{
    Bounds, ClickEvent, Context, CursorStyle, ObjectFit, Render, Window, div, img, prelude::*, px,
    relative, rgb,
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
        let progress = if duration.is_zero() {
            0.0
        } else {
            (position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
        };

        div()
            .on_children_prepainted({
                let seek_bounds = seek_bounds.clone();
                move |bounds, _, _| {
                    seek_bounds.set(bounds[1]); // 子元素依次为视频、进度条、控制栏；使用进度条的窗口坐标。
                }
            })
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
                        let content = match frame {
                            #[cfg(target_os = "macos")]
                            DisplayedFrame::Surface(buffer) => surface(buffer.clone())
                                .object_fit(ObjectFit::Contain)
                                .size_full()
                                .into_any_element(),
                            DisplayedFrame::GpuiImage(image) => img(image.clone())
                                .object_fit(ObjectFit::Contain)
                                .size_full()
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
                    .on_click(cx.listener({
                        let seek_bounds = seek_bounds.clone();
                        move |player, event: &ClickEvent, _, cx| {
                            let bounds = seek_bounds.get();
                            let width = f32::from(bounds.size.width).max(1.0);
                            let fraction = f32::from(event.position().x - bounds.left()) / width;
                            if let Err(error) = seek(player, fraction, cx) {
                                eprintln!("Player seek failed: {error:?}");
                                std::process::exit(1);
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

pub fn seek(
    player: &mut VideoPlayer,
    fraction: f32,
    cx: &mut Context<VideoPlayer>,
) -> anyhow::Result<()> {
    let duration = player.duration();
    let position = duration.mul_f64(f64::from(fraction.clamp(0.0, 1.0)));
    player.seek(position)?;
    cx.notify();
    Ok(())
}

fn format_time(time: Duration) -> String {
    let seconds = time.as_secs();
    format!("{}:{:02}", seconds / 60, seconds % 60)
}
