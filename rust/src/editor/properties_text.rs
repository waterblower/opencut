use super::properties_transform::{properties_section_label, properties_tab};
use super::*;
use gpui_component::input::{Input, InputEvent, InputState};

#[derive(IntoElement)]
pub(super) struct TextClipPropertiesView {
    clip: TextClip,
    event_bus: Entity<EventBus>,
}

impl TextClipPropertiesView {
    pub(super) fn new(clip: TextClip, event_bus: Entity<EventBus>) -> Self {
        Self { clip, event_bus }
    }
}

impl RenderOnce for TextClipPropertiesView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let clip = self.clip;
        let event_bus = self.event_bus;

        let text_input_state = {
            let clip_id = clip.id;
            let initial_clip = clip.clone();
            let initial_text = initial_clip.properties.text.clone();
            window.use_keyed_state(
                format!("text-clip-{clip_id}-text-input"),
                cx,
                move |window, cx| {
                    let input =
                        cx.new(|cx| InputState::new(window, cx).default_value(initial_text));
                    let subscription =
                        cx.subscribe(&input, move |_, input, event: &InputEvent, cx| {
                            if let InputEvent::Change = event {
                                let new_value = input.read(cx).value();
                                eprintln!("text input changed: {}", new_value);
                                let edit_action = EditAction::UpdateClip {
                                    clip: Clip::Text(TextClip {
                                        properties: TextClipProperties {
                                            text: new_value.to_string(),
                                            ..initial_clip.properties.clone()
                                        },
                                        ..initial_clip.clone()
                                    }),
                                };
                                event_bus.update(cx, |_, cx| {
                                    cx.emit(edit_action);
                                });
                            }
                        });
                    TextPropertyInputState {
                        input,
                        _subscription: subscription,
                    }
                },
            )
        };
        let text_input = text_input_state.read(cx).input.clone();
        let text_input_field = div()
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
                    .child("Text"),
            )
            .child(
                Input::new(&text_input)
                    .aria_label("Text")
                    .appearance(false)
                    .border_1()
                    .focus_bordered(false)
                    .h(px(48.0))
                    .min_w_0()
                    .flex_1()
                    .bg(rgb(0x000000)),
            );
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
            .h_full()
            .min_h_0()
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
                    .id("text-clip-properties-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .px_5()
                    .py_5()
                    .child(properties_section_label("CONTENT"))
                    .child(text_input_field)
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

struct TextPropertyInputState {
    input: Entity<InputState>,
    _subscription: gpui::Subscription,
}
