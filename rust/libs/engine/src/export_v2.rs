use std::{path::Path, sync::Arc};

use gpui::{
    AnyElement, AppContext as _, Context, HeadlessAppContext, IntoElement, ParentElement, Render,
    Styled, TextAlign, Window, div, img, px, rgb, rgba, size,
};
use timeline::TimelineSerialization;

use crate::{
    export::ExportOption,
    export_encoder::ExportEncoder,
    timeline_decoder::{PreparedFrame, PreparedLayer, TimelineDecoder},
};
use anyhow::{Context as _, Result};

pub fn export_v2(
    timeline_serialization: &TimelineSerialization,
    output_path: &Path,
    option: &ExportOption,
) -> Result<()> {
    // Convenient Variables
    let editing_state = timeline_serialization.to_editing_state();
    let timeline_settings = editing_state.settings;
    let frame_count = timeline_serialization.frame_count();

    // Init the decoder and encoder
    let mut decoder = TimelineDecoder::new(&option.project_root);
    let mut encoder = ExportEncoder::open(
        output_path,
        timeline_serialization.editing_state.settings,
        option.video_bitrate,
    )?;

    // ----------------------------------|
    // creating the headless GPUI window |
    // ----------------------------------|
    let platform = gpui_platform::current_platform(true);
    let mut cx = HeadlessAppContext::with_platform(
        platform.text_system(),
        Arc::new(()),
        gpui_platform::current_headless_renderer,
    );
    let window = cx
        .open_window(
            size(
                px(timeline_settings.width as f32),
                px(timeline_settings.height as f32),
            ),
            |_, cx| cx.new(|_| ExportCanvas { element: None }),
        )
        .context("Creating export canvas")?;

    let (logical_width, logical_height) = window.update(&mut cx, |_, window, cx| {
        let scale = window.scale_factor();
        let width = timeline_settings.width as f32 / scale;
        let height = timeline_settings.height as f32 / scale;

        window.resize(size(px(width), px(height)));
        window.bounds_changed(cx);

        (width, height)
    })?;

    // --------------- //
    // The Render Loop //
    // --------------- //
    for i in 0..frame_count {
        // render each frame
        let frame = decoder.frame_at(&editing_state, i.into())?;
        let element = render_frame(&frame, logical_width, logical_height);

        let image = cx.update_window(window.into(), |root, window, cx| {
            let view = root.downcast::<ExportCanvas>().unwrap();
            view.update(cx, |view, _| {
                view.element = Some(element);
            });

            window.refresh();
            let arena = window.draw(cx);
            let image = window.render_to_image();
            arena.clear(cx);
            image
        })??;

        encoder.video(&image, i)?;
    }
    encoder.finish(0)?;
    return Ok(());
}

struct ExportCanvas {
    // question: why not element: AnyElement?
    element: Option<AnyElement>,
}

impl Render for ExportCanvas {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.element
            .take()
            .unwrap_or_else(|| div().into_any_element())
    }
}

/// Same composition as the editor preview: fit the canvas, then apply each layer's transform.
fn render_frame(frame: &PreparedFrame, width: f32, height: f32) -> AnyElement {
    let root = div()
        .w(px(width.max(0.0)))
        .h(px(height.max(0.0)))
        .flex()
        .items_center()
        .justify_center()
        .overflow_hidden()
        .bg(rgb(0));
    let scale = (width / frame.width as f32).min(height / frame.height as f32);
    if !scale.is_finite() || scale <= 0.0 {
        return root.into_any_element();
    }
    let canvas_width = frame.width as f32 * scale;
    let canvas_height = frame.height as f32 * scale;
    let mut canvas = div()
        .relative()
        .flex_shrink_0()
        .overflow_hidden()
        .w(px(canvas_width))
        .h(px(canvas_height));
    for layer in &frame.layers {
        match layer {
            PreparedLayer::Picture {
                image, properties, ..
            } => {
                if properties.scale <= 0.0 {
                    continue;
                }
                let size = image.size(0);
                let source_width = size.width.0 as f32;
                let source_height = size.height.0 as f32;
                let fit = (canvas_width / source_width).min(canvas_height / source_height);
                let width = source_width * fit * properties.scale as f32;
                let height = source_height * fit * properties.scale as f32;
                let x = (canvas_width - width) / 2.0 + properties.position_x as f32 * scale;
                let y = (canvas_height - height) / 2.0 + properties.position_y as f32 * scale;
                canvas = canvas.child(
                    img(Arc::clone(image))
                        .absolute()
                        .left(px(x))
                        .top(px(y))
                        .w(px(width))
                        .h(px(height)),
                );
            }
            PreparedLayer::Text { properties, .. } => {
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
