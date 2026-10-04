use crate::audio_player::{AudioPlayer, PlaybackState};
use gpui::{Bounds, ClickEvent, Context, Render, Window, div, prelude::*, px, relative, rgb};
use std::{cell::Cell, rc::Rc};

impl Render for AudioPlayer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let seek_bounds = Rc::new(Cell::new(Bounds::default()));
        let duration = self.audio_backend.metadata.duration;
        let position = self.position;
        let label = match self.playback_state {
            PlaybackState::Playing => "Pause",
            PlaybackState::Paused => "Play",
            PlaybackState::Ended => "Replay",
        };
        let progress = if duration.is_zero() {
            0.0
        } else {
            (position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
        };
        let detail = format!(
            "{}:{:02} / {}:{:02}",
            position.as_secs() / 60,
            position.as_secs() % 60,
            duration.as_secs() / 60,
            duration.as_secs() % 60
        );
        div()
            .on_children_prepainted({
                let seek_bounds = seek_bounds.clone();
                move |bounds, _, _| {
                    seek_bounds.set(bounds[1]); // 子元素依次为音频信息、进度条、播放按钮；使用进度条的窗口坐标。
                }
            })
            .id("audio-player")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x080808))
            .text_color(rgb(0xeeeeee))
            .child(
                div()
                    .flex_1()
                    .p_4()
                    .child(self.title.clone())
                    .child(div().mt_4().child(detail)),
            )
            .child(
                div()
                    .id("audio-seek")
                    .w_full()
                    .h(px(20.0))
                    .bg(rgb(0x303030))
                    .cursor_pointer()
                    .on_click(cx.listener(move |player, event: &ClickEvent, _, cx| {
                        let bounds = seek_bounds.get();
                        let width = f32::from(bounds.size.width).max(1.0);
                        let fraction =
                            (f32::from(event.position().x - bounds.left()) / width).clamp(0.0, 1.0);
                        match player.seek(duration.mul_f64(f64::from(fraction))) {
                            Ok(()) => cx.notify(),
                            Err(error) => eprintln!("Seeking audio failed: {error:?}"),
                        }
                    }))
                    .child(div().h_full().w(relative(progress)).bg(rgb(0xdba34b))),
            )
            .child(
                div()
                    .id("audio-play-pause")
                    .p_4()
                    .cursor_pointer()
                    .child(label)
                    .on_click(cx.listener(|player, _, _, cx| {
                        if let Err(error) = player.toggle_playback(cx) {
                            eprintln!("Toggling audio playback failed: {error:?}");
                        }
                    })),
            )
    }
}
