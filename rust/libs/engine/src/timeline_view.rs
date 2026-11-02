//! GPUI views for composed timeline pictures and subtitle layout.

use crate::timeline_decoder::{PreparedLayer, TimelineFrameComposition};
use crate::video_surface::{retired_surfaces_release_view, video_surface_size, video_surface_view};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, IntoElement, ObjectFit, ParentElement, Styled, TextAlign, div, img, px, rgb, rgba,
};
use std::sync::Arc;
use timeline::{Clip, TextClipProperties, TimelineEditingState};

impl TimelineFrameComposition {
    /// Fits the composition into the given logical dimensions and applies layer transforms.
    pub fn render_frame(
        &self,
        width: f32,
        height: f32,
        timeline: &TimelineEditingState,
    ) -> AnyElement {
        let root = div()
            .w(px(width.max(0.0)))
            .h(px(height.max(0.0)))
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .bg(rgb(0))
            .child(retired_surfaces_release_view()); // 无视频层（如缩放为 0、片段结束）时也回收旧帧。
        let scale = (width / self.width as f32).min(height / self.height as f32);
        if !scale.is_finite() || scale <= 0.0 {
            return root.into_any_element();
        }
        let canvas_width = self.width as f32 * scale;
        let canvas_height = self.height as f32 * scale;
        let mut canvas = div()
            .relative()
            .flex_shrink_0()
            .overflow_hidden()
            .w(px(canvas_width))
            .h(px(canvas_height));
        for layer in &self.layers {
            let (content, source_width, source_height, properties) = match layer {
                PreparedLayer::VideoFrame {
                    frame, properties, ..
                } => {
                    let (width, height) = video_surface_size(frame);
                    (
                        video_surface_view(frame, ObjectFit::Contain),
                        width,
                        height,
                        properties,
                    )
                }
                PreparedLayer::Image {
                    image, properties, ..
                } => {
                    let size = image.size(0);
                    (
                        img(Arc::clone(image)).size_full().into_any_element(),
                        size.width.0 as f32,
                        size.height.0 as f32,
                        properties,
                    )
                }
                PreparedLayer::Text { clip_id } => {
                    let Some(Clip::Text(clip)) = timeline.clip(*clip_id) else {
                        continue;
                    };
                    let properties = &clip.properties;
                    canvas = canvas.child(
                        div()
                            .absolute()
                            .left(px(properties.position.x as f32 * canvas_width))
                            .top(px(properties.position.y as f32 * canvas_height))
                            .w(px(0.0))
                            .h(px(0.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(text_layer_element(properties, scale)),
                    );
                    continue;
                }
            };
            if properties.scale <= 0.0 {
                continue;
            }
            let fit = (canvas_width / source_width).min(canvas_height / source_height);
            let width = source_width * fit * properties.scale as f32;
            let height = source_height * fit * properties.scale as f32;
            let x = (canvas_width - width) / 2.0 + properties.position_x as f32 * scale;
            let y = (canvas_height - height) / 2.0 + properties.position_y as f32 * scale;
            canvas = canvas.child(
                div()
                    .child(content)
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(width))
                    .h(px(height)),
            );
        }
        root.child(canvas).into_any_element()
    }
}

/// Shared text layout for picture rendering and editor hit testing.
pub fn text_layer_element(properties: &TextClipProperties, scale: f32) -> AnyElement {
    let font_size = properties.font_size as f32 * scale;
    div()
        .flex_shrink_0()
        .when_some(properties.background, |text, background| {
            // 背景框的内边距随字号缩放，导出与预览一致。
            text.bg(rgba(background.rgba()))
                .px(px(font_size * BACKGROUND_PADDING_X))
                .py(px(font_size * BACKGROUND_PADDING_Y))
        })
        .whitespace_nowrap()
        .font_family(properties.font.clone())
        .text_size(px(properties.font_size as f32 * scale))
        .line_height(px(properties.font_size as f32 * scale * 1.2))
        .text_align(TextAlign::Center)
        .text_color(rgba(properties.color.rotate_left(8)))
        .child(properties.text.clone())
        .into_any_element()
}

const BACKGROUND_PADDING_X: f32 = 0.3; // 背景框左右内边距，相对于字号
const BACKGROUND_PADDING_Y: f32 = 0.1; // 背景框上下内边距，相对于字号
