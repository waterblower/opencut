use super::*;

const MAX_RULER_TICKS: usize = 240;
const MIN_RULER_LABEL_SPACING: f32 = 72.0;
const MIN_FRAME_TICK_SPACING: f32 = 4.0;
const FRAME_TICK_OVERSCAN: f32 = 120.0;
const TICK_STEPS: [f64; 12] = [
    1.0, 2.0, 5.0, 10.0, 15.0, 20.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0,
];

impl Editor {
    pub(super) fn timeline(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let frames_per_second = self.project.settings.frame_rate.frames_per_second();

        div()
            .id("editor-timeline")
            .relative()
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
            .on_mouse_move(cx.listener(Self::update_marquee_selection))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_trim))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_clip_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_playhead_scrub))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(Self::finish_marquee_selection),
            )
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_trim))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_clip_move))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_playhead_scrub))
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(Self::finish_marquee_selection),
            )
            .child(self.timeline_toolbar(frames_per_second, cx))
            .child(self.timeline_tracks_container(cx))
            .when_some(self.timeline_marquee(), |this, marquee| this.child(marquee))
            .into_any_element()
    }

    fn timeline_marquee(&self) -> Option<gpui::AnyElement> {
        let selection = self.marquee_selection.as_ref()?;
        let left = selection.start_x.min(selection.current_x);
        let top = selection.start_y.min(selection.current_y);
        let width = (selection.start_x - selection.current_x).abs();
        let height = (selection.start_y - selection.current_y).abs();

        Some(
            div()
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(width))
                .h(px(height))
                .border_1()
                .border_color(rgb(ACCENT))
                .bg(gpui::rgba(0xf0b75e24))
                .into_any_element(),
        )
    }

    fn timeline_tracks_container(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let duration = self
            .project
            .seconds(self.project.timeline_duration())
            .max(12.0);
        let timeline_width =
            (duration as f32 * self.pixels_per_second + TIMELINE_PADDING * 2.0).max(900.0);
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
            .id("timeline-tracks-vertical-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&self.timeline_vertical_scroll)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, event: &MouseDownEvent, window, cx| {
                    editor.begin_marquee_selection(event, window, cx);
                }),
            )
            .child(
                div()
                    .h(px(
                        RULER_HEIGHT + self.project.tracks.len() as f32 * TRACK_HEIGHT
                    ))
                    .min_h_full()
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
                            .cursor(match self.active_timeline_tool {
                                TimelineTool::Blade => CursorStyle::Crosshair,
                                TimelineTool::Selection | TimelineTool::Trim => CursorStyle::Arrow,
                            })
                            .on_scroll_wheel(cx.listener(Self::log_timeline_trackpad_scroll))
                            .on_mouse_move(cx.listener(Self::update_blade_guide))
                            .on_hover(cx.listener(Self::update_blade_guide_hover))
                            .child(
                                div()
                                    .relative()
                                    .w(px(timeline_width))
                                    .min_h_full()
                                    .on_drag_move::<ExplorerMediaDrag>(
                                        cx.listener(Self::update_explorer_media_drag),
                                    )
                                    .on_drop(cx.listener(
                                        |editor, drag: &ExplorerMediaDrag, _, cx| {
                                            editor.drop_explorer_media(drag, cx);
                                        },
                                    ))
                                    .child(self.timeline_ruler(duration, cx))
                                    .children(track_rows)
                                    .child(
                                        div()
                                            .absolute()
                                            .top_0()
                                            .bottom_0()
                                            .left(px(playhead_left))
                                            .w(px(2.0))
                                            .bg(rgb(ACCENT))
                                            .cursor(
                                                if self.active_timeline_tool == TimelineTool::Blade
                                                {
                                                    CursorStyle::Crosshair
                                                } else {
                                                    CursorStyle::ResizeLeftRight
                                                },
                                            )
                                            .when(
                                                self.active_timeline_tool != TimelineTool::Blade,
                                                |this| {
                                                    this.on_mouse_down(
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
                                                },
                                            )
                                            .child(
                                                div()
                                                    .absolute()
                                                    .top_0()
                                                    .left(px(-4.0))
                                                    .size_2()
                                                    .bg(rgb(ACCENT)),
                                            ),
                                    )
                                    .when_some(self.snap_guide, |this, guide| {
                                        let guide_left = TIMELINE_PADDING
                                            + self.project.seconds(guide) as f32
                                                * self.pixels_per_second;
                                        this.child(
                                            div()
                                                .absolute()
                                                .top_0()
                                                .bottom_0()
                                                .left(px(guide_left))
                                                .w(px(2.0))
                                                .bg(rgb(0x63c8ff))
                                                .child(
                                                    div()
                                                        .absolute()
                                                        .top_0()
                                                        .left(px(-3.0))
                                                        .size_2()
                                                        .rounded_full()
                                                        .bg(rgb(0x63c8ff)),
                                                ),
                                        )
                                    })
                                    .when_some(self.blade_guide_position, |this, position| {
                                        let guide_left = TIMELINE_PADDING
                                            + self.project.seconds(position) as f32
                                                * self.pixels_per_second;
                                        this.child(
                                            div()
                                                .absolute()
                                                .top_0()
                                                .bottom_0()
                                                .left(px(guide_left))
                                                .w(px(2.0))
                                                .bg(rgb(ERROR))
                                                .cursor(CursorStyle::Crosshair)
                                                .child(
                                                    div()
                                                        .absolute()
                                                        .top_0()
                                                        .left(px(-4.0))
                                                        .size_2()
                                                        .bg(rgb(ERROR)),
                                                ),
                                        )
                                    }),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn timeline_ruler(&self, duration: f64, cx: &mut Context<Self>) -> gpui::AnyElement {
        let frame_rate = self.project.settings.frame_rate;
        let frames_per_second = frame_rate.frames_per_second();
        let displayed_frames = frame_rate.ceil(duration).frames().max(1);
        let pixels_per_frame = self.pixels_per_second / frames_per_second as f32;
        let frame_step = frame_tick_step(pixels_per_frame);
        let scroll_left = (-f32::from(self.timeline_scroll.offset().x)).max(0.0);
        let viewport_width = {
            let width = f32::from(self.timeline_scroll.bounds().size.width);
            if width > 0.0 { width } else { 1_200.0 }
        };
        let visible_start = ((scroll_left - FRAME_TICK_OVERSCAN - TIMELINE_PADDING).max(0.0)
            / pixels_per_frame.max(f32::EPSILON))
        .floor() as i64;
        let visible_end = ((scroll_left + viewport_width + FRAME_TICK_OVERSCAN - TIMELINE_PADDING)
            .max(0.0)
            / pixels_per_frame.max(f32::EPSILON))
        .ceil() as i64;
        let first_frame = visible_start
            .div_euclid(frame_step)
            .saturating_mul(frame_step)
            .max(frame_step);
        let last_frame = visible_end.min(displayed_frames);
        let nominal_fps = frames_per_second.round().max(1.0) as i64;
        let frame_ticks = (first_frame..=last_frame)
            .step_by(frame_step as usize)
            .map(|frame| {
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
        let tick_step = ruler_tick_step(duration, self.pixels_per_second);
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
        div()
            .id("timeline-ruler")
            .relative()
            .w_full()
            .h(px(RULER_HEIGHT))
            .border_b_1()
            .border_color(rgb(BORDER))
            .cursor(CursorStyle::PointingHand)
            .children(frame_ticks)
            .children(ruler_ticks)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, event: &MouseDownEvent, _, cx| {
                    editor.begin_playhead_scrub(event);
                    cx.stop_propagation();
                    cx.notify();
                }),
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
                        timeline_tool_button(
                            "timeline-selection-tool",
                            "V Select",
                            self.active_timeline_tool == TimelineTool::Selection,
                        )
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.activate_timeline_tool(TimelineTool::Selection);
                            cx.notify();
                        })),
                    )
                    .child(
                        timeline_tool_button(
                            "timeline-blade-tool",
                            "B Blade",
                            self.active_timeline_tool == TimelineTool::Blade,
                        )
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.activate_timeline_tool(TimelineTool::Blade);
                            cx.notify();
                        })),
                    )
                    .child(
                        timeline_tool_button(
                            "timeline-trim-tool",
                            "T Trim",
                            self.active_timeline_tool == TimelineTool::Trim,
                        )
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.activate_timeline_tool(TimelineTool::Trim);
                            cx.notify();
                        })),
                    )
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
                                format_time(self.project.seconds(self.playhead), false),
                                format_time(self.project.seconds(self.project.timeline_duration()), false)
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
                        timeline_icon_button(
                            "toggle-timeline-snapping",
                            if self.snapping_enabled {
                                "Snap on"
                            } else {
                                "Snap off"
                            },
                        )
                        .border_1()
                        .border_color(rgb(if self.snapping_enabled {
                            ACCENT
                        } else {
                            BORDER
                        }))
                        .text_color(rgb(if self.snapping_enabled { ACCENT } else { MUTED }))
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.toggle_snapping();
                            cx.notify();
                        })),
                    )
                    .child(
                        timeline_icon_button(
                            "toggle-track-magnet",
                            if self.track_magnet_enabled {
                                "Magnet on"
                            } else {
                                "Magnet off"
                            },
                        )
                        .border_1()
                        .border_color(rgb(if self.track_magnet_enabled {
                            ACCENT
                        } else {
                            BORDER
                        }))
                        .text_color(rgb(if self.track_magnet_enabled {
                            ACCENT
                        } else {
                            MUTED
                        }))
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.toggle_track_magnet();
                            cx.notify();
                        })),
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

fn frame_tick_step(pixels_per_frame: f32) -> i64 {
    (MIN_FRAME_TICK_SPACING / pixels_per_frame.max(f32::EPSILON))
        .ceil()
        .max(1.0) as i64
}

fn ruler_tick_step(duration: f64, pixels_per_second: f32) -> f64 {
    let spacing_step = MIN_RULER_LABEL_SPACING as f64 / pixels_per_second.max(f32::EPSILON) as f64;
    let count_step = duration.max(0.0) / MAX_RULER_TICKS as f64;
    let minimum_step = spacing_step.max(count_step);

    TICK_STEPS
        .iter()
        .copied()
        .find(|step| *step >= minimum_step)
        .unwrap_or_else(|| {
            let largest_step = *TICK_STEPS
                .last()
                .expect("timeline tick steps are not empty");
            (minimum_step / largest_step).ceil() * largest_step
        })
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

fn timeline_tool_button(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    active: bool,
) -> gpui::Stateful<gpui::Div> {
    timeline_icon_button(id, label)
        .border_1()
        .border_color(rgb(if active { ACCENT } else { BORDER }))
        .text_color(rgb(if active { ACCENT } else { MUTED }))
}

fn format_time_precise(seconds: f64) -> String {
    let minutes = (seconds / 60.0).floor() as u64;
    let seconds = seconds % 60.0;
    format!("{minutes:02}:{seconds:04.1}")
}
