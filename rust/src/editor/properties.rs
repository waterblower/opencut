use super::*;

impl Editor {
    pub(super) fn properties_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let selected = self.selected_clip_id.and_then(|id| {
            let index = self.project.clip_index(id)?;
            let clip = &self.project.clips[index];
            let asset = clip
                .asset_id
                .and_then(|asset_id| self.project.asset(asset_id));
            let track = self.project.track(clip.track_id)?;
            Some((clip, asset, track))
        });

        div()
            .id("editor-properties-panel")
            .w(px(PROPERTIES_PANEL_WIDTH))
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
                    .child("Properties"),
            )
            .child(
                div()
                    .id("editor-properties-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .when_some(selected, |this, (clip, asset, track)| {
                        let title = asset
                            .map(|asset| asset.name.clone())
                            .unwrap_or_else(|| "Missing media".to_string());
                        this.flex()
                            .flex_col()
                            .gap_4()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(title),
                            )
                            .child(properties_value(
                                "Timeline start",
                                format_time(self.project.seconds(clip.timeline_start)),
                            ))
                            .child(properties_value(
                                "Source in",
                                format_time(self.project.source_start_seconds(clip)),
                            ))
                            .child(properties_value(
                                "Source out",
                                format_time(
                                    self.project
                                        .source_position_at(clip, clip.timeline_end())
                                        .as_secs_f64(),
                                ),
                            ))
                            .child(properties_value(
                                "Clip duration",
                                format_time(self.project.seconds(clip.duration())),
                            ))
                            .child(properties_value("Track", track.name.clone()))
                            .when_some(asset, |this, asset| {
                                this.child(properties_value("Source", asset_description(asset)))
                            })
                            .child(
                                div()
                                    .mt_2()
                                    .flex()
                                    .gap_2()
                                    .child(panel_button("Nudge left").on_click(cx.listener(
                                        |editor, _, _, cx| {
                                            editor.move_selected(-1);
                                            cx.notify();
                                        },
                                    )))
                                    .child(panel_button("Nudge right").on_click(cx.listener(
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
                            .child(panel_button("Duplicate clip").on_click(cx.listener(
                                |editor, _, _, cx| {
                                    editor.duplicate_selected();
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
                            .child("Select a timeline clip to view its properties.")
                    }),
            )
            .into_any_element()
    }
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

fn properties_value(label: &str, value: String) -> gpui::Div {
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

fn asset_description(asset: &MediaAsset) -> String {
    match asset.kind {
        MediaKind::Image => format!("{} image · {}×{}", asset.codec, asset.width, asset.height),
        MediaKind::Audio => format!("{} audio", asset.codec),
        MediaKind::Video => format!(
            "{} · {}×{} · {:.2} fps",
            asset.codec, asset.width, asset.height, asset.framerate
        ),
    }
}
