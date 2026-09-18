use anyhow::{Context as _, Result, bail};
use gpui::{
    AnyWindowHandle, AppContext as _, Context, HeadlessAppContext, IntoElement, Render, Window,
    div, prelude::*, px, rgb, size,
};
use opencut_player::{
    cli::engine::encode::{Encoder, VideoEncoding},
    timeline::FrameRate,
};
use serde_json::{Value, json};
use std::{fs, path::Path, sync::Arc};
use ulid::Ulid;

/// Draw each frame through GPUI's production renderer, then encode its pixels.
pub fn render(output: &Path) -> Result<Value> {
    validate_render_request(output)?;
    let platform = gpui_platform::current_platform(true);
    let mut cx = HeadlessAppContext::with_platform(
        platform.text_system(),
        Arc::new(()),
        gpui_platform::current_headless_renderer,
    );
    let window = cx
        .open_window(size(px(640.0), px(360.0)), |_, cx| cx.new(|_| HelloGpui))
        .context("could not create GPUI render window")?;
    let temporary = output.with_file_name(format!(".opencut-{}.mp4", Ulid::generate()));
    let result: Result<Value> = (|| {
        let image =
            render_frame(&mut cx, window.into()).context("could not render GPUI frame 0")?;
        let dimensions = image.dimensions();
        let mut encoder = Encoder::open(
            &temporary,
            dimensions,
            FrameRate::new(30, 1),
            48_000,
            &VideoEncoding {
                codec: "h264".into(),
                preset: "draft".into(),
                bitrate: 2_000_000,
            },
            None,
        )?;
        encoder.encode_new_frame(&image)?;
        for frame in 1..150 {
            let image = render_frame(&mut cx, window.into())
                .context(format!("could not render GPUI frame {frame}"))?;
            encoder.encode_new_frame(&image)?;
        }
        encoder.finish()?;
        // Publishing without replacement also protects against another writer
        // creating the destination while frames were being rendered.
        fs::hard_link(&temporary, output)
            .context(format!("could not publish render to {}", output.display()))?;
        Ok(
            json!({"output": output, "frames": 150, "fps": 30, "duration_seconds": 5,
            "width": dimensions.0, "height": dimensions.1}),
        )
    })();
    let cleanup = fs::remove_file(&temporary);
    let value = result?;
    cleanup.context("could not remove temporary render")?;
    Ok(value)
}

struct HelloGpui;

impl Render for HelloGpui {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0x000000))
            .flex()
            .items_center()
            .justify_center()
            .font_family("Helvetica")
            .text_size(px(32.0))
            .text_color(rgb(0xffffff))
            .child("hello gpui")
    }
}

fn render_frame(cx: &mut HeadlessAppContext, window: AnyWindowHandle) -> Result<image::RgbaImage> {
    cx.update_window(window, |_, window, cx| {
        window.refresh();
        let arena = window.draw(cx);
        let image = window.render_to_image();
        arena.clear(cx);
        image
    })
    .context("could not update GPUI frame")?
    .context("could not capture GPUI frame")
}

fn validate_render_request(output: &Path) -> Result<()> {
    if !cfg!(target_os = "macos") {
        bail!("GPUI demo rendering currently requires macOS");
    }
    if output.extension().and_then(|ext| ext.to_str()) != Some("mp4") {
        bail!("render output must have an .mp4 extension");
    }
    if output.try_exists().context("could not check output")? {
        bail!("output already exists: {}", output.display());
    }
    Ok(())
}
