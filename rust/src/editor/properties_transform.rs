use super::*;

#[derive(Clone, Copy)]
enum VideoTransformProperty {
    PositionX,
    PositionY,
    Scale,
}

pub(super) struct VideoTransformInputs {
    position_x: Entity<ExplorerFilter>,
    position_y: Entity<ExplorerFilter>,
    scale: Entity<ExplorerFilter>,
}

impl VideoTransformInputs {
    pub(super) fn new(return_focus: FocusHandle, cx: &mut Context<Editor>) -> Self {
        let field = |cx: &mut Context<Editor>, id, value: &str| {
            cx.new(|cx| {
                ExplorerFilter::new_inline_number_field(
                    id,
                    value.to_string(),
                    "0.0",
                    return_focus.clone(),
                    cx,
                )
            })
        };
        Self {
            position_x: field(cx, "transform-position-x-input", "0.0"),
            position_y: field(cx, "transform-position-y-input", "0.0"),
            scale: field(cx, "transform-scale-input", "100.0"),
        }
    }

    fn input(&self, property: VideoTransformProperty) -> Entity<ExplorerFilter> {
        match property {
            VideoTransformProperty::PositionX => self.position_x.clone(),
            VideoTransformProperty::PositionY => self.position_y.clone(),
            VideoTransformProperty::Scale => self.scale.clone(),
        }
    }

    fn fields(&self) -> [(VideoTransformProperty, Entity<ExplorerFilter>); 3] {
        [
            (VideoTransformProperty::PositionX, self.position_x.clone()),
            (VideoTransformProperty::PositionY, self.position_y.clone()),
            (VideoTransformProperty::Scale, self.scale.clone()),
        ]
    }
}

impl Editor {
    pub(super) fn observe_video_transform_inputs(
        inputs: &VideoTransformInputs,
        cx: &mut Context<Self>,
    ) {
        for (property, input) in inputs.fields() {
            let observed_input = input.clone();
            cx.observe(&input, move |editor, _, cx| {
                let value = observed_input.read(cx).query().to_string();
                editor.set_video_transform_from_text(property, &value);
                cx.notify();
            })
            .detach();
        }
    }

    pub(super) fn sync_video_transform_inputs(&mut self, cx: &mut Context<Self>) {
        let Some(timeline) = self.timeline.as_ref() else {
            self.properties.transform_input_clip_id = None;
            return;
        };
        let Some(clip_id) = timeline.interaction.selected_clip_id else {
            self.properties.transform_input_clip_id = None;
            return;
        };
        if timeline.interaction.selected_clip_ids.len() != 1
            || self.properties.transform_input_clip_id == Some(clip_id)
        {
            return;
        }
        let Some(clip) = timeline.data.clip(clip_id) else {
            self.properties.transform_input_clip_id = None;
            return;
        };
        let Some(clip) = clip.media() else {
            self.properties.transform_input_clip_id = None;
            return;
        };
        let properties = clip.video_properties;
        self.properties.transform_input_clip_id = Some(clip_id);
        let values = [
            (VideoTransformProperty::PositionX, properties.position_x),
            (VideoTransformProperty::PositionY, properties.position_y),
            (VideoTransformProperty::Scale, properties.scale * 100.0),
        ];
        for (property, value) in values {
            let text = format_transform_value(value);
            let input = self.properties.transform_inputs.input(property);
            if input.read(cx).query() != text {
                input.update(cx, |input, _| input.set_text_silently(text));
            }
        }
    }
}

impl Editor {
    fn set_video_transform_from_text(&mut self, property: VideoTransformProperty, text: &str) {
        let Some(clip_id) = self.properties.transform_input_clip_id else {
            return;
        };
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        if timeline.data.clip_locked(clip_id) {
            return;
        }
        let Ok(mut value) = text.trim().parse::<f64>() else {
            return;
        };
        if !value.is_finite() {
            return;
        }
        if !matches!(
            property,
            VideoTransformProperty::PositionX | VideoTransformProperty::PositionY
        ) {
            value /= 100.0;
        }
        let Some(index) = timeline.data.clip_index(clip_id) else {
            return;
        };
        let Some(clip) = timeline.data.clips[index].media() else {
            return;
        };
        let mut properties = clip.video_properties;
        match property {
            VideoTransformProperty::PositionX => properties.position_x = value,
            VideoTransformProperty::PositionY => properties.position_y = value,
            VideoTransformProperty::Scale => properties.scale = value.clamp(0.0, 100.0),
        }
        if properties == clip.video_properties {
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
            EditAction::SetVideoProperties {
                clip_ids: vec![clip_id],
                properties,
            },
        )
        .expect("setting video properties cannot be rejected");

        timeline.save(&self.global_settings.project_root);
    }
}

fn format_transform_value(value: f64) -> String {
    let formatted = format!("{value:.4}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

pub(super) fn properties_tab(label: &'static str, active: bool) -> gpui::Div {
    div()
        .h_full()
        .flex()
        .items_center()
        .border_b_2()
        .border_color(if active { rgb(ACCENT) } else { rgb(0x00000000) })
        .text_sm()
        .text_color(rgb(if active { TEXT } else { MUTED }))
        .child(label)
}

pub(super) fn properties_section_label(label: &'static str) -> gpui::Div {
    div()
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(rgb(MUTED))
        .child(label)
}
