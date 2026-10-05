use crate::audio_player::{AudioPlayer, PlaybackState};
use crate::progress_bar::{ProgressBarDrag, progress, progress_bar, seek_position};
use crate::{Seeker, format_time};
use gpui::{Bounds, ClickEvent, Context, DragMoveEvent, Render, Window, div, prelude::*, rgb};
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
        let detail = format!("{} / {}", format_time(position), format_time(duration));
        div()
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
                progress_bar(
                    "progress_bar",
                    progress(position, duration),
                    seek_bounds.clone(),
                )
                .on_click(cx.listener(move |player, event: &ClickEvent, _, cx| {
                    let target = seek_position(event.position().x, seek_bounds.get(), duration);
                    match player.seek(target) {
                        Ok(()) => cx.notify(),
                        Err(error) => eprintln!("Seeking audio failed: {error:?}"),
                    }
                }))
                .on_drag_move(cx.listener(
                    move |player, event: &DragMoveEvent<ProgressBarDrag>, _, cx| {
                        let target = seek_position(event.event.position.x, event.bounds, duration);
                        match player.seek(target) {
                            Ok(()) => cx.notify(),
                            Err(error) => eprintln!("Seeking audio failed: {error:?}"),
                        }
                    },
                )),
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
