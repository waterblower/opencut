use super::*;

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let preview_width =
            (f32::from(viewport.width) - MEDIA_PANEL_WIDTH - INSPECTOR_WIDTH).max(320.0);
        let preview_height =
            (f32::from(viewport.height) - TOPBAR_HEIGHT - TIMELINE_HEIGHT).max(240.0);
        let preview = if let Some(video_handle) = &self.video {
            video(video_handle.clone())
                .id("editor-preview-video")
                .size(px(preview_width), px(preview_height))
                .buffer_capacity(3)
                .into_any_element()
        } else {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(MUTED))
                .child("Import a video to begin")
                .into_any_element()
        };

        div()
            .id("editor-root")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::action_toggle_playback))
            .on_action(cx.listener(Self::action_delete_selected))
            .on_action(cx.listener(Self::action_split_clip))
            .on_action(cx.listener(Self::action_undo))
            .on_action(cx.listener(Self::action_redo))
            .size_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgb(BACKGROUND))
            .text_color(rgb(TEXT))
            .child(self.topbar(cx))
            .child(
                div()
                    .id("editor-media-scroll")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.media_panel(cx))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .h_full()
                            .flex()
                            .flex_col()
                            .child(
                                div()
                                    .id("editor-preview")
                                    .min_h_0()
                                    .flex_1()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .overflow_hidden()
                                    .bg(rgb(0x000000))
                                    .cursor(CursorStyle::PointingHand)
                                    .on_click(cx.listener(|editor, _, _, cx| {
                                        editor.toggle_playback();
                                        cx.notify();
                                    }))
                                    .child(preview),
                            )
                            .child(self.timeline(cx)),
                    )
                    .child(self.inspector(cx)),
            )
    }
}

impl Editor {
    fn topbar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let undo_enabled = !self.undo_stack.is_empty();
        let redo_enabled = !self.redo_stack.is_empty();
        let export_enabled = !self.project.timeline.is_empty() && !self.exporting;
        let has_error = self.error.is_some();
        let message = self
            .error
            .as_deref()
            .or(self.status.as_deref())
            .unwrap_or("Ready")
            .to_string();

        div()
            .id("editor-topbar")
            .h(px(TOPBAR_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px_5()
            .border_b_1()
            .border_color(rgb(BORDER))
            .bg(rgb(PANEL))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("OpenCut"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(rgb(MUTED))
                            .child("EDITOR · AUTOSAVED"),
                    ),
            )
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .px_5()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .text_sm()
                            .text_ellipsis()
                            .text_color(if has_error { rgb(ERROR) } else { rgb(MUTED) })
                            .child(message),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(toolbar_button("Undo", undo_enabled).on_click(cx.listener(
                        |editor, _, _, cx| {
                            editor.undo();
                            cx.notify();
                        },
                    )))
                    .child(toolbar_button("Redo", redo_enabled).on_click(cx.listener(
                        |editor, _, _, cx| {
                            editor.redo();
                            cx.notify();
                        },
                    )))
                    .child(toolbar_button("Import", true).on_click(cx.listener(
                        |editor, _, _, cx| {
                            editor.import_media(cx);
                        },
                    )))
                    .child(
                        toolbar_button(
                            if self.exporting {
                                "Exporting…"
                            } else {
                                "Export MP4"
                            },
                            export_enabled,
                        )
                        .bg(rgb(ACCENT))
                        .text_color(rgb(0x17120a))
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.export(cx);
                        })),
                    ),
            )
            .into_any_element()
    }

    fn media_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let assets = self
            .project
            .assets
            .iter()
            .enumerate()
            .map(|(index, asset)| {
                let id = asset.id;
                let selected = self.selected_asset_id == Some(id);
                div()
                    .id(("editor-asset", index))
                    .h(px(78.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_3()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(if selected { ACCENT } else { BORDER }))
                    .bg(rgb(if selected { 0x1b1916 } else { SURFACE }))
                    .px_3()
                    .cursor(CursorStyle::PointingHand)
                    .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                    .on_click(cx.listener(move |editor, _, _, cx| {
                        editor.select_asset(id);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w(px(56.0))
                            .h(px(42.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .bg(rgb(0x09090a))
                            .text_color(rgb(0x5d5d66))
                            .child("▶"),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(div().text_sm().text_ellipsis().child(asset.name.clone()))
                            .child(
                                div()
                                    .text_xs()
                                    .font_family("monospace")
                                    .text_color(rgb(MUTED))
                                    .child(format!(
                                        "{}×{} · {}",
                                        asset.width,
                                        asset.height,
                                        format_time(asset.duration)
                                    )),
                            ),
                    )
                    .child(
                        div()
                            .id(("add-asset", index))
                            .size_8()
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_md()
                            .bg(rgb(0x25252a))
                            .hover(|style| style.bg(rgb(ACCENT)).text_color(rgb(0x17120a)))
                            .child("+")
                            .on_click(cx.listener(move |editor, _, _, cx| {
                                editor.selected_asset_id = Some(id);
                                editor.add_selected_asset();
                                cx.notify();
                            })),
                    )
            })
            .collect::<Vec<_>>();

        div()
            .id("editor-media-panel")
            .w(px(MEDIA_PANEL_WIDTH))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(rgb(BORDER))
            .bg(rgb(PANEL))
            .child(
                div()
                    .h(px(52.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(MUTED))
                            .child("MEDIA"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(rgb(MUTED))
                            .child(self.project.assets.len().to_string()),
                    ),
            )
            .child(
                div()
                    .id("editor-media-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .when(assets.is_empty(), |this| {
                        this.child(
                            div()
                                .p_3()
                                .text_sm()
                                .text_color(rgb(MUTED))
                                .child("Imported videos appear here."),
                        )
                    })
                    .children(assets),
            )
            .into_any_element()
    }

    fn inspector(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let selected = self.selected_clip_id.and_then(|id| {
            let index = self.project.clip_index(id)?;
            let clip = &self.project.timeline[index];
            let asset = self.project.asset(clip.asset_id)?;
            Some((index, clip, asset))
        });

        div()
            .id("editor-inspector")
            .w(px(INSPECTOR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(rgb(BORDER))
            .bg(rgb(PANEL))
            .child(
                div()
                    .h(px(52.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px_4()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Inspector"),
            )
            .child(
                div()
                    .id("editor-inspector-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .when_some(selected, |this, (index, clip, asset)| {
                        this.flex()
                            .flex_col()
                            .gap_4()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(asset.name.clone()),
                            )
                            .child(inspector_value(
                                "Timeline start",
                                format_time(self.project.timeline_start(index)),
                            ))
                            .child(inspector_value("Source in", format_time(clip.source_in)))
                            .child(inspector_value("Source out", format_time(clip.source_out)))
                            .child(inspector_value(
                                "Clip duration",
                                format_time(clip.duration()),
                            ))
                            .child(inspector_value(
                                "Source",
                                format!(
                                    "{} · {}×{} · {:.2} fps",
                                    asset.codec, asset.width, asset.height, asset.framerate
                                ),
                            ))
                            .child(
                                div()
                                    .mt_2()
                                    .flex()
                                    .gap_2()
                                    .child(panel_button("Move left").on_click(cx.listener(
                                        |editor, _, _, cx| {
                                            editor.move_selected(-1);
                                            cx.notify();
                                        },
                                    )))
                                    .child(panel_button("Move right").on_click(cx.listener(
                                        |editor, _, _, cx| {
                                            editor.move_selected(1);
                                            cx.notify();
                                        },
                                    ))),
                            )
                            .child(panel_button("Split at playhead").on_click(cx.listener(
                                |editor, _, _, cx| {
                                    editor.split_selected();
                                    cx.notify();
                                },
                            )))
                            .child(panel_button("Delete clip").text_color(rgb(ERROR)).on_click(
                                cx.listener(|editor, _, _, cx| {
                                    editor.delete_selected();
                                    cx.notify();
                                }),
                            ))
                    })
                    .when(selected.is_none(), |this| {
                        this.text_sm()
                            .text_color(rgb(MUTED))
                            .child("Select a timeline clip to inspect it.")
                    }),
            )
            .into_any_element()
    }

    fn timeline(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let timeline_width = (self.project.timeline_duration() as f32 * self.pixels_per_second
            + TIMELINE_PADDING * 2.0)
            .max(640.0);
        let clip_elements = self
            .project
            .timeline
            .iter()
            .enumerate()
            .map(|(index, clip)| {
                let clip_id = clip.id;
                let selected = self.selected_clip_id == Some(clip_id);
                let asset_name = self
                    .project
                    .asset(clip.asset_id)
                    .map(|asset| asset.name.clone())
                    .unwrap_or_else(|| "Missing media".to_string());
                let width = (clip.duration() as f32 * self.pixels_per_second).max(2.0);
                div()
                    .id(("timeline-clip", index))
                    .relative()
                    .w(px(width))
                    .h(px(86.0))
                    .flex_shrink_0()
                    .overflow_hidden()
                    .rounded_lg()
                    .border_2()
                    .border_color(rgb(if selected { ACCENT } else { 0x3a6695 }))
                    .bg(rgb(CLIP_BLUE))
                    .cursor(CursorStyle::PointingHand)
                    .on_click(cx.listener(move |editor, _, _, cx| {
                        editor.select_clip(clip_id);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .flex()
                            .flex_col()
                            .justify_between()
                            .p_3()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_ellipsis()
                                    .child(asset_name),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .font_family("monospace")
                                    .text_color(rgb(0xb9cee5))
                                    .child(format!(
                                        "{} – {}",
                                        format_time(clip.source_in),
                                        format_time(clip.source_out)
                                    )),
                            ),
                    )
                    .child(trim_handle(("left-trim", index), true).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                            editor.begin_trim(clip_id, TrimEdge::Left, event.position.x.into());
                            cx.notify();
                        }),
                    ))
                    .child(trim_handle(("right-trim", index), false).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                            editor.begin_trim(clip_id, TrimEdge::Right, event.position.x.into());
                            cx.notify();
                        }),
                    ))
            })
            .collect::<Vec<_>>();
        let playhead_left = TIMELINE_PADDING + self.playhead as f32 * self.pixels_per_second;

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
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_trim))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_trim))
            .child(
                div()
                    .h(px(TIMELINE_HEADER_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(
                                div()
                                    .id("timeline-play")
                                    .size_8()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_md()
                                    .bg(rgb(SURFACE))
                                    .cursor(CursorStyle::PointingHand)
                                    .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                                    .child(if self.playing { "Ⅱ" } else { "▶" })
                                    .on_click(cx.listener(|editor, _, _, cx| {
                                        editor.toggle_playback();
                                        cx.notify();
                                    })),
                            )
                            .child(div().font_family("monospace").text_sm().child(format!(
                                "{} / {}",
                                format_time(self.playhead),
                                format_time(self.project.timeline_duration())
                            )))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MUTED))
                                    .child("Attached video + audio"),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(panel_button("−").on_click(cx.listener(|editor, _, _, cx| {
                                editor.zoom(0.8);
                                cx.notify();
                            })))
                            .child(
                                div()
                                    .w(px(58.0))
                                    .text_center()
                                    .font_family("monospace")
                                    .text_xs()
                                    .text_color(rgb(MUTED))
                                    .child(format!("{:.0}px/s", self.pixels_per_second)),
                            )
                            .child(panel_button("+").on_click(cx.listener(|editor, _, _, cx| {
                                editor.zoom(1.25);
                                cx.notify();
                            }))),
                    ),
            )
            .child(
                div()
                    .id("editor-timeline-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_x_scroll()
                    .track_scroll(&self.timeline_scroll)
                    .child(
                        div()
                            .relative()
                            .w(px(timeline_width))
                            .h_full()
                            .pt_8()
                            .px(px(TIMELINE_PADDING))
                            .child(
                                div()
                                    .id("timeline-seek-ruler")
                                    .absolute()
                                    .top_0()
                                    .left_0()
                                    .w_full()
                                    .h(px(24.0))
                                    .cursor(CursorStyle::PointingHand)
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(|editor, event: &MouseDownEvent, _, cx| {
                                            editor.seek_from_timeline_x(event.position.x.into());
                                            cx.notify();
                                        }),
                                    ),
                            )
                            .child(div().h(px(86.0)).flex().gap_0().children(clip_elements))
                            .child(
                                div()
                                    .absolute()
                                    .top_3()
                                    .bottom_3()
                                    .left(px(playhead_left))
                                    .w(px(2.0))
                                    .bg(rgb(ACCENT))
                                    .child(
                                        div()
                                            .absolute()
                                            .top_0()
                                            .left(px(-5.0))
                                            .w(px(12.0))
                                            .h(px(8.0))
                                            .bg(rgb(ACCENT)),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }
}

fn toolbar_button(label: &'static str, enabled: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(label)
        .h_9()
        .px_3()
        .flex()
        .items_center()
        .justify_center()
        .rounded_lg()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(SURFACE))
        .cursor(if enabled {
            CursorStyle::PointingHand
        } else {
            CursorStyle::Arrow
        })
        .text_sm()
        .text_color(rgb(if enabled { TEXT } else { 0x55555d }))
        .when(enabled, |this| {
            this.hover(|style| style.bg(rgb(SURFACE_HOVER)))
        })
        .child(label.to_string())
}

fn panel_button(label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(label)
        .h_9()
        .px_3()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .rounded_lg()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(SURFACE))
        .cursor(CursorStyle::PointingHand)
        .text_sm()
        .hover(|style| style.bg(rgb(SURFACE_HOVER)))
        .child(label.to_string())
}

fn inspector_value(label: &str, value: String) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(rgb(MUTED))
                .child(label.to_string()),
        )
        .child(div().text_sm().child(value))
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
