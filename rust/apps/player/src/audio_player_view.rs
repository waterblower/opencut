use crate::audio_player::{AudioPlayer, PlaybackState, ToggleAudio};
use gpui::{ClickEvent, Context, Render, Window, div, prelude::*, px, relative, rgb};

impl Render for AudioPlayer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let width = f32::from(window.viewport_size().width).max(1.0);
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
            .id("audio-player")
            .track_focus(&self.focus)
            .on_action(cx.listener(|player, _: &ToggleAudio, _, cx| {
                if let Err(error) = player.toggle_playback(cx) {
                    eprintln!("Toggling audio playback failed: {error:?}");
                }
            }))
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
                    .on_click(cx.listener(move |player, event: &ClickEvent, window, cx| {
                        let fraction = (f32::from(event.position().x) / width).clamp(0.0, 1.0);
                        if let Err(error) = player.seek(duration.mul_f64(f64::from(fraction)), cx) {
                            eprintln!("Seeking audio failed: {error:?}");
                        }
                        player.focus.focus(window, cx);
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
