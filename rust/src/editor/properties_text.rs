use super::properties_transform::{
    disabled_field_overlay, properties_section_label, properties_tab,
};
use super::*;

pub(super) struct TextClipInputs {
    text: Entity<ExplorerFilter>,
    font_size: Entity<ExplorerFilter>,
    color: Entity<ExplorerFilter>,
    position_x: Entity<ExplorerFilter>,
    position_y: Entity<ExplorerFilter>,
}

impl TextClipInputs {
    pub(super) fn new(return_focus: FocusHandle, cx: &mut Context<Editor>) -> Self {
        let field =
            |cx: &mut Context<Editor>, id: &'static str, value: &str, placeholder: &'static str| {
                cx.new(|cx| {
                    ExplorerFilter::new_inline_field(
                        id,
                        value.to_string(),
                        placeholder,
                        return_focus.clone(),
                        cx,
                    )
                })
            };
        let number_field =
            |cx: &mut Context<Editor>, id: &'static str, value: &str, placeholder: &'static str| {
                cx.new(|cx| {
                    ExplorerFilter::new_inline_number_field(
                        id,
                        value.to_string(),
                        placeholder,
                        return_focus.clone(),
                        cx,
                    )
                })
            };
        Self {
            text: field(cx, "text-clip-text-input", "Text", "Enter text"),
            font_size: number_field(cx, "text-clip-font-size-input", "64", "64"),
            color: field(cx, "text-clip-color-input", "#FFFFFFFF", "#FFFFFFFF"),
            position_x: number_field(cx, "text-clip-position-x-input", "0.5", "0.5"),
            position_y: number_field(cx, "text-clip-position-y-input", "0.5", "0.5"),
        }
    }

    fn input(&self, property: TextProperty) -> Entity<ExplorerFilter> {
        match property {
            TextProperty::Text => self.text.clone(),
            TextProperty::FontSize => self.font_size.clone(),
            TextProperty::Color => self.color.clone(),
            TextProperty::PositionX => self.position_x.clone(),
            TextProperty::PositionY => self.position_y.clone(),
        }
    }

    fn fields(&self) -> [(TextProperty, Entity<ExplorerFilter>); 5] {
        [
            (TextProperty::Text, self.text.clone()),
            (TextProperty::FontSize, self.font_size.clone()),
            (TextProperty::Color, self.color.clone()),
            (TextProperty::PositionX, self.position_x.clone()),
            (TextProperty::PositionY, self.position_y.clone()),
        ]
    }
}

impl Editor {
    pub(super) fn observe_text_clip_inputs(inputs: &TextClipInputs, cx: &mut Context<Self>) {
        for (property, input) in inputs.fields() {
            let observed_input = input.clone();
            cx.observe(&input, move |editor, _, cx| {
                let value = observed_input.read(cx).query().to_string();
                editor.set_text_clip_property(property, &value);
                cx.notify();
            })
            .detach();
        }
    }

    pub(super) fn sync_text_clip_inputs(&mut self, cx: &mut Context<Self>) {
        let Some(timeline) = self.timeline.as_ref() else {
            self.properties.text_input_clip_id = None;
            return;
        };
        let Some(clip_id) = timeline.interaction.selected_clip_id else {
            self.properties.text_input_clip_id = None;
            return;
        };
        if timeline.interaction.selected_clip_ids.len() != 1
            || self.properties.text_input_clip_id == Some(clip_id)
        {
            return;
        }
        let Some(clip) = timeline.data.clip(clip_id).and_then(Clip::text) else {
            self.properties.text_input_clip_id = None;
            return;
        };
        self.properties.text_input_clip_id = Some(clip_id);
        let values = [
            (TextProperty::Text, clip.properties.text.clone()),
            (
                TextProperty::FontSize,
                format_text_number(clip.properties.font_size),
            ),
            (
                TextProperty::Color,
                format!("#{:08X}", clip.properties.color),
            ),
            (
                TextProperty::PositionX,
                format_text_number(clip.properties.position_x),
            ),
            (
                TextProperty::PositionY,
                format_text_number(clip.properties.position_y),
            ),
        ];
        for (property, value) in values {
            let input = self.properties.text_inputs.input(property);
            if input.read(cx).query() != value {
                input.update(cx, |input, _| input.set_text_silently(value));
            }
        }
    }
}

pub(super) fn text_clip_panel(
    panel: &PropertiesPanelState,
    editable: bool,
    color: u32,
) -> gpui::AnyElement {
    div()
        .id("text-clip-properties")
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
                .child(text_property_field(
                    panel,
                    TextProperty::Text,
                    "Text",
                    "",
                    "text-clip-text",
                    editable,
                    None,
                ))
                .child(properties_section_label("STYLE"))
                .child(text_property_field(
                    panel,
                    TextProperty::FontSize,
                    "Font size",
                    "px",
                    "text-clip-font-size",
                    editable,
                    None,
                ))
                .child(text_property_field(
                    panel,
                    TextProperty::Color,
                    "Color",
                    "",
                    "text-clip-color",
                    editable,
                    Some(color),
                ))
                .child(properties_section_label("POSITION"))
                .child(text_property_field(
                    panel,
                    TextProperty::PositionX,
                    "Position X",
                    "",
                    "text-clip-position-x",
                    editable,
                    None,
                ))
                .child(text_property_field(
                    panel,
                    TextProperty::PositionY,
                    "Position Y",
                    "",
                    "text-clip-position-y",
                    editable,
                    None,
                )),
        )
        .into_any_element()
}

#[derive(Clone, Copy)]
enum TextProperty {
    Text,
    FontSize,
    Color,
    PositionX,
    PositionY,
}

impl Editor {
    fn set_text_clip_property(&mut self, property: TextProperty, text: &str) {
        let Some(clip_id) = self.properties.text_input_clip_id else {
            return;
        };
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        if timeline.data.clip_locked(clip_id) {
            return;
        }
        let Some(index) = timeline.data.clip_index(clip_id) else {
            return;
        };
        let Some(clip) = timeline.data.clips[index].text() else {
            return;
        };
        let Some(properties) = text_properties_with_input(&clip.properties, property, text) else {
            return;
        };
        if properties == clip.properties {
            return;
        }
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.record_editing_history();
        self.preview.timeline_needs_rebuild = true;
        let Clip::Text(clip) = &mut timeline.data.clips[index] else {
            return;
        };
        clip.properties = properties;
        timeline.save(&self.global_settings.project_root);
        self.rebuild_timeline_preview_if_needed();
    }
}

fn text_property_field(
    panel: &PropertiesPanelState,
    property: TextProperty,
    label: &'static str,
    unit: &'static str,
    field_id: &'static str,
    editable: bool,
    color: Option<u32>,
) -> gpui::AnyElement {
    let input = panel.text_inputs.input(property);
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
                .id(field_id)
                .h(px(48.0))
                .relative()
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .px_3()
                .rounded_md()
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(SURFACE))
                .cursor(if editable {
                    CursorStyle::IBeam
                } else {
                    CursorStyle::Arrow
                })
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
                .child(input)
                .when(!unit.is_empty(), |field| {
                    field.child(div().text_sm().text_color(rgb(MUTED)).child(unit))
                })
                .when(editable, |field| {
                    field.hover(|style| style.border_color(rgb(0x4a4a52)))
                })
                .when(!editable, disabled_field_overlay),
        )
        .into_any_element()
}

fn finite_text_number(text: &str) -> Result<f64, ()> {
    let value = text.trim().parse::<f64>().map_err(|_| ())?;
    value.is_finite().then_some(value).ok_or(())
}

fn text_properties_with_input(
    current: &TextClipProperties,
    property: TextProperty,
    text: &str,
) -> Option<TextClipProperties> {
    let mut properties = current.clone();
    match property {
        TextProperty::Text => properties.text = text.to_string(),
        TextProperty::FontSize => {
            properties.font_size = finite_text_number(text).ok()?.clamp(1.0, 1000.0);
        }
        TextProperty::Color => properties.color = parse_text_color(text)?,
        TextProperty::PositionX => properties.position_x = finite_text_number(text).ok()?,
        TextProperty::PositionY => properties.position_y = finite_text_number(text).ok()?,
    }
    Some(properties)
}

fn parse_text_color(text: &str) -> Option<u32> {
    let text = text.trim().trim_start_matches('#').trim_start_matches("0x");
    match text.len() {
        6 => u32::from_str_radix(text, 16)
            .ok()
            .map(|color| color << 8 | 0xff),
        8 => u32::from_str_radix(text, 16).ok(),
        _ => None,
    }
}

fn format_text_number(value: f64) -> String {
    let formatted = format!("{value:.4}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rgb_and_rgba_text_colors() {
        assert_eq!(parse_text_color("#336699"), Some(0x336699ff));
        assert_eq!(parse_text_color("0x33669980"), Some(0x33669980));
        assert_eq!(parse_text_color("invalid"), None);
    }

    #[test]
    fn applies_supported_text_property_inputs() {
        let properties = TextClipProperties::default();
        assert_eq!(
            text_properties_with_input(&properties, TextProperty::Text, "Title")
                .unwrap()
                .text,
            "Title"
        );
        assert_eq!(
            text_properties_with_input(&properties, TextProperty::FontSize, "72")
                .unwrap()
                .font_size,
            72.0
        );
        assert_eq!(
            text_properties_with_input(&properties, TextProperty::Color, "#336699")
                .unwrap()
                .color,
            0x336699ff
        );
        assert_eq!(
            text_properties_with_input(&properties, TextProperty::PositionX, "0.25")
                .unwrap()
                .position_x,
            0.25
        );
        assert_eq!(
            text_properties_with_input(&properties, TextProperty::PositionY, "0.75")
                .unwrap()
                .position_y,
            0.75
        );
    }
}
