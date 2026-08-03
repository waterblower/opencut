use super::*;

#[derive(Clone, Copy)]
enum VideoTransformProperty {
    PositionX,
    PositionY,
    Scale,
    Rotation,
    Opacity,
    CropLeft,
    CropRight,
    CropTop,
    CropBottom,
}

impl Editor {
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
                        clip_id,
                        VideoTransformProperty::PositionX,
                        "Position X",
                        format!("{:.1}", properties.position_x),
                        "px",
                        1.0,
                        "transform-position-x",
                        editable,
                        cx,
                    ))
                    .child(self.video_transform_field(
                        clip_id,
                        VideoTransformProperty::PositionY,
                        "Position Y",
                        format!("{:.1}", properties.position_y),
                        "px",
                        1.0,
                        "transform-position-y",
                        editable,
                        cx,
                    ))
                    .child(self.video_transform_field(
                        clip_id,
                        VideoTransformProperty::Scale,
                        "Scale",
                        format!("{:.1}", properties.scale * 100.0),
                        "%",
                        0.01,
                        "transform-scale",
                        editable,
                        cx,
                    ))
                    .child(self.video_transform_field(
                        clip_id,
                        VideoTransformProperty::Rotation,
                        "Rotation",
                        format!("{:.1}", properties.rotation_degrees),
                        "°",
                        1.0,
                        "transform-rotation",
                        editable,
                        cx,
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
                                clip_id,
                                VideoTransformProperty::CropLeft,
                                "Left",
                                properties.crop_left,
                                "transform-crop-left",
                                editable,
                                cx,
                            ))
                            .child(self.video_crop_field(
                                clip_id,
                                VideoTransformProperty::CropRight,
                                "Right",
                                properties.crop_right,
                                "transform-crop-right",
                                editable,
                                cx,
                            ))
                            .child(self.video_crop_field(
                                clip_id,
                                VideoTransformProperty::CropTop,
                                "Top",
                                properties.crop_top,
                                "transform-crop-top",
                                editable,
                                cx,
                            ))
                            .child(self.video_crop_field(
                                clip_id,
                                VideoTransformProperty::CropBottom,
                                "Bottom",
                                properties.crop_bottom,
                                "transform-crop-bottom",
                                editable,
                                cx,
                            )),
                    ),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn video_transform_field(
        &self,
        clip_id: u64,
        property: VideoTransformProperty,
        label: &'static str,
        value: String,
        unit: &'static str,
        step: f64,
        field_id: &'static str,
        editable: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let decrement_property = property;
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
                        CursorStyle::ResizeLeftRight
                    } else {
                        CursorStyle::Arrow
                    })
                    .child(
                        div()
                            .min_w_0()
                            .font_family("monospace")
                            .text_base()
                            .text_color(rgb(if editable { TEXT } else { MUTED }))
                            .child(value),
                    )
                    .child(div().text_sm().text_color(rgb(MUTED)).child(unit))
                    .when(editable, |field| {
                        field
                            .hover(|style| style.border_color(rgb(0x4a4a52)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |editor, _, _, cx| {
                                    editor.adjust_video_transform(clip_id, property, step);
                                    cx.notify();
                                }),
                            )
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |editor, _, _, cx| {
                                    editor.adjust_video_transform(
                                        clip_id,
                                        decrement_property,
                                        -step,
                                    );
                                    cx.notify();
                                }),
                            )
                    }),
            )
            .into_any_element()
    }

    fn video_crop_field(
        &self,
        clip_id: u64,
        property: VideoTransformProperty,
        label: &'static str,
        value: f64,
        field_id: &'static str,
        editable: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let decrement_property = property;
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
                        CursorStyle::ResizeLeftRight
                    } else {
                        CursorStyle::Arrow
                    })
                    .child(
                        div()
                            .font_family("monospace")
                            .text_sm()
                            .text_color(rgb(if editable { TEXT } else { MUTED }))
                            .child(format!("{:.1}%", value.clamp(0.0, 0.99) * 100.0)),
                    )
                    .when(editable, |field| {
                        field
                            .hover(|style| style.border_color(rgb(0x4a4a52)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |editor, _, _, cx| {
                                    editor.adjust_video_transform(clip_id, property, 0.01);
                                    cx.notify();
                                }),
                            )
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |editor, _, _, cx| {
                                    editor.adjust_video_transform(
                                        clip_id,
                                        decrement_property,
                                        -0.01,
                                    );
                                    cx.notify();
                                }),
                            )
                    }),
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
                                slider
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |editor, _, _, cx| {
                                            editor.adjust_video_transform(
                                                clip_id,
                                                VideoTransformProperty::Opacity,
                                                0.05,
                                            );
                                            cx.notify();
                                        }),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Right,
                                        cx.listener(move |editor, _, _, cx| {
                                            editor.adjust_video_transform(
                                                clip_id,
                                                VideoTransformProperty::Opacity,
                                                -0.05,
                                            );
                                            cx.notify();
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

    fn adjust_video_transform(
        &mut self,
        clip_id: u64,
        property: VideoTransformProperty,
        delta: f64,
    ) {
        if self.clip_locked(clip_id) {
            return;
        }
        let Some(index) = self.project.clip_index(clip_id) else {
            return;
        };
        let mut properties = self.project.clips[index].video_properties;
        match property {
            VideoTransformProperty::PositionX => properties.position_x += delta,
            VideoTransformProperty::PositionY => properties.position_y += delta,
            VideoTransformProperty::Scale => {
                properties.scale = (properties.scale + delta).clamp(0.0, 100.0)
            }
            VideoTransformProperty::Rotation => properties.rotation_degrees += delta,
            VideoTransformProperty::Opacity => {
                properties.opacity = (properties.opacity + delta).clamp(0.0, 1.0)
            }
            VideoTransformProperty::CropLeft => {
                properties.crop_left = (properties.crop_left + delta)
                    .clamp(0.0, 0.99 - properties.crop_right.clamp(0.0, 0.99))
            }
            VideoTransformProperty::CropRight => {
                properties.crop_right = (properties.crop_right + delta)
                    .clamp(0.0, 0.99 - properties.crop_left.clamp(0.0, 0.99))
            }
            VideoTransformProperty::CropTop => {
                properties.crop_top = (properties.crop_top + delta)
                    .clamp(0.0, 0.99 - properties.crop_bottom.clamp(0.0, 0.99))
            }
            VideoTransformProperty::CropBottom => {
                properties.crop_bottom = (properties.crop_bottom + delta)
                    .clamp(0.0, 0.99 - properties.crop_top.clamp(0.0, 0.99))
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
        self.preview_refresh_ticks = 2;
        self.save_project();
    }
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
