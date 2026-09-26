use crate::editor::timeline_backend::{TimelineBackend, TimelineFrame, TimelineLayer};
use anyhow::Result;
use gpui::{
    AnyElement, App, ElementId, IntoElement, Pixels, RenderImage, RenderOnce, Size, TextAlign,
    Window, div, img, prelude::*, px, rgb, rgba, size,
};
use image::Frame;
use opencut_player::timeline::TimelineTime;
use smallvec::smallvec;
use std::{collections::HashMap, sync::Arc};
use ulid::Ulid;

/// Requests a still frame without waiting for media I/O or decoding.
/// Repeated requests for the same position reuse the backend's prepared frame.
pub fn timeline_preview(
    timeline: &TimelineBackend,
    position: TimelineTime,
) -> TimelinePreviewCanvasElement {
    TimelinePreviewCanvasElement {
        frame: timeline.preview_frame(position),
        id: "timeline-preview".into(),
        size: size(px(0.0), px(0.0)),
    }
}

/// A fitted timeline canvas, including its loading and error presentation.
/// The backend owns decoding, scheduling, and prepared image caching.
#[derive(IntoElement)]
pub struct TimelinePreviewCanvasElement {
    frame: Result<Option<Arc<TimelinePreviewFrame>>>,
    id: ElementId,
    size: Size<Pixels>,
}

impl TimelinePreviewCanvasElement {
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    pub fn size(mut self, width: Pixels, height: Pixels) -> Self {
        self.size = size(width, height);
        self
    }
}

impl RenderOnce for TimelinePreviewCanvasElement {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let canvas = div()
            .id(self.id)
            .w(self.size.width)
            .h(self.size.height)
            .overflow_hidden()
            .flex()
            .items_center()
            .justify_center()
            .bg(rgb(0));
        match self.frame {
            Ok(Some(frame)) => canvas
                .child(frame.render(self.size.width.into(), self.size.height.into()))
                .into_any_element(),
            Ok(None) => {
                window.request_animation_frame();
                canvas.into_any_element()
            }
            Err(_) => canvas
                .text_color(rgb(0xcccccc))
                .child("Unable to render timeline preview")
                .into_any_element(),
        }
    }
}

/// Render images are prepared once on the backend worker, never during UI rendering.
pub struct TimelinePreviewFrame {
    pub frame: Arc<TimelineFrame>,
    images: HashMap<Ulid, Arc<RenderImage>>,
}

impl TimelinePreviewFrame {
    pub fn new(frame: Arc<TimelineFrame>) -> Self {
        let mut images = HashMap::new();
        for layer in &frame.layers {
            let (clip_id, pixels) = match layer {
                TimelineLayer::Video {
                    clip_id, pixels, ..
                }
                | TimelineLayer::Image {
                    clip_id, pixels, ..
                } => (*clip_id, pixels),
                TimelineLayer::Text { .. } => continue,
            };
            let mut bgra = pixels.as_ref().clone();
            // GPUI expects BGRA bytes even though the image crate calls this RGBA.
            for pixel in bgra.pixels_mut() {
                pixel.0.swap(0, 2);
            }
            images.insert(
                clip_id,
                Arc::new(RenderImage::new(smallvec![Frame::new(bgra)])),
            );
        }
        Self { frame, images }
    }

    pub fn render(&self, width: f32, height: f32) -> AnyElement {
        let root = div()
            .w(px(width.max(0.0)))
            .h(px(height.max(0.0)))
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .bg(rgb(0));
        let scale = (width / self.frame.width as f32).min(height / self.frame.height as f32);
        if !scale.is_finite() || scale <= 0.0 {
            return root.into_any_element();
        }
        let canvas_width = self.frame.width as f32 * scale;
        let canvas_height = self.frame.height as f32 * scale;
        let mut canvas = div()
            .relative()
            .flex_shrink_0()
            .overflow_hidden()
            .w(px(canvas_width))
            .h(px(canvas_height));
        for layer in &self.frame.layers {
            match layer {
                TimelineLayer::Video {
                    clip_id,
                    pixels,
                    properties,
                }
                | TimelineLayer::Image {
                    clip_id,
                    pixels,
                    properties,
                } => {
                    if properties.scale <= 0.0 {
                        continue;
                    }
                    let width = pixels.width() as f32 * properties.scale as f32 * scale;
                    let height = pixels.height() as f32 * properties.scale as f32 * scale;
                    let x = (canvas_width - width) / 2.0 + properties.position_x as f32 * scale;
                    let y = (canvas_height - height) / 2.0 + properties.position_y as f32 * scale;
                    canvas = canvas.child(
                        img(Arc::clone(&self.images[clip_id]))
                            .absolute()
                            .left(px(x))
                            .top(px(y))
                            .w(px(width))
                            .h(px(height)),
                    );
                }
                TimelineLayer::Text { properties, .. } => {
                    // A zero-size flex anchor centers the intrinsic text block on
                    // its normalized position, including multiline text.
                    canvas = canvas.child(
                        div()
                            .absolute()
                            .left(px(properties.position_x as f32 * canvas_width))
                            .top(px(properties.position_y as f32 * canvas_height))
                            .w(px(0.0))
                            .h(px(0.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .whitespace_nowrap()
                                    .font_family(properties.font.clone())
                                    .text_size(px(properties.font_size as f32 * scale))
                                    .line_height(px(properties.font_size as f32 * scale * 1.2))
                                    .text_align(TextAlign::Center)
                                    .text_color(rgba(properties.color.rotate_left(8)))
                                    .child(properties.text.clone()),
                            ),
                    );
                }
            }
        }
        root.child(canvas).into_any_element()
    }
}

#[cfg(test)]
#[path = "tests/preview_timeline.test.rs"]
mod tests;
