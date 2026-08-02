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
        let move_previews = self
            .clip_move_drag
            .as_ref()
            .filter(|drag| drag.changed)
            .map(|drag| {
                drag.placements
                    .iter()
                    .filter(|placement| placement.track_id == track.id)
                    .map(|placement| {
                        self.timeline_clip_move_preview(placement, drag.invalid_reason)
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

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
            .cursor(match self.active_timeline_tool {
                TimelineTool::Blade => CursorStyle::Crosshair,
                TimelineTool::Selection | TimelineTool::Trim => CursorStyle::Arrow,
            })
            .children(clips)
            .children(move_previews)
            .into_any_element()
    }

    fn timeline_clip(
        &self,
        track: &TimelineTrack,
        clip: &TimelineClip,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        match track.kind {
            TrackKind::Video => self.video_clip(clip, cx),
            TrackKind::Audio => self.audio_clip(clip, cx),
        }
    }

    fn video_clip(&self, clip: &TimelineClip, cx: &mut Context<Self>) -> gpui::AnyElement {
        let asset = clip.asset_id.and_then(|id| self.project.asset(id));
        let name = asset
            .map(|asset| asset.name.clone())
            .unwrap_or_else(|| "Missing media".to_string());
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
        let detail = if asset.is_some_and(|asset| asset.has_audio) {
            "Audio".to_string()
        } else {
            format!("{}s", self.project.seconds(clip.duration()).round())
        };
        let content = div()
            .absolute()
            .inset_0()
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
                this.child(timeline_clip_waveform(path))
            })
            .child(timeline_clip_label(name, detail));

        self.timeline_clip_frame(clip, CLIP_BLUE, content.into_any_element(), cx)
    }

    fn audio_clip(&self, clip: &TimelineClip, cx: &mut Context<Self>) -> gpui::AnyElement {
        let asset = clip.asset_id.and_then(|id| self.project.asset(id));
        let name = asset
            .map(|asset| asset.name.clone())
            .unwrap_or_else(|| "Missing media".to_string());
        let cached = asset.is_some_and(|asset| self.media_cache_ready.contains(&asset.id));
        let waveform = asset.and_then(|asset| {
            (cached && asset.has_audio)
                .then(|| media_cache::waveform_path(&self.project_root, asset))
        });
        let detail = if asset.is_some_and(|asset| asset.has_audio) {
            "Audio".to_string()
        } else {
            format!("{}s", self.project.seconds(clip.duration()).round())
        };
        let content = div()
            .absolute()
            .inset_0()
            .when_some(waveform, |this, path| {
                this.child(timeline_clip_waveform(path))
            })
            .child(timeline_clip_label(name, detail));

        self.timeline_clip_frame(clip, 0x24656b, content.into_any_element(), cx)
    }

    fn timeline_clip_frame(
        &self,
        clip: &TimelineClip,
        color: u32,
        content: gpui::AnyElement,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let clip_id = clip.id;
        let selected = self.selected_clip_ids.contains(&clip_id);
        let moving = self.clip_move_drag.as_ref().is_some_and(|drag| {
            drag.changed && drag.items.iter().any(|item| item.clip_id == clip_id)
        });
        let left = TIMELINE_PADDING
            + self.project.seconds(clip.timeline_start) as f32 * self.pixels_per_second;
        let width =
            (self.project.seconds(clip.duration()) as f32 * self.pixels_per_second).max(4.0);

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
            .opacity(if moving { 0.3 } else { 1.0 })
            .cursor(match self.active_timeline_tool {
                TimelineTool::Selection => CursorStyle::PointingHand,
                TimelineTool::Blade => CursorStyle::Crosshair,
                TimelineTool::Trim => CursorStyle::Arrow,
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                    editor.begin_clip_interaction(clip_id, event, cx);
                    cx.notify();
                }),
            )
            .child(content)
            .when(
                selected
                    && self.selected_clip_ids.len() == 1
                    && !moving
                    && self.active_timeline_tool == TimelineTool::Trim,
                |this| {
                    this.child(trim_handle(("left-trim", clip_id), true).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            editor.begin_trim(clip_id, TrimEdge::Left, event.position.x.into());
                            cx.notify();
                        }),
                    ))
                    .child(
                        trim_handle(("right-trim", clip_id), false).on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                editor.begin_trim(
                                    clip_id,
                                    TrimEdge::Right,
                                    event.position.x.into(),
                                );
                                cx.notify();
                            }),
                        ),
                    )
                },
            )
            .into_any_element()
    }

    fn timeline_clip_move_preview(
        &self,
        placement: &ClipPlacement,
        invalid_reason: Option<&'static str>,
    ) -> gpui::AnyElement {
        let name = self
            .project
            .clip(placement.clip_id)
            .and_then(|clip| clip.asset_id)
            .and_then(|asset_id| self.project.asset(asset_id))
            .map(|asset| asset.name.clone())
            .unwrap_or_else(|| "Missing media".to_string());
        let left = TIMELINE_PADDING
            + self.project.seconds(placement.start) as f32 * self.pixels_per_second;
        let width =
            (self.project.seconds(placement.duration) as f32 * self.pixels_per_second).max(4.0);
        let valid = invalid_reason.is_none();
        let feedback_color = if valid { ACCENT } else { ERROR };

        div()
            .id(("timeline-clip-move-preview", placement.clip_id))
            .absolute()
            .left(px(left))
            .top(px(5.0))
            .w(px(width))
            .h(px(TRACK_HEIGHT - 10.0))
            .overflow_hidden()
            .rounded_md()
            .border_2()
            .border_color(rgb(feedback_color))
            .bg(gpui::rgba(if valid { 0xf0b75e38 } else { 0xff8b8b38 }))
            .cursor(if valid {
                CursorStyle::ClosedHand
            } else {
                CursorStyle::OperationNotAllowed
            })
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .p_2()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .text_color(rgb(feedback_color))
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
                            .text_ellipsis()
                            .child(invalid_reason.unwrap_or("Move")),
                    ),
            )
            .into_any_element()
    }
}

fn timeline_clip_waveform(path: PathBuf) -> gpui::Div {
    div()
        .absolute()
        .left_0()
        .right_0()
        .bottom_0()
        .h(px(24.0))
        .opacity(0.82)
        .child(img(path).size_full().object_fit(ObjectFit::Fill))
}

fn timeline_clip_label(name: String, detail: String) -> gpui::Div {
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
                .child(detail),
        )
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
        .cursor(if left {
            CursorStyle::ResizeLeft
        } else {
            CursorStyle::ResizeRight
        })
        .occlude()
        .hover(|style| style.opacity(1.0))
}
