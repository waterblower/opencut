use super::*;

#[derive(Clone, Copy)]
enum VideoTransformProperty {
    PositionX,
    PositionY,
    Scale,
    CropLeft,
    CropRight,
    CropTop,
    CropBottom,
}

pub(super) struct VideoTransformInputs {
    position_x: Entity<ExplorerFilter>,
    position_y: Entity<ExplorerFilter>,
    scale: Entity<ExplorerFilter>,
    crop_left: Entity<ExplorerFilter>,
    crop_right: Entity<ExplorerFilter>,
    crop_top: Entity<ExplorerFilter>,
    crop_bottom: Entity<ExplorerFilter>,
}

pub(super) struct OpacityDrag {
    clip_id: u64,
    slider_left: f32,
    slider_width: f32,
    changed: bool,
}

impl VideoTransformInputs {
    pub(super) fn new(return_focus: FocusHandle, cx: &mut Context<Editor>) -> Self {
        let field = |cx: &mut Context<Editor>, id, value: &str| {
            cx.new(|cx| {
                ExplorerFilter::new_inline_field(
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
            crop_left: field(cx, "transform-crop-left-input", "0.0"),
            crop_right: field(cx, "transform-crop-right-input", "0.0"),
            crop_top: field(cx, "transform-crop-top-input", "0.0"),
            crop_bottom: field(cx, "transform-crop-bottom-input", "0.0"),
        }
    }

    fn input(&self, property: VideoTransformProperty) -> Entity<ExplorerFilter> {
        match property {
            VideoTransformProperty::PositionX => self.position_x.clone(),
            VideoTransformProperty::PositionY => self.position_y.clone(),
            VideoTransformProperty::Scale => self.scale.clone(),
            VideoTransformProperty::CropLeft => self.crop_left.clone(),
            VideoTransformProperty::CropRight => self.crop_right.clone(),
            VideoTransformProperty::CropTop => self.crop_top.clone(),
            VideoTransformProperty::CropBottom => self.crop_bottom.clone(),
        }
    }

    fn fields(&self) -> [(VideoTransformProperty, Entity<ExplorerFilter>); 7] {
        [
            (VideoTransformProperty::PositionX, self.position_x.clone()),
            (VideoTransformProperty::PositionY, self.position_y.clone()),
            (VideoTransformProperty::Scale, self.scale.clone()),
            (VideoTransformProperty::CropLeft, self.crop_left.clone()),
            (VideoTransformProperty::CropRight, self.crop_right.clone()),
            (VideoTransformProperty::CropTop, self.crop_top.clone()),
            (VideoTransformProperty::CropBottom, self.crop_bottom.clone()),
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
        let Some(clip_id) = self.selected_clip_id else {
            self.video_transform_input_clip_id = None;
            return;
        };
        if self.selected_clip_ids.len() != 1 || self.video_transform_input_clip_id == Some(clip_id)
        {
            return;
        }
        let Some(clip) = self.project.clip(clip_id) else {
            self.video_transform_input_clip_id = None;
            return;
        };
        let properties = clip.video_properties;
        self.video_transform_input_clip_id = Some(clip_id);
        let values = [
            (VideoTransformProperty::PositionX, properties.position_x),
            (VideoTransformProperty::PositionY, properties.position_y),
            (VideoTransformProperty::Scale, properties.scale * 100.0),
            (
                VideoTransformProperty::CropLeft,
                properties.crop_left * 100.0,
            ),
            (
                VideoTransformProperty::CropRight,
                properties.crop_right * 100.0,
            ),
            (VideoTransformProperty::CropTop, properties.crop_top * 100.0),
            (
                VideoTransformProperty::CropBottom,
                properties.crop_bottom * 100.0,
            ),
        ];
        for (property, value) in values {
            let text = format_transform_value(value);
            let input = self.video_transform_inputs.input(property);
            if input.read(cx).query() != text {
                input.update(cx, |input, _| input.set_text_silently(text));
            }
        }
    }

    pub(super) fn video_transform_panel(
        &self,
        clip_id: u64,
        properties: VideoClipProperties,
        editable: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        div()
            .id("video-transform-properties")
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
                    .child(properties_tab("Transform", true)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .px_5()
                    .py_5()
                    .child(properties_section_label("POSITION & SCALE"))
                    .child(self.video_transform_field(
                        VideoTransformProperty::PositionX,
                        "Position X",
                        "px",
                        "transform-position-x",
                        editable,
                    ))
                    .child(self.video_transform_field(
                        VideoTransformProperty::PositionY,
                        "Position Y",
                        "px",
                        "transform-position-y",
                        editable,
                    ))
                    .child(self.video_transform_field(
                        VideoTransformProperty::Scale,
                        "Scale",
                        "%",
                        "transform-scale",
                        editable,
                    ))
                    .child(self.video_opacity_control(clip_id, properties.opacity, editable, cx))
                    .child(
                        div()
                            .mt_2()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(properties_section_label("CROP"))
                            .child(crop_reset_button(editable).when(editable, |button| {
                                button.on_click(cx.listener(move |editor, _, _, cx| {
                                    editor.reset_video_crop(clip_id);
                                    cx.notify();
                                }))
                            })),
                    )
                    .child(
                        div()
                            .grid()
                            .grid_cols(2)
                            .gap_3()
                            .child(self.video_crop_field(
                                VideoTransformProperty::CropLeft,
                                "Left",
                                "transform-crop-left",
                                editable,
                            ))
                            .child(self.video_crop_field(
                                VideoTransformProperty::CropRight,
                                "Right",
                                "transform-crop-right",
                                editable,
                            ))
                            .child(self.video_crop_field(
                                VideoTransformProperty::CropTop,
                                "Top",
                                "transform-crop-top",
                                editable,
                            ))
                            .child(self.video_crop_field(
                                VideoTransformProperty::CropBottom,
                                "Bottom",
                                "transform-crop-bottom",
                                editable,
                            )),
                    ),
            )
            .into_any_element()
    }

    fn video_transform_field(
        &self,
        property: VideoTransformProperty,
        label: &'static str,
        unit: &'static str,
        field_id: &'static str,
        editable: bool,
    ) -> gpui::AnyElement {
        let input = self.video_transform_inputs.input(property);
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
                    .child(input)
                    .child(div().text_sm().text_color(rgb(MUTED)).child(unit))
                    .when(editable, |field| {
                        field.hover(|style| style.border_color(rgb(0x4a4a52)))
                    })
                    .when(!editable, disabled_field_overlay),
            )
            .into_any_element()
    }

    fn video_crop_field(
        &self,
        property: VideoTransformProperty,
        label: &'static str,
        field_id: &'static str,
        editable: bool,
    ) -> gpui::AnyElement {
        let input = self.video_transform_inputs.input(property);
        div()
            .min_w_0()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .w(px(58.0))
                    .flex_shrink_0()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child(label),
            )
            .child(
                div()
                    .id(field_id)
                    .h(px(46.0))
                    .relative()
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_between()
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
                    .child(input)
                    .child(div().text_sm().text_color(rgb(MUTED)).child("%"))
                    .when(editable, |field| {
                        field.hover(|style| style.border_color(rgb(0x4a4a52)))
                    })
                    .when(!editable, disabled_field_overlay),
            )
            .into_any_element()
    }

    fn video_opacity_control(
        &self,
        clip_id: u64,
        opacity: f64,
        editable: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let opacity = opacity.clamp(0.0, 1.0);
        div()
            .h(px(42.0))
            .flex()
            .items_center()
            .gap_4()
            .child(
                div()
                    .w(px(112.0))
                    .flex_shrink_0()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child("Opacity"),
            )
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .id("transform-opacity-slider")
                            .relative()
                            .h(px(24.0))
                            .min_w_0()
                            .flex_1()
                            .flex()
                            .items_center()
                            .cursor(if editable {
                                CursorStyle::PointingHand
                            } else {
                                CursorStyle::Arrow
                            })
                            .child(
                                div()
                                    .absolute()
                                    .left_0()
                                    .right_0()
                                    .h(px(4.0))
                                    .rounded_full()
                                    .bg(rgb(0x45454d)),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .left_0()
                                    .w(gpui::relative(opacity as f32))
                                    .h(px(4.0))
                                    .rounded_full()
                                    .bg(rgb(ACCENT)),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .left(gpui::relative(opacity as f32))
                                    .ml(px(-8.0))
                                    .size(px(16.0))
                                    .rounded_full()
                                    .bg(rgb(0xf7f7f8)),
                            )
                            .when(editable, |slider| {
                                slider.on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(move |editor, event, window, cx| {
                                        editor.begin_video_opacity_drag(clip_id, event, window, cx);
                                    }),
                                )
                            }),
                    )
                    .child(
                        div()
                            .w(px(38.0))
                            .font_family("monospace")
                            .text_base()
                            .text_color(rgb(if editable { TEXT } else { MUTED }))
                            .child(format!("{:.0}", opacity * 100.0)),
                    ),
            )
            .into_any_element()
    }

    fn begin_video_opacity_drag(
        &mut self,
        clip_id: u64,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.clip_locked(clip_id) {
            return;
        }
        let viewport_width = f32::from(window.viewport_size().width);
        let editor_width = (viewport_width - crate::gpui_inspector::docked_width(window)).max(0.0);
        let slider_left = editor_width - self.properties_panel_width + 148.0;
        let slider_width = (self.properties_panel_width - 218.0).max(1.0);
        self.opacity_drag = Some(OpacityDrag {
            clip_id,
            slider_left,
            slider_width,
            changed: false,
        });
        self.apply_video_opacity_drag(f32::from(event.position.x), cx);
        cx.stop_propagation();
    }

    pub(super) fn update_video_opacity_drag(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.opacity_drag.is_some() && event.dragging() {
            self.apply_video_opacity_drag(f32::from(event.position.x), cx);
        }
    }

    pub(super) fn finish_video_opacity_drag(
        &mut self,
        event: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.button != MouseButton::Left || self.opacity_drag.is_none() {
            return;
        }
        self.apply_video_opacity_drag(f32::from(event.position.x), cx);
        let changed = self.opacity_drag.take().is_some_and(|drag| drag.changed);
        if changed {
            self.save_project();
        }
        cx.notify();
    }

    fn apply_video_opacity_drag(&mut self, pointer_x: f32, cx: &mut Context<Self>) {
        let Some(mut drag) = self.opacity_drag.take() else {
            return;
        };
        let opacity = opacity_from_pointer(pointer_x, drag.slider_left, drag.slider_width);
        let Some(index) = self.project.clip_index(drag.clip_id) else {
            self.opacity_drag = None;
            return;
        };
        if (self.project.clips[index].video_properties.opacity - opacity).abs() <= f64::EPSILON {
            self.opacity_drag = Some(drag);
            return;
        }
        if !drag.changed {
            self.checkpoint();
            drag.changed = true;
        }
        self.project.clips[index].video_properties.opacity = opacity;
        self.opacity_drag = Some(drag);
        self.preview_refresh_ticks = 2;
        cx.notify();
    }

    fn set_video_transform_from_text(&mut self, property: VideoTransformProperty, text: &str) {
        let Some(clip_id) = self.video_transform_input_clip_id else {
            return;
        };
        if self.clip_locked(clip_id) {
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
        let Some(index) = self.project.clip_index(clip_id) else {
            return;
        };
        let mut properties = self.project.clips[index].video_properties;
        match property {
            VideoTransformProperty::PositionX => properties.position_x = value,
            VideoTransformProperty::PositionY => properties.position_y = value,
            VideoTransformProperty::Scale => properties.scale = value.clamp(0.0, 100.0),
            VideoTransformProperty::CropLeft => {
                properties.crop_left = value.clamp(0.0, 0.99 - properties.crop_right)
            }
            VideoTransformProperty::CropRight => {
                properties.crop_right = value.clamp(0.0, 0.99 - properties.crop_left)
            }
            VideoTransformProperty::CropTop => {
                properties.crop_top = value.clamp(0.0, 0.99 - properties.crop_bottom)
            }
            VideoTransformProperty::CropBottom => {
                properties.crop_bottom = value.clamp(0.0, 0.99 - properties.crop_top)
            }
        }
        if properties == self.project.clips[index].video_properties {
            return;
        }
        self.checkpoint();
        self.project.clips[index].video_properties = properties;
        self.preview_refresh_ticks = 2;
        self.save_project();
    }

    fn reset_video_crop(&mut self, clip_id: u64) {
        if self.clip_locked(clip_id) {
            return;
        }
        let Some(index) = self.project.clip_index(clip_id) else {
            return;
        };
        let mut properties = self.project.clips[index].video_properties;
        if properties.crop_left == 0.0
            && properties.crop_right == 0.0
            && properties.crop_top == 0.0
            && properties.crop_bottom == 0.0
        {
            return;
        }
        properties.crop_left = 0.0;
        properties.crop_right = 0.0;
        properties.crop_top = 0.0;
        properties.crop_bottom = 0.0;
        self.checkpoint();
        self.project.clips[index].video_properties = properties;
        self.video_transform_input_clip_id = None;
        self.preview_refresh_ticks = 2;
        self.save_project();
    }
}

fn format_transform_value(value: f64) -> String {
    let formatted = format!("{value:.4}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

fn disabled_field_overlay(field: gpui::Stateful<gpui::Div>) -> gpui::Stateful<gpui::Div> {
    field.child(div().absolute().inset_0().occlude())
}

fn opacity_from_pointer(pointer_x: f32, slider_left: f32, slider_width: f32) -> f64 {
    ((pointer_x - slider_left) / slider_width.max(1.0)).clamp(0.0, 1.0) as f64
}

fn properties_tab(label: &'static str, active: bool) -> gpui::Div {
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

fn properties_section_label(label: &'static str) -> gpui::Div {
    div()
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(rgb(MUTED))
        .child(label)
}

fn crop_reset_button(enabled: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id("reset-video-crop")
        .h_8()
        .px_2()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .cursor(if enabled {
            CursorStyle::PointingHand
        } else {
            CursorStyle::Arrow
        })
        .text_sm()
        .text_color(rgb(if enabled { ACCENT } else { MUTED }))
        .when(enabled, |this| {
            this.hover(|style| style.bg(rgb(SURFACE_HOVER)))
        })
        .child("Reset")
}

#[cfg(test)]
#[path = "properties_transform.test.rs"]
mod tests;
