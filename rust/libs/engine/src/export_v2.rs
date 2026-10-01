use std::{path::Path, sync::Arc, time::Instant};

use gpui::{
    AnyElement, AppContext as _, Context, HeadlessAppContext, IntoElement, Render, Window, div, px,
    size,
};
use timeline::TimelineSerialization;

use crate::{
    export::ExportOption, export_encoder::ExportEncoder, timeline_decoder::TimelineDecoder,
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
        let started = Instant::now();
        // get frame compositions from the decoder
        let frame = decoder.frame_at(&editing_state, i.into())?;
        let decoded = Instant::now();

        // convert the composition to GPUI element
        let element = frame.render_frame(logical_width, logical_height);
        let composed = Instant::now();

        // convert the GPUI element to image buffer, aka raw frame data
        let image = cx.update_window(window.into(), |root, window, cx| {
            let view = root.downcast::<ExportCanvas>().unwrap();
            view.update(cx, |view, _| {
                view.element = Some(element);
            });

            window.refresh();
            let arena = window.draw(cx);
            let capture_started = Instant::now();
            let image = window.render_to_image();
            let capture_elapsed = capture_started.elapsed();
            arena.clear(cx);
            // eprintln!("Export frame {i}: window.render_to_image={capture_elapsed:?}");
            image
        })??;
        let rendered = Instant::now();

        // send to encoder
        encoder.video(&image, i)?;
        let encoded = Instant::now();
        eprintln!(
            "Export frame {i}: decode={:?}, compose={:?}, render_to_image={:?}, encode={:?}, total={:?}",
            decoded.duration_since(started),
            composed.duration_since(decoded),
            rendered.duration_since(composed),
            encoded.duration_since(rendered),
            encoded.duration_since(started),
        );
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
