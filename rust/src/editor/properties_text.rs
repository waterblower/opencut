use super::*;

pub(super) struct TextClipInputs {
    text: Entity<ExplorerFilter>,
    length: Entity<ExplorerFilter>,
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
            length: number_field(cx, "text-clip-length-input", "5", "5"),
            font_size: number_field(cx, "text-clip-font-size-input", "64", "64"),
            color: field(cx, "text-clip-color-input", "#FFFFFFFF", "#FFFFFFFF"),
            position_x: number_field(cx, "text-clip-position-x-input", "0.5", "0.5"),
            position_y: number_field(cx, "text-clip-position-y-input", "0.5", "0.5"),
        }
    }

    fn fields(&self) -> [(TextProperty, Entity<ExplorerFilter>); 6] {
        [
            (TextProperty::Text, self.text.clone()),
            (TextProperty::Length, self.length.clone()),
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
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum TextProperty {
    Text,
    Length,
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
        if property == TextProperty::Length {
            let Some(length) = text_length_with_input(timeline.data.settings.frame_rate, text)
            else {
                return;
            };
            if length == clip.length {
                return;
            }
            let clip_length = timeline
                .data
                .settings
                .frame_rate
                .frames_from_duration_nearest(length);
            if validate_text_clip_placement(
                &timeline.data,
                clip.track_id,
                clip_length,
                clip.timeline_start,
                &HashSet::from([clip_id]),
            )
            .is_err()
            {
                return;
            }
            let Some(timeline) = self.timeline.as_mut() else {
                return;
            };
            timeline.record_editing_history();
            edit_and_rebuild_timeline(
                &mut self.preview,
                &self.global_settings.project_root,
                timeline,
                EditAction::SetTextLength { clip_id, length },
            )
            .expect("text clip length was validated before recording history");
            timeline.save(&self.global_settings.project_root);
            return;
        }
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
        edit_and_rebuild_timeline(
            &mut self.preview,
            &self.global_settings.project_root,
            timeline,
            EditAction::SetTextProperties {
                clip_id,
                properties,
            },
        )
        .expect("setting text properties cannot be rejected");
        timeline.save(&self.global_settings.project_root);
    }
}

fn finite_text_number(text: &str) -> Result<f64, ()> {
    let value = text.trim().parse::<f64>().map_err(|_| ())?;
    value.is_finite().then_some(value).ok_or(())
}

fn text_length_with_input(frame_rate: FrameRate, text: &str) -> Option<Duration> {
    let seconds = finite_text_number(text).ok()?;
    let requested = Duration::try_from_secs_f64(seconds).ok()?;
    let frames = frame_rate
        .frames_from_duration_nearest(requested)
        .max(TimelineTime::ONE_FRAME);
    Some(frame_rate.duration(frames))
}

fn text_properties_with_input(
    current: &TextClipProperties,
    property: TextProperty,
    text: &str,
) -> Option<TextClipProperties> {
    let mut properties = current.clone();
    match property {
        TextProperty::Text => properties.text = text.to_string(),
        TextProperty::Length => return None,
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

    #[test]
    fn quantizes_text_length_to_timeline_frames() {
        assert_eq!(
            text_length_with_input(FrameRate::new(30, 1), "2.5"),
            Some(Duration::from_millis(2500))
        );
        assert_eq!(
            text_length_with_input(FrameRate::new(30, 1), "0"),
            Some(FrameRate::new(30, 1).duration(TimelineTime::ONE_FRAME))
        );
        assert_eq!(text_length_with_input(FrameRate::new(30, 1), "-1"), None);
    }
}
