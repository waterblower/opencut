use crate::edit_action::EditAction;
use crate::event_bus::{AppEvent, EventBus};
use crate::generic_containers::{TextInput, TextInputEvent};
use crate::properties_transform::{properties_section_label, properties_tab};
use crate::theme::{BORDER, MUTED, PANEL, SURFACE};
use crate::timeline_clip::TextClip;
use gpui::prelude::*;
use gpui::{App, Entity, FocusHandle, Window, div, px, rgb};

#[derive(IntoElement)]
pub(super) struct TextClipPropertiesView {
    clip: TextClip,
    event_bus: Entity<EventBus>,
    return_focus: FocusHandle,
}

impl TextClipPropertiesView {
    pub(super) fn new(
        clip: TextClip,
        event_bus: Entity<EventBus>,
        return_focus: FocusHandle,
    ) -> Self {
        Self {
            clip,
            event_bus,
            return_focus,
        }
    }
}

impl RenderOnce for TextClipPropertiesView {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let clip = self.clip;
        let event_bus = self.event_bus;
        let return_focus = self.return_focus;

        let text_input_state = {
            let clip_id = clip.id;
            let event_bus = event_bus.clone();
            let return_focus = return_focus.clone();
            let initial_text = clip.properties.text.clone();
            window.use_keyed_state(
                format!("text-clip-{clip_id}-text-input"),
                cx,
                move |_, cx| {
                    let input = cx.new(|cx| {
                        TextInput::new_inline_field(
                            "text-clip-text-input",
                            initial_text,
                            "",
                            return_focus,
                            cx,
                        )
                    });
                    let subscription =
                        cx.subscribe(&input, move |_, input, _: &TextInputEvent, cx| {
                            let new_value = input.read(cx).text().to_string();
                            eprintln!("text input changed: {}", new_value);
                            let edit_action = EditAction::SetTextContent {
                                clip_id,
                                text: new_value,
                            };
                            event_bus.update(cx, |_, cx| {
                                cx.emit(AppEvent::Edit(edit_action));
                            });
                        });
                    TextPropertyInputState {
                        input,
                        _subscription: subscription,
                    }
                },
            )
        };
        let color_input_state = {
            let clip_id = clip.id;
            let initial_color = format_text_color(clip.properties.color);
            let event_bus = event_bus.clone();
            let return_focus = return_focus.clone();
            window.use_keyed_state(
                format!("text-clip-{clip_id}-color-input"),
                cx,
                move |_, cx| {
                    let input = cx.new(|cx| {
                        TextInput::new_inline_field(
                            "text-clip-color-input",
                            initial_color,
                            "#FFFFFF",
                            return_focus,
                            cx,
                        )
                    });
                    let subscription =
                        cx.subscribe(&input, move |_, input, _: &TextInputEvent, cx| {
                            // 输入未完成或无效时不修改；只有合法的十六进制颜色才生效。
                            let Some(color) = parse_text_color(input.read(cx).text()) else {
                                return;
                            };
                            event_bus.update(cx, |_, cx| {
                                cx.emit(AppEvent::Edit(EditAction::SetTextColor {
                                    clip_id,
                                    color,
                                }));
                            });
                        });
                    TextPropertyInputState {
                        input,
                        _subscription: subscription,
                    }
                },
            )
        };
        let color_input = color_input_state.read(cx).input.clone();
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
                div()
                    .h(px(48.0))
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .items_center()
                    .px_3()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(SURFACE))
                    .child(text_input),
            );
        let property_field = |label: &'static str, value: String, unit: &'static str| {
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
                        "Duration",
                        format!("{:.2}", clip.duration.as_secs_f64()),
                        "s",
                    ))
                    .child(properties_section_label("STYLE"))
                    .child(property_field("Font", clip.properties.font.clone(), ""))
                    .child(property_field(
                        "Font size",
                        format!("{}", clip.properties.font_size),
                        "px",
                    ))
                    .child(
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
                                    .child("Color"),
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
                                    .child(
                                        div()
                                            .size(px(18.0))
                                            .flex_shrink_0()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(rgb(BORDER))
                                            .bg(gpui::rgba(clip.properties.color.rotate_left(8))), // ARGB → RGBA。
                                    )
                                    .child(div().min_w_0().flex_1().child(color_input)),
                            ),
                    )
                    .child(properties_section_label("POSITION"))
                    .child(property_field(
                        "Position X",
                        format!("{:.2}", clip.properties.position.x),
                        "",
                    ))
                    .child(property_field(
                        "Position Y",
                        format!("{:.2}", clip.properties.position.y),
                        "",
                    )),
            )
            .into_any_element()
    }
}

struct TextPropertyInputState {
    input: Entity<TextInput>,
    _subscription: gpui::Subscription,
}

/// `#RRGGBB` when opaque, otherwise `#RRGGBBAA`; `color` is big-endian ARGB.
fn format_text_color(color: u32) -> String {
    let alpha = color >> 24;
    let rgb = color & 0x00ff_ffff;
    if alpha == 0xff {
        format!("#{rgb:06X}")
    } else {
        format!("#{rgb:06X}{alpha:02X}")
    }
}

/// Parses `#RRGGBB` or `#RRGGBBAA` (the `#` is optional) into big-endian ARGB.
fn parse_text_color(text: &str) -> Option<u32> {
    let digits = text.trim().trim_start_matches('#');
    if !digits.chars().all(|digit| digit.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(digits, 16).ok()?;
    match digits.len() {
        6 => Some(0xff00_0000 | value),
        8 => Some(value.rotate_right(8)),
        _ => None,
    }
}
