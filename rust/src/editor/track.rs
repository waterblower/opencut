use super::*;

impl Editor {
    pub(super) fn track_header(
        &self,
        index: usize,
        track: &TimelineTrack,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let track_id = track.id;
        div()
            .id(("track-header", index))
            .h(px(TRACK_HEIGHT))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .justify_center()
            .gap_2()
            .px_3()
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(track.name.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(track_kind_label(track.kind)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        track_button(("track-lock", index), if track.locked { "🔒" } else { "♢" })
                            .on_click(cx.listener(move |editor, _, _, cx| {
                                editor.toggle_track_lock(track_id);
                                cx.notify();
                            })),
                    )
                    .child(
                        track_button(
                            ("track-visible", index),
                            if track.visible { "◉" } else { "○" },
                        )
                        .on_click(cx.listener(move |editor, _, _, cx| {
                            editor.toggle_track_visibility(track_id);
                            cx.notify();
                        })),
                    )
                    .child(
                        track_button(("track-mute", index), if track.muted { "M×" } else { "M" })
                            .on_click(cx.listener(move |editor, _, _, cx| {
                                editor.toggle_track_mute(track_id);
                                cx.notify();
                            })),
                    )
                    .child(track_button(("track-up", index), "↑").on_click(cx.listener(
                        move |editor, _, _, cx| {
                            editor.move_track(track_id, -1);
                            cx.notify();
                        },
                    )))
                    .child(
                        track_button(("track-down", index), "↓").on_click(cx.listener(
                            move |editor, _, _, cx| {
                                editor.move_track(track_id, 1);
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        track_button(("track-delete", index), "×").on_click(cx.listener(
                            move |editor, _, _, cx| {
                                editor.delete_track(track_id);
                                cx.notify();
                            },
                        )),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn track_row(
        &self,
        index: usize,
        track: &TimelineTrack,
        timeline_width: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let clips = self
            .project
            .clips_on_track(track.id)
            .map(|clip| self.timeline_clip(track, clip, cx))
            .collect::<Vec<_>>();

        div()
            .id(("track-row", index))
            .relative()
            .w(px(timeline_width))
            .h(px(TRACK_HEIGHT))
            .flex_shrink_0()
            .border_b_1()
            .border_color(rgb(BORDER))
            .bg(rgb(if index.is_multiple_of(2) {
                0x101012
            } else {
                0x0d0d0f
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, event: &MouseDownEvent, window, cx| {
                    editor.begin_marquee_selection(event, window, cx);
                }),
            )
            .children(clips)
            .into_any_element()
    }

    fn timeline_clip(
        &self,
        track: &TimelineTrack,
        clip: &TimelineClip,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let clip_id = clip.id;
        let selected = self.selected_clip_ids.contains(&clip_id);
        let asset = clip.asset_id.and_then(|id| self.project.asset(id));
        let name = asset
            .map(|asset| asset.name.clone())
            .unwrap_or_else(|| "Missing media".to_string());
        let left = TIMELINE_PADDING
            + self.project.seconds(clip.timeline_start) as f32 * self.pixels_per_second;
        let width =
            (self.project.seconds(clip.duration()) as f32 * self.pixels_per_second).max(4.0);
        let color = match track.kind {
            TrackKind::Video => CLIP_BLUE,
            TrackKind::Audio => 0x24656b,
        };
        let cached = asset.is_some_and(|asset| self.media_cache_ready.contains(&asset.id));
        let thumbnail = asset.and_then(|asset| match asset.kind {
            MediaKind::Image => Some(self.project_root.join(&asset.path)),
            MediaKind::Video => {
                cached.then(|| media_cache::thumbnail_path(&self.project_root, asset))
            }
            MediaKind::Audio => None,
        });
        let waveform = asset.and_then(|asset| {
            (cached && asset.has_audio)
                .then(|| media_cache::waveform_path(&self.project_root, asset))
        });
        let has_audio = asset.is_some_and(|asset| asset.has_audio);

        div()
            .id(("timeline-clip", clip_id))
            .absolute()
            .left(px(left))
            .top(px(5.0))
            .w(px(width))
            .h(px(TRACK_HEIGHT - 10.0))
            .overflow_hidden()
            .rounded_md()
            .border_2()
            .border_color(rgb(if selected { ACCENT } else { color + 0x101010 }))
            .bg(rgb(color))
            .cursor(CursorStyle::PointingHand)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                    editor.begin_clip_move(clip_id, event, cx);
                    cx.notify();
                }),
            )
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .when_some(thumbnail, |this, path| {
                        this.child(
                            img(path)
                                .size_full()
                                .object_fit(ObjectFit::Cover)
                                .opacity(0.45),
                        )
                    }),
            )
            .when_some(waveform, |this, path| {
                this.child(
                    div()
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h(px(24.0))
                        .opacity(0.82)
                        .child(img(path).size_full().object_fit(ObjectFit::Fill)),
                )
            })
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .p_2()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_ellipsis()
                            .child(name),
                    )
                    .child(
                        div()
                            .font_family("monospace")
                            .text_xs()
                            .text_color(rgb(0xc8d8e8))
                            .child(if has_audio {
                                "Audio".to_string()
                            } else {
                                format!("{}s", self.project.seconds(clip.duration()).round())
                            }),
                    ),
            )
            .when(selected && self.selected_clip_ids.len() == 1, |this| {
                this.child(trim_handle(("left-trim", clip_id), true).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        editor.begin_trim(clip_id, TrimEdge::Left, event.position.x.into());
                        cx.notify();
                    }),
                ))
                .child(trim_handle(("right-trim", clip_id), false).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                        cx.stop_propagation();
                        editor.begin_trim(clip_id, TrimEdge::Right, event.position.x.into());
                        cx.notify();
                    }),
                ))
            })
            .into_any_element()
    }
}

fn track_button(id: impl Into<gpui::ElementId>, label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h_5()
        .min_w(px(24.0))
        .px_1()
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .bg(rgb(SURFACE))
        .cursor(CursorStyle::PointingHand)
        .text_xs()
        .hover(|style| style.bg(rgb(SURFACE_HOVER)))
        .child(label)
}

fn track_kind_label(kind: TrackKind) -> &'static str {
    match kind {
        TrackKind::Video => "V",
        TrackKind::Audio => "A",
    }
}

fn trim_handle(id: impl Into<gpui::ElementId>, left: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .absolute()
        .top_0()
        .bottom_0()
        .when(left, |this| this.left_0())
        .when(!left, |this| this.right_0())
        .w(px(10.0))
        .bg(rgb(ACCENT))
        .opacity(0.72)
        .cursor(CursorStyle::ResizeLeftRight)
        .occlude()
        .hover(|style| style.opacity(1.0))
}
