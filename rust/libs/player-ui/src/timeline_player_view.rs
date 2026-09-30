use crate::timeline_player::TimelinePlayer;
use engine::{
    timeline_backend::TimelineBackend,
    timeline_decoder::{PreparedFrame, PreparedLayer},
};
use gpui::{
    AnyElement, AvailableSpace, Bounds, ClickEvent, Context, CursorStyle, TextAlign, Window,
    canvas, div, img, prelude::*, px, relative, rgb, rgba,
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
                            if let Err(error) = player.seek(position, cx) {
                                player.fail(error, cx);
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

fn format_time(time: Duration) -> String {
    let seconds = time.as_secs();
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// The composited picture alone, filling its parent.
pub fn timeline_backend_picture(backend: &TimelineBackend) -> AnyElement {
    let frame = backend.preview_frame();
    canvas(
        move |bounds, window, cx| {
            let mut element =
                render_frame(&frame, bounds.size.width.into(), bounds.size.height.into());
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

/// Same composition as the editor preview: fit the canvas, then apply each layer's transform.
fn render_frame(frame: &PreparedFrame, width: f32, height: f32) -> AnyElement {
    let root = div()
        .w(px(width.max(0.0)))
        .h(px(height.max(0.0)))
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .bg(rgb(0));
    let scale = (width / frame.width as f32).min(height / frame.height as f32);
    if !scale.is_finite() || scale <= 0.0 {
        return root.into_any_element();
    }
    let canvas_width = frame.width as f32 * scale;
    let canvas_height = frame.height as f32 * scale;
    let mut canvas = div()
        .relative()
        .flex_shrink_0()
        .overflow_hidden()
        .w(px(canvas_width))
        .h(px(canvas_height));
    for layer in &frame.layers {
        match layer {
            PreparedLayer::Picture {
                image, properties, ..
            } => {
                if properties.scale <= 0.0 {
                    continue;
                }
                let size = image.size(0);
                let source_width = size.width.0 as f32;
                let source_height = size.height.0 as f32;
                let fit = (canvas_width / source_width).min(canvas_height / source_height);
                let width = source_width * fit * properties.scale as f32;
                let height = source_height * fit * properties.scale as f32;
                let x = (canvas_width - width) / 2.0 + properties.position_x as f32 * scale;
                let y = (canvas_height - height) / 2.0 + properties.position_y as f32 * scale;
                canvas = canvas.child(
                    img(Arc::clone(image))
                        .absolute()
                        .left(px(x))
                        .top(px(y))
                        .w(px(width))
                        .h(px(height)),
                );
            }
            PreparedLayer::Text { properties, .. } => {
                canvas = canvas.child(
                    div()
                        .absolute()
                        .left(px(properties.position_x as f32 * canvas_width))
                        .top(px(properties.position_y as f32 * canvas_height))
                        .w(px(0.0))
                        .h(px(0.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .flex_shrink_0()
                                .whitespace_nowrap()
                                .font_family(properties.font.clone())
                                .text_size(px(properties.font_size as f32 * scale))
                                .line_height(px(properties.font_size as f32 * scale * 1.2))
                                .text_align(TextAlign::Center)
                                .text_color(rgba(properties.color.rotate_left(8)))
                                .child(properties.text.clone()),
                        ),
                );
            }
        }
    }
    root.child(canvas).into_any_element()
}
