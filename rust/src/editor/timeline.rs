use super::*;

const MAX_RULER_TICKS: usize = 240;
const MAX_FRAME_TICKS: usize = 2_000;
const MIN_FRAME_TICK_SPACING: f32 = 4.0;
const TICK_STEPS: [f64; 10] = [1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 300.0, 600.0, 1800.0];

impl Editor {
    pub(super) fn timeline(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let duration = self
            .project
            .seconds(self.project.timeline_duration())
            .max(12.0);
        let timeline_width =
            (duration as f32 * self.pixels_per_second + TIMELINE_PADDING * 2.0).max(900.0);
        let frame_rate = self.project.settings.frame_rate;
        let frames_per_second = frame_rate.frames_per_second();
        let displayed_frames = frame_rate.ceil(duration).frames().max(1);
        let pixels_per_frame = self.pixels_per_second / frames_per_second as f32;
        let frame_step = frame_tick_step(displayed_frames, pixels_per_frame);
        let frame_tick_count = displayed_frames / frame_step;
        let nominal_fps = frames_per_second.round().max(1.0) as i64;
        let frame_ticks = (1..=frame_tick_count).map(|index| {
            let frame = index * frame_step;
            let emphasized = frame % nominal_fps == 0;
            let medium = !emphasized && frame % 5 == 0;
            let height = if emphasized {
                12.0
            } else if medium {
                8.0
            } else {
                5.0
            };
            div()
                .absolute()
                .left(px(TIMELINE_PADDING
                    + frame_rate.seconds(TimelineTime::from_frames(frame)) as f32
                        * self.pixels_per_second))
                .bottom_0()
                .h(px(height))
                .border_l_1()
                .border_color(rgb(if emphasized { 0x5a5a62 } else { 0x3a3a40 }))
        });
        let zoom_step = if self.pixels_per_second >= 120.0 {
            1.0
        } else if self.pixels_per_second >= 60.0 {
            2.0
        } else if self.pixels_per_second >= 36.0 {
            5.0
        } else {
            10.0
        };
        // The ruler is not virtualised, so coarsen the step until a long project stays
        // within a bounded number of labels rather than one per second.
        let tick_step = TICK_STEPS
            .iter()
            .copied()
            .find(|step| *step >= zoom_step && duration / step <= MAX_RULER_TICKS as f64)
            .unwrap_or(duration / MAX_RULER_TICKS as f64);
        let tick_count = (duration / tick_step).ceil() as usize + 1;
        let ruler_ticks = (0..tick_count).map(|index| {
            let time = index as f64 * tick_step;
            div()
                .absolute()
                .left(px(TIMELINE_PADDING + time as f32 * self.pixels_per_second))
                .top_0()
                .h_full()
                .border_l_1()
                .border_color(rgb(0x333338))
                .pl_1()
                .font_family("monospace")
                .text_xs()
                .text_color(rgb(MUTED))
                .child(format_time_precise(time))
        });
        let marker_elements = self
            .project
            .markers
            .iter()
            .enumerate()
            .map(|(index, marker)| {
                let marker_time = marker.time;
                div()
                    .id(("timeline-marker", index))
                    .absolute()
                    .left(px(TIMELINE_PADDING
                        + self.project.seconds(marker.time) as f32
                            * self.pixels_per_second
                        - 4.0))
                    .top_0()
                    .size_2()
                    .bg(rgb(ACCENT))
                    .cursor(CursorStyle::PointingHand)
                    .on_click(cx.listener(move |editor, _, _, cx| {
                        editor.load_timeline_position(marker_time, false);
                        cx.notify();
                    }))
            })
            .collect::<Vec<_>>();

        let track_headers = self
            .project
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| self.track_header(index, track, cx))
            .collect::<Vec<_>>();
        let track_rows = self
            .project
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| self.track_row(index, track, timeline_width, cx))
            .collect::<Vec<_>>();
        let playhead_left =
            TIMELINE_PADDING + self.project.seconds(self.playhead) as f32 * self.pixels_per_second;

        div()
            .id("editor-timeline")
            .h(px(TIMELINE_HEIGHT))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(rgb(BORDER))
            .bg(rgb(0x0a0a0c))
            .on_mouse_move(cx.listener(Self::update_trim))
            .on_mouse_move(cx.listener(Self::update_clip_move))
            .on_mouse_move(cx.listener(Self::update_playhead_scrub))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_trim))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_clip_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_playhead_scrub))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_trim))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_clip_move))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_playhead_scrub))
            .child(self.timeline_toolbar(frames_per_second, cx))
            .child(
                div()
                    .id("timeline-tracks-vertical-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.timeline_vertical_scroll)
                    .child(
                        div()
                            .h(px(
                                RULER_HEIGHT + self.project.tracks.len() as f32 * TRACK_HEIGHT,
                            ))
                            .w_full()
                            .flex()
                            .child(
                                div()
                                    .w(px(TRACK_HEADER_WIDTH))
                                    .h_full()
                                    .flex_shrink_0()
                                    .flex()
                                    .flex_col()
                                    .border_r_1()
                                    .border_color(rgb(BORDER))
                                    .child(
                                        div()
                                            .h(px(RULER_HEIGHT))
                                            .flex_shrink_0()
                                            .border_b_1()
                                            .border_color(rgb(BORDER)),
                                    )
                                    .children(track_headers),
                            )
                            .child(
                                div()
                                    .id("editor-timeline-scroll")
                                    .min_w_0()
                                    .flex_1()
                                    .h_full()
                                    .overflow_x_scroll()
                                    .track_scroll(&self.timeline_scroll)
                                    .on_scroll_wheel(
                                        cx.listener(Self::log_timeline_trackpad_scroll),
                                    )
                                    .child(
                                        div()
                                            .relative()
                                            .w(px(timeline_width))
                                            .min_h_full()
                                            .child(
                                                div()
                                                    .id("timeline-seek-ruler")
                                                    .relative()
                                                    .w_full()
                                                    .h(px(RULER_HEIGHT))
                                                    .border_b_1()
                                                    .border_color(rgb(BORDER))
                                                    .cursor(CursorStyle::PointingHand)
                                                    .children(frame_ticks)
                                                    .children(ruler_ticks)
                                                    .children(marker_elements)
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            |editor,
                                                             event: &MouseDownEvent,
                                                             _,
                                                             cx| {
                                                                editor.begin_playhead_scrub(event);
                                                                cx.notify();
                                                            },
                                                        ),
                                                    ),
                                            )
                                            .children(track_rows)
                                            .child(
                                                div()
                                                    .absolute()
                                                    .top_0()
                                                    .bottom_0()
                                                    .left(px(playhead_left))
                                                    .w(px(2.0))
                                                    .bg(rgb(ACCENT))
                                                    .cursor(CursorStyle::ResizeLeftRight)
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            |editor,
                                                             event: &MouseDownEvent,
                                                             _,
                                                             cx| {
                                                                editor.begin_playhead_scrub(event);
                                                                cx.stop_propagation();
                                                                cx.notify();
                                                            },
                                                        ),
                                                    )
                                                    .child(
                                                        div()
                                                            .absolute()
                                                            .top_0()
                                                            .left(px(-4.0))
                                                            .size_2()
                                                            .bg(rgb(ACCENT)),
                                                    ),
                                            ),
                                    )
                            ),
                    ),
            )
            .into_any_element()
    }

    fn timeline_toolbar(&self, frames_per_second: f64, cx: &mut Context<Self>) -> gpui::AnyElement {
        div()
            .id("timeline-toolbar")
            .h(px(TIMELINE_HEADER_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        timeline_icon_button("timeline-play", if self.playing { "Ⅱ" } else { "▶" })
                            .on_click(cx.listener(|editor, _, _, cx| {
                                editor.toggle_playback();
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .w(px(108.0))
                            .font_family("monospace")
                            .text_sm()
                            .child(format!(
                                "{} / {}",
                                format_time(self.project.seconds(self.playhead)),
                                format_time(self.project.seconds(self.project.timeline_duration()))
                            )),
                    )
                    .child(
                        timeline_icon_button("add-video-track", "+V").on_click(cx.listener(
                            |editor, _, _, cx| {
                                editor.add_track(TrackKind::Video);
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        timeline_icon_button("add-audio-track", "+A").on_click(cx.listener(
                            |editor, _, _, cx| {
                                editor.add_track(TrackKind::Audio);
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        timeline_icon_button("add-marker", "◆").on_click(cx.listener(
                            |editor, _, _, cx| {
                                editor.add_marker();
                                cx.notify();
                            },
                        )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(timeline_icon_button("zoom-out", "−").on_click(cx.listener(
                        |editor, _, _, cx| {
                            editor.zoom(0.8);
                            cx.notify();
                        },
                    )))
                    .child(
                        div()
                            .w(px(58.0))
                            .text_center()
                            .font_family("monospace")
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(format!("{:.0}px/s", self.pixels_per_second)),
                    )
                    .child(
                        div()
                            .w(px(66.0))
                            .text_center()
                            .font_family("monospace")
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(format!("{frames_per_second:.2} fps")),
                    )
                    .child(timeline_icon_button("zoom-in", "+").on_click(cx.listener(
                        |editor, _, _, cx| {
                            editor.zoom(1.25);
                            cx.notify();
                        },
                    ))),
            )
            .into_any_element()
    }
}

fn frame_tick_step(total_frames: i64, pixels_per_frame: f32) -> i64 {
    let spacing_step = (MIN_FRAME_TICK_SPACING / pixels_per_frame.max(f32::EPSILON))
        .ceil()
        .max(1.0) as i64;
    let count_step = ((total_frames.max(1) as f64 / MAX_FRAME_TICKS as f64).ceil() as i64).max(1);
    spacing_step.max(count_step)
}

fn timeline_icon_button(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h_7()
        .min_w(px(28.0))
        .px_2()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .bg(rgb(SURFACE))
        .cursor(CursorStyle::PointingHand)
        .text_xs()
        .hover(|style| style.bg(rgb(SURFACE_HOVER)))
        .child(label)
}

fn format_time_precise(seconds: f64) -> String {
    let minutes = (seconds / 60.0).floor() as u64;
    let seconds = seconds % 60.0;
    format!("{minutes:02}:{seconds:04.1}")
}
