use super::*;

impl Player {
    pub(super) fn inspector_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        div()
            .id("inspector-panel")
            .w(px(INSPECTOR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(rgb(0x303036))
            .bg(rgb(0x111114))
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
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Inspector"),
                    )
                    .child(
                        div()
                            .id("close-inspector")
                            .size_8()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor(CursorStyle::PointingHand)
                            .rounded_md()
                            .text_color(rgb(MUTED))
                            .hover(|style| style.bg(rgb(SURFACE_HOVER)).text_color(rgb(TEXT)))
                            .child("×")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.inspector_open = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(0x777780))
                            .child("RENDERING"),
                    )
                    .child(
                        div()
                            .h(px(64.0))
                            .flex()
                            .items_center()
                            .justify_between()
                            .rounded_lg()
                            .border_1()
                            .border_color(rgb(BORDER))
                            .bg(rgb(0x17171a))
                            .px_4()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(div().size_2().rounded_full().bg(rgb(0x63d68b)))
                                    .child("Render FPS"),
                            )
                            .child(
                                div()
                                    .font_family("monospace")
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(rgb(ACCENT))
                                    .child(format!("{:.1}", self.render_fps)),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x606068))
                            .child("GPUI render passes per second"),
                    ),
            )
            .into_any_element()
    }
}
