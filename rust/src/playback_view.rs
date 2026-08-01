use gpui::{
    AnyElement, Context, CursorStyle, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    Window, div, prelude::*, px, relative, rgb,
};
use std::time::Duration;

pub(crate) const CONTROL_HEIGHT: f32 = 116.0;
pub(crate) const HORIZONTAL_PADDING: f32 = 22.0;
const VOLUME_TRACK_HEIGHT: f32 = 144.0;
const VOLUME_TRACK_BOTTOM_OFFSET: f32 = 102.0;

const SURFACE_HOVER: u32 = 0x1b1b1f;
const BORDER: u32 = 0x29292e;
const TEXT: u32 = 0xf0f0f2;
const MUTED: u32 = 0x77777f;
const ACCENT: u32 = 0xf0b75e;

#[derive(Clone, Copy)]
pub(crate) enum DragPhase {
    Start,
    Update,
    End,
}

pub(crate) trait PlaybackViewDelegate: Sized + 'static {
    fn playback_toggle(&mut self, window: &mut Window, cx: &mut Context<Self>);
    fn playback_seek(
        &mut self,
        fraction: f32,
        phase: DragPhase,
        window: &mut Window,
        cx: &mut Context<Self>,
    );
    fn playback_set_volume(
        &mut self,
        volume: f64,
        phase: DragPhase,
        window: &mut Window,
        cx: &mut Context<Self>,
    );
    fn playback_toggle_volume(&mut self, window: &mut Window, cx: &mut Context<Self>);
    fn playback_dismiss_volume(&mut self, window: &mut Window, cx: &mut Context<Self>);

    fn playback_toggle_fullscreen(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.toggle_fullscreen();
        cx.notify();
    }
}

pub(crate) struct PlaybackViewProps {
    pub(crate) origin_x: f32,
    pub(crate) origin_y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) has_media: bool,
    pub(crate) can_play: bool,
    pub(crate) paused: bool,
    pub(crate) scrubbing: bool,
    pub(crate) progress: f32,
    pub(crate) position: Duration,
    pub(crate) duration: Duration,
    pub(crate) volume: f64,
    pub(crate) muted: bool,
    pub(crate) volume_open: bool,
    pub(crate) content: AnyElement,
    pub(crate) extra_control: Option<AnyElement>,
}

pub(crate) fn playback_view<T: PlaybackViewDelegate>(
    props: PlaybackViewProps,
    cx: &mut Context<T>,
) -> AnyElement {
    let surface_height = (props.height - CONTROL_HEIGHT).max(1.0);
    let usable_width = (props.width - HORIZONTAL_PADDING * 2.0).max(1.0);
    let timeline_left = props.origin_x + HORIZONTAL_PADDING;
    let volume_track_bottom = props.origin_y + props.height - VOLUME_TRACK_BOTTOM_OFFSET;
    let volume = props.volume.clamp(0.0, 1.0);
    let displayed_volume = if props.muted { 0.0 } else { volume } as f32;
    let volume_percent = (displayed_volume * 100.0).round() as u32;
    let volume_fill_height = displayed_volume * VOLUME_TRACK_HEIGHT;
    let volume_thumb_bottom = displayed_volume * (VOLUME_TRACK_HEIGHT - 20.0);
    let progress = props.progress.clamp(0.0, 1.0);

    div()
        .id("shared-playback-view")
        .relative()
        .w(px(props.width))
        .h(px(props.height))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(rgb(0x000000))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|owner, _, window, cx| owner.playback_dismiss_volume(window, cx)),
        )
        .on_mouse_move(cx.listener(move |owner, event: &MouseMoveEvent, window, cx| {
            if event.dragging() {
                let fraction = ((f32::from(event.position.x) - timeline_left) / usable_width)
                    .clamp(0.0, 1.0);
                owner.playback_seek(fraction, DragPhase::Update, window, cx);
                let volume = ((volume_track_bottom - f32::from(event.position.y))
                    / VOLUME_TRACK_HEIGHT)
                    .clamp(0.0, 1.0) as f64;
                owner.playback_set_volume(volume, DragPhase::Update, window, cx);
            }
        }))
        .on_mouse_up(
            MouseButton::Left,
            cx.listener(move |owner, event: &MouseUpEvent, window, cx| {
                let fraction = ((f32::from(event.position.x) - timeline_left) / usable_width)
                    .clamp(0.0, 1.0);
                owner.playback_seek(fraction, DragPhase::End, window, cx);
                let volume = ((volume_track_bottom - f32::from(event.position.y))
                    / VOLUME_TRACK_HEIGHT)
                    .clamp(0.0, 1.0) as f64;
                owner.playback_set_volume(volume, DragPhase::End, window, cx);
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            cx.listener(move |owner, event: &MouseUpEvent, window, cx| {
                let fraction = ((f32::from(event.position.x) - timeline_left) / usable_width)
                    .clamp(0.0, 1.0);
                owner.playback_seek(fraction, DragPhase::End, window, cx);
                let volume = ((volume_track_bottom - f32::from(event.position.y))
                    / VOLUME_TRACK_HEIGHT)
                    .clamp(0.0, 1.0) as f64;
                owner.playback_set_volume(volume, DragPhase::End, window, cx);
            }),
        )
        .child(
            div()
                .id("shared-playback-surface")
                .h(px(surface_height))
                .w_full()
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .bg(rgb(0x000000))
                .when(props.can_play, |this| {
                    this.cursor(CursorStyle::PointingHand).on_click(cx.listener(
                        |owner, _, window, cx| owner.playback_toggle(window, cx),
                    ))
                })
                .child(props.content),
        )
        .child(
            div()
                .relative()
                .h(px(CONTROL_HEIGHT))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .justify_center()
                .gap_3()
                .px(px(HORIZONTAL_PADDING))
                .border_t_1()
                .border_b_1()
                .border_color(rgb(0x19191c))
                .bg(rgb(0x0b0b0d))
                .when(props.has_media, |this| {
                    this.child(
                        div()
                            .id("shared-playback-timeline")
                            .relative()
                            .h_4()
                            .flex()
                            .items_center()
                            .cursor(CursorStyle::PointingHand)
                            .child(
                                div()
                                    .w_full()
                                    .h(px(3.0))
                                    .rounded_full()
                                    .bg(rgb(0x4a4a4f))
                                    .child(
                                        div()
                                            .w(relative(progress))
                                            .h_full()
                                            .flex()
                                            .items_center()
                                            .justify_end()
                                            .rounded_full()
                                            .bg(rgb(ACCENT))
                                            .child(
                                                div()
                                                    .size(px(if props.scrubbing { 16.0 } else { 12.0 }))
                                                    .flex_shrink_0()
                                                    .rounded_full()
                                                    .bg(rgb(ACCENT)),
                                            ),
                                    ),
                            )
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |owner, event: &MouseDownEvent, window, cx| {
                                    let fraction = ((f32::from(event.position.x) - timeline_left)
                                        / usable_width)
                                        .clamp(0.0, 1.0);
                                    owner.playback_seek(fraction, DragPhase::Start, window, cx);
                                }),
                            ),
                    )
                })
                .child(
                    div()
                        .h_12()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_3()
                                .child(
                                    div()
                                        .id("shared-play-pause")
                                        .w_9()
                                        .h_9()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .cursor(CursorStyle::PointingHand)
                                        .rounded_full()
                                        .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                                        .text_lg()
                                        .text_color(if props.can_play { rgb(TEXT) } else { rgb(MUTED) })
                                        .child(if props.paused { "▶" } else { "Ⅱ" })
                                        .on_click(cx.listener(|owner, _, window, cx| {
                                            owner.playback_toggle(window, cx)
                                        })),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .font_family("monospace")
                                        .child(format!(
                                            "{} / {}",
                                            format_duration(props.position),
                                            format_duration(props.duration)
                                        )),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .id("shared-volume-control")
                                        .relative()
                                        .w(px(72.0))
                                        .h_12()
                                        .flex_shrink_0()
                                        .when(props.volume_open && props.has_media, |this| {
                                            this.child(
                                                div()
                                                    .absolute()
                                                    .left_0()
                                                    .bottom(px(58.0))
                                                    .w(px(72.0))
                                                    .h(px(232.0))
                                                    .flex()
                                                    .flex_col()
                                                    .items_center()
                                                    .rounded(px(22.0))
                                                    .border_1()
                                                    .border_color(rgb(0x35353b))
                                                    .bg(rgb(0x1a1a1d))
                                                    .shadow_lg()
                                                    .occlude()
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(|_, _, _, cx| {
                                                            cx.stop_propagation()
                                                        }),
                                                    )
                                                    .on_mouse_move(cx.listener(
                                                        move |owner,
                                                              event: &MouseMoveEvent,
                                                              window,
                                                              cx| {
                                                            if event.dragging() {
                                                                let volume = ((volume_track_bottom
                                                                    - f32::from(event.position.y))
                                                                    / VOLUME_TRACK_HEIGHT)
                                                                    .clamp(0.0, 1.0)
                                                                    as f64;
                                                                owner.playback_set_volume(
                                                                    volume,
                                                                    DragPhase::Update,
                                                                    window,
                                                                    cx,
                                                                );
                                                            }
                                                            cx.stop_propagation();
                                                        },
                                                    ))
                                                    .on_mouse_up(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            move |owner,
                                                                  event: &MouseUpEvent,
                                                                  window,
                                                                  cx| {
                                                                let volume = ((volume_track_bottom
                                                                    - f32::from(event.position.y))
                                                                    / VOLUME_TRACK_HEIGHT)
                                                                    .clamp(0.0, 1.0)
                                                                    as f64;
                                                                owner.playback_set_volume(
                                                                    volume,
                                                                    DragPhase::End,
                                                                    window,
                                                                    cx,
                                                                );
                                                                cx.stop_propagation();
                                                            },
                                                        ),
                                                    )
                                                    .child(
                                                        div()
                                                            .absolute()
                                                            .top(px(18.0))
                                                            .font_family("monospace")
                                                            .text_lg()
                                                            .text_color(rgb(MUTED))
                                                            .child(volume_percent.to_string()),
                                                    )
                                                    .child(
                                                        div()
                                                            .id("shared-volume-track")
                                                            .absolute()
                                                            .top(px(64.0))
                                                            .w_6()
                                                            .h(px(VOLUME_TRACK_HEIGHT))
                                                            .flex()
                                                            .justify_center()
                                                            .cursor(CursorStyle::PointingHand)
                                                            .child(
                                                                div()
                                                                    .w(px(5.0))
                                                                    .h_full()
                                                                    .rounded_full()
                                                                    .bg(rgb(0x55555b)),
                                                            )
                                                            .child(
                                                                div()
                                                                    .absolute()
                                                                    .bottom_0()
                                                                    .w(px(5.0))
                                                                    .h(px(volume_fill_height))
                                                                    .rounded_full()
                                                                    .bg(rgb(0xdedee2)),
                                                            )
                                                            .child(
                                                                div()
                                                                    .absolute()
                                                                    .left(px(2.0))
                                                                    .bottom(px(volume_thumb_bottom))
                                                                    .size(px(20.0))
                                                                    .rounded_full()
                                                                    .bg(rgb(0xffffff)),
                                                            )
                                                            .on_mouse_down(
                                                                MouseButton::Left,
                                                                cx.listener(move |owner, event: &MouseDownEvent, window, cx| {
                                                                    let volume = ((volume_track_bottom
                                                                        - f32::from(event.position.y))
                                                                        / VOLUME_TRACK_HEIGHT)
                                                                        .clamp(0.0, 1.0)
                                                                        as f64;
                                                                    owner.playback_set_volume(volume, DragPhase::Start, window, cx);
                                                                }),
                                                            ),
                                                    ),
                                            )
                                        })
                                        .child(
                                            div()
                                                .id("shared-volume-toggle")
                                                .absolute()
                                                .left(px(12.0))
                                                .bottom_0()
                                                .size(px(48.0))
                                                .flex()
                                                .items_center()
                                                .justify_center()
                                                .cursor(CursorStyle::PointingHand)
                                                .rounded_xl()
                                                .border_1()
                                                .border_color(rgb(BORDER))
                                                .bg(rgb(0x1a1a1d))
                                                .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(|_, _, _, cx| {
                                                        cx.stop_propagation()
                                                    }),
                                                )
                                                .child(
                                                    div()
                                                        .h(px(28.0))
                                                        .flex()
                                                        .items_end()
                                                        .gap_1()
                                                        .children(
                                                            [10.0_f32, 18.0, 28.0]
                                                                .into_iter()
                                                                .map(|height| {
                                                                    div()
                                                                        .w(px(5.0))
                                                                        .h(px(height))
                                                                        .rounded_full()
                                                                        .bg(if props.muted || volume <= 0.0 {
                                                                            rgb(MUTED)
                                                                        } else {
                                                                            rgb(TEXT)
                                                                        })
                                                                }),
                                                        ),
                                                )
                                                .on_click(cx.listener(|owner, _, window, cx| {
                                                    owner.playback_toggle_volume(window, cx)
                                                })),
                                        ),
                                )
                                .when_some(props.extra_control, |this, control| this.child(control))
                                .child(
                                    div()
                                        .id("shared-fullscreen")
                                        .cursor(CursorStyle::PointingHand)
                                        .rounded_md()
                                        .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                                        .px_3()
                                        .py_2()
                                        .text_lg()
                                        .child("⛶")
                                        .on_click(cx.listener(|owner, _, window, cx| {
                                            owner.playback_toggle_fullscreen(window, cx)
                                        })),
                                ),
                        ),
                ),
        )
        .into_any_element()
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{hours}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes}:{seconds:02}")
    }
}
