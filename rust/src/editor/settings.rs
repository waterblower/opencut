use super::*;

impl Editor {
    pub(super) fn settings_modal(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let selected = self.project.settings.frame_rate;
        let options = FRAME_RATE_PRESETS
            .into_iter()
            .enumerate()
            .map(|(index, (frame_rate, label))| {
                let active = frame_rate == selected;
                div()
                    .id(("timeline-frame-rate", index))
                    .h(px(44.0))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(if active { ACCENT } else { BORDER }))
                    .bg(rgb(if active { 0x2a241b } else { SURFACE }))
                    .cursor(CursorStyle::PointingHand)
                    .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                    .child(label)
                    .child(div().size_2().rounded_full().bg(rgb(if active {
                        ACCENT
                    } else {
                        0x45454d
                    })))
                    .on_click(cx.listener(move |editor, _, _, cx| {
                        editor.set_timeline_frame_rate(frame_rate);
                        cx.notify();
                    }))
            })
            .collect::<Vec<_>>();

        div()
            .id("project-settings-overlay")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .occlude()
            .bg(gpui::rgba(0x000000b3))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _, _, cx| {
                    editor.settings_open = false;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("project-settings-modal")
                    .w(px(460.0))
                    .flex()
                    .flex_col()
                    .rounded_xl()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(PANEL))
                    .shadow_lg()
                    .occlude()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|_, _, _, cx| cx.stop_propagation()),
                    )
                    .child(
                        div()
                            .h(px(58.0))
                            .px_5()
                            .flex()
                            .items_center()
                            .justify_between()
                            .border_b_1()
                            .border_color(rgb(BORDER))
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child("Project Settings"),
                            )
                            .child(
                                div()
                                    .id("close-project-settings")
                                    .size_8()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .cursor(CursorStyle::PointingHand)
                                    .text_color(rgb(MUTED))
                                    .hover(|style| {
                                        style.bg(rgb(SURFACE_HOVER)).text_color(rgb(TEXT))
                                    })
                                    .child("×")
                                    .on_click(cx.listener(|editor, _, _, cx| {
                                        editor.settings_open = false;
                                        cx.notify();
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .p_5()
                            .flex()
                            .flex_col()
                            .gap_4()
                            .child(
                                div()
                                    .text_xs()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(rgb(MUTED))
                                    .child("TIMELINE FRAME RATE"),
                            )
                            .child(div().grid().grid_cols(2).gap_2().children(options))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MUTED))
                                    .child(
                                        "Existing edit points keep their elapsed time and snap to the nearest frame in the new rate.",
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn set_timeline_frame_rate(&mut self, frame_rate: FrameRate) {
        self.settings_open = false;
        let previous = self.project.settings.frame_rate;
        if previous == frame_rate {
            return;
        }

        if let Some(video) = &self.preview.video {
            video.set_paused(true);
        }
        self.checkpoint();
        let playhead = previous.rescale_nearest(self.preview.playhead, frame_rate);
        self.project.set_frame_rate(frame_rate);
        self.preview.playhead =
            playhead.clamp(TimelineTime::ZERO, self.project.timeline_duration());
        self.preview.video = None;
        self.preview.timeline_needs_rebuild = true;
        self.preview.playing = false;
        self.preview.timeline_clock = None;
        self.save_project();
        if !self.project.clips.is_empty() {
            self.load_timeline_position(self.preview.playhead, false);
        }
        self.status = Some(format!(
            "Timeline frame rate changed to {}.",
            frame_rate.label()
        ));
    }
}
