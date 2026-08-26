use super::properties_transform::{properties_section_label, properties_tab};
use super::*;
use gpui_component::input::InputState;

#[derive(IntoElement)]
pub(super) struct TextClipPropertiesView {
    clip: TextClip,
}

impl TextClipPropertiesView {
    pub(super) fn new(clip: TextClip) -> Self {
        Self { clip }
    }
}

impl RenderOnce for TextClipPropertiesView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let initial_text = self.clip.properties.text.clone();
        let _text_input = window.use_keyed_state(
            format!("text-clip-{}-text-input", self.clip.id),
            cx,
            move |window, cx| InputState::new(window, cx).default_value(initial_text),
        );
        let clip = self.clip;
        let property_field =
            |label: &'static str, value: String, unit: &'static str, color: Option<u32>| {
                div()
                    .h(px(48.0))
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .w(px(112.0))
                            .flex_shrink_0()
                            .text_sm()
                            .text_color(rgb(MUTED))
                            .child(label),
                    )
                    .child(
                        div()
                            .h(px(48.0))
                            .min_w_0()
                            .flex_1()
                            .flex()
                            .items_center()
                            .gap_2()
                            .px_3()
                            .rounded_md()
                            .border_1()
                            .border_color(rgb(BORDER))
                            .bg(rgb(SURFACE))
                            .when_some(color, |field, color| {
                                field.child(
                                    div()
                                        .size(px(18.0))
                                        .flex_shrink_0()
                                        .rounded_sm()
                                        .border_1()
                                        .border_color(rgb(BORDER))
                                        .bg(gpui::rgba(color)),
                                )
                            })
                            .child(
                                div()
                                    .min_w_0()
                                    .flex_1()
                                    .text_sm()
                                    .text_ellipsis()
                                    .child(value),
                            )
                            .when(!unit.is_empty(), |field| {
                                field.child(div().text_sm().text_color(rgb(MUTED)).child(unit))
                            }),
                    )
            };

        div()
            .id("text-clip-properties-v2")
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgb(PANEL))
            .child(
                div()
                    .h(px(58.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_5()
                    .px_5()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(properties_tab("Text", true)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .px_5()
                    .py_5()
                    .child(properties_section_label("CONTENT"))
                    .child(property_field(
                        "Text",
                        clip.properties.text.clone(),
                        "",
                        None,
                    ))
                    .child(property_field(
                        "Length",
                        format!("{:.3}", clip.length.as_secs_f64()),
                        "s",
                        None,
                    ))
                    .child(properties_section_label("STYLE"))
                    .child(property_field(
                        "Font",
                        clip.properties.font.clone(),
                        "",
                        None,
                    ))
                    .child(property_field(
                        "Font size",
                        format!("{}", clip.properties.font_size),
                        "px",
                        None,
                    ))
                    .child(property_field(
                        "Color",
                        format!("#{:08X}", clip.properties.color),
                        "",
                        Some(clip.properties.color),
                    ))
                    .child(properties_section_label("POSITION"))
                    .child(property_field(
                        "Position X",
                        format!("{}", clip.properties.position_x),
                        "",
                        None,
                    ))
                    .child(property_field(
                        "Position Y",
                        format!("{}", clip.properties.position_y),
                        "",
                        None,
                    )),
            )
            .into_any_element()
    }
}
