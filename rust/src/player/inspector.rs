use super::*;
use std::path::{Path, PathBuf};
use yuv::{YuvBiPlanarImage, YuvConversionMode, YuvRange, YuvStandardMatrix};

impl Player {
    fn save_current_frame(&mut self, cx: &mut Context<Self>) {
        self.error = None;

        let Some(video) = &self.video else {
            self.error = Some("Open a video before saving a frame.".to_string());
            cx.notify();
            return;
        };
        let Some(frame) = current_frame_rgba(video) else {
            self.error = Some("The current video frame is not ready yet.".to_string());
            cx.notify();
            return;
        };

        let directory = self
            .current_media_path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));
        let suggested_name = self.frame_filename();
        let selection = cx.prompt_for_new_path(&directory, Some(&suggested_name));

        cx.spawn(async move |player, cx| {
            let path = match selection.await {
                Ok(Ok(Some(mut path))) => {
                    if !path
                        .extension()
                        .and_then(|extension| extension.to_str())
                        .is_some_and(|extension| extension.eq_ignore_ascii_case("png"))
                    {
                        path.set_extension("png");
                    }
                    path
                }
                Ok(Ok(None)) => return,
                Ok(Err(error)) => {
                    player
                        .update(cx, |player, cx| {
                            player.error = Some(format!("Could not open save dialog: {error}"));
                            cx.notify();
                        })
                        .ok();
                    return;
                }
                Err(error) => {
                    player
                        .update(cx, |player, cx| {
                            player.error =
                                Some(format!("Save dialog closed unexpectedly: {error}"));
                            cx.notify();
                        })
                        .ok();
                    return;
                }
            };

            let save_result = cx
                .background_executor()
                .spawn(async move { save_frame_as_png(frame, &path) })
                .await;
            player
                .update(cx, |player, cx| {
                    player.error = save_result.err();
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    fn frame_filename(&self) -> String {
        let title = self
            .display_title()
            .chars()
            .map(|character| match character {
                '/' | ':' => '-',
                _ => character,
            })
            .collect::<String>();
        let position = self.video.as_ref().map_or(Duration::ZERO, Video::position);
        let total_seconds = position.as_secs();
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;

        format!("{title}-frame-{hours:02}-{minutes:02}-{seconds:02}.png")
    }

    pub(super) fn inspector_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        div()
            .id("inspector-panel")
            .w(px(INSPECTOR_WIDTH))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(rgb(0x303036))
            .bg(rgb(0x111114))
            .child(
                div()
                    .h(px(52.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("Inspector"),
                    )
                    .child(
                        div()
                            .id("close-inspector")
                            .size_8()
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor(CursorStyle::PointingHand)
                            .rounded_md()
                            .text_color(rgb(MUTED))
                            .hover(|style| style.bg(rgb(SURFACE_HOVER)).text_color(rgb(TEXT)))
                            .child("×")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.inspector_open = false;
                                cx.notify();
                            })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(0x777780))
                            .child("RENDERING"),
                    )
                    .child(
                        div()
                            .h(px(64.0))
                            .flex()
                            .items_center()
                            .justify_between()
                            .rounded_lg()
                            .border_1()
                            .border_color(rgb(BORDER))
                            .bg(rgb(0x17171a))
                            .px_4()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .child(div().size_2().rounded_full().bg(rgb(0x63d68b)))
                                    .child("Render FPS"),
                            )
                            .child(
                                div()
                                    .font_family("monospace")
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(rgb(ACCENT))
                                    .child(format!("{:.1}", self.render_fps)),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x606068))
                            .child("GPUI render passes per second"),
                    )
                    .child(
                        div()
                            .mt_5()
                            .text_xs()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(rgb(0x777780))
                            .child("FRAME"),
                    )
                    .child(
                        div()
                            .id("save-current-frame")
                            .h(px(44.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .rounded_lg()
                            .border_1()
                            .border_color(rgb(BORDER))
                            .bg(rgb(0x17171a))
                            .cursor(CursorStyle::PointingHand)
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .hover(|style| style.bg(rgb(SURFACE_HOVER)).border_color(rgb(0x3b3b42)))
                            .child("Save current frame as PNG")
                            .on_click(cx.listener(|player, _, _, cx| {
                                player.save_current_frame(cx);
                            })),
                    ),
            )
            .into_any_element()
    }
}

fn current_frame_rgba(video: &Video) -> Option<(Vec<u8>, u32, u32)> {
    let (nv12, width, height) = video.current_frame_data()?;
    let y_size = width as usize * height as usize;
    let uv_size = width as usize * (height as usize).div_ceil(2);
    let image = YuvBiPlanarImage {
        y_plane: nv12.get(..y_size)?,
        y_stride: width,
        uv_plane: nv12.get(y_size..y_size.checked_add(uv_size)?)?,
        uv_stride: width,
        width,
        height,
    };
    let mut rgba = vec![0; y_size.checked_mul(4)?];
    yuv::yuv_nv12_to_rgba(
        &image,
        &mut rgba,
        width.checked_mul(4)?,
        YuvRange::Full,
        YuvStandardMatrix::Bt709,
        YuvConversionMode::Balanced,
    )
    .ok()?;
    Some((rgba, width, height))
}

fn save_frame_as_png(frame: (Vec<u8>, u32, u32), path: &Path) -> Result<(), String> {
    let (rgba, width, height) = frame;

    image::save_buffer_with_format(
        path,
        &rgba,
        width,
        height,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .map_err(|error| format!("Could not save {}: {error}", path.display()))
}
