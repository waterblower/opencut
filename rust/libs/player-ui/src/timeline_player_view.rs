use crate::Seeker;
use crate::playback_row::{
    PLAYBACK_ROW_HEIGHT, PlaybackIcon, playback_button_view, playback_row_view,
};
use crate::progress_bar::{
    PROGRESS_BAR_HEIGHT, ProgressBarDrag, progress, progress_bar, seek_position,
};
use crate::timeline_player::TimelinePlayer;
use gpui::{
    AnyElement, AvailableSpace, Bounds, ClickEvent, Context, CursorStyle, DragMoveEvent, Entity,
    Window, canvas, div, prelude::*, px, rgb,
};
use std::{cell::Cell, rc::Rc};

pub const TIMELINE_CONTROLS_HEIGHT: f32 = 1.0 + PROGRESS_BAR_HEIGHT + PLAYBACK_ROW_HEIGHT; // 1：控制栏顶部分隔线。

/// Standalone view: picture, progress bar, and playback controls.
impl Render for TimelinePlayer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let picture = timeline_backend_picture(cx.entity());
        self.render_with_picture(picture, cx)
    }
}

impl TimelinePlayer {
    /// Uses the caller's picture while retaining the player's playback controls.
    pub fn render_with_picture(
        &mut self,
        picture: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let seek_bounds = Rc::new(Cell::new(Bounds::default()));
        let duration = self.backend().duration();
        let ended = self.backend().is_ended();
        let position = if ended {
            duration
        } else {
            self.backend().clock_position() // 帧起点会把进度条拉到点击位置左侧，最多一帧。
        };
        let playback_icon = match (ended, self.backend().is_playing()) {
            (true, _) => PlaybackIcon::Replay,
            (_, true) => PlaybackIcon::Pause,
            (_, false) => PlaybackIcon::Play,
        };
        let drag_seek_bounds = seek_bounds.clone();

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
                    .child(picture),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .border_t_1()
                    .border_color(rgb(0x1f1f24))
                    .bg(rgb(0x0d0d0f))
                    .child(
                        progress_bar(
                            "progress_bar",
                            progress(position, duration),
                            seek_bounds.clone(),
                        )
                        .on_click(cx.listener(move |player, event: &ClickEvent, _, cx| {
                            let target =
                                seek_position(event.position().x, seek_bounds.get(), duration);
                            match player.seek(target) {
                                Ok(()) => cx.notify(),
                                Err(error) => player.fail(error, cx),
                            }
                        }))
                        .on_drag_move(cx.listener(
                            move |player, event: &DragMoveEvent<ProgressBarDrag>, window, cx| {
                                let target = seek_position(
                                    event.event.position.x,
                                    drag_seek_bounds.get(),
                                    duration,
                                );
                                player.request_seek(target, window, cx);
                            },
                        )),
                    )
                    .child(playback_row_view(
                        playback_button_view("play-pause", playback_icon).on_click(cx.listener(
                            |player, _, _, cx| {
                                if let Err(error) = player.toggle_playback(cx) {
                                    player.fail(error, cx);
                                }
                            },
                        )),
                        position,
                        duration,
                        self.title.clone(),
                    )),
            )
            .into_any_element()
    }
}

/// The composited picture alone, filling its parent.
fn timeline_backend_picture(player: Entity<TimelinePlayer>) -> AnyElement {
    canvas(
        move |bounds, window, cx| {
            let backend = player.read(cx).backend();
            let frame = backend.preview_frame();
            let mut element = frame.render_frame(
                bounds.size.width.into(),
                bounds.size.height.into(),
                backend.timeline(),
            );
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
