use super::*;
use crate::video_backend::current_frame_rgba;
use gpui::{
    App, Element, ElementId, GlobalElementId, InspectorElementId, LayoutId, RenderImage, Window,
};
use image::{Frame, ImageBuffer, Rgba, RgbaImage};
use std::{path::Path, sync::Arc};

#[derive(Clone)]
enum MediaSource {
    Image(PathBuf),
    Video(Video),
}

pub(super) struct TransformedMedia {
    source: MediaSource,
    source_path: PathBuf,
    clip_id: u64,
    properties: VideoClipProperties,
    project_width: u32,
    project_height: u32,
    width: gpui::Pixels,
    height: gpui::Pixels,
    element_id: ElementId,
}

#[derive(Default)]
struct TransformedMediaState {
    clip_id: u64,
    source_path: PathBuf,
    source_frame: Option<RgbaImage>,
    rendered_frame: Option<Arc<RenderImage>>,
    properties: VideoClipProperties,
    canvas_width: u32,
    canvas_height: u32,
    failed: bool,
}

pub(super) fn transformed_image(
    clip_id: u64,
    path: PathBuf,
    properties: VideoClipProperties,
    project_width: u32,
    project_height: u32,
    width: f32,
    height: f32,
) -> TransformedMedia {
    TransformedMedia {
        source: MediaSource::Image(path.clone()),
        source_path: path,
        clip_id,
        properties,
        project_width,
        project_height,
        width: px(width),
        height: px(height),
        element_id: ("transformed-timeline-image", clip_id).into(),
    }
}

pub(super) fn transformed_video(
    clip_id: u64,
    path: PathBuf,
    video: Video,
    properties: VideoClipProperties,
    project_width: u32,
    project_height: u32,
    width: f32,
    height: f32,
) -> TransformedMedia {
    TransformedMedia {
        source: MediaSource::Video(video),
        source_path: path,
        clip_id,
        properties,
        project_width,
        project_height,
        width: px(width),
        height: px(height),
        element_id: ("transformed-timeline-video", clip_id).into(),
    }
}

impl Element for TransformedMedia {
    type RequestLayoutState = ();
    type PrepaintState = bool;

    fn id(&self) -> Option<ElementId> {
        Some(self.element_id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let style = gpui::Style {
            size: gpui::Size {
                width: gpui::Length::Definite(gpui::DefiniteLength::Absolute(
                    gpui::AbsoluteLength::Pixels(self.width),
                )),
                height: gpui::Length::Definite(gpui::DefiniteLength::Absolute(
                    gpui::AbsoluteLength::Pixels(self.height),
                )),
            },
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: gpui::Bounds<gpui::Pixels>,
        _request_layout_state: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        match &self.source {
            MediaSource::Video(video) => {
                let frame_ready = video.take_frame_ready();
                if frame_ready || (!video.paused() && !video.eos()) {
                    window.request_animation_frame();
                }
                frame_ready
            }
            MediaSource::Image(_) => false,
        }
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        _request_layout_state: &mut Self::RequestLayoutState,
        frame_ready: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let canvas_width = f32::from(bounds.size.width).round().max(1.0) as u32;
        let canvas_height = f32::from(bounds.size.height).round().max(1.0) as u32;
        let state: Entity<TransformedMediaState> =
            window.use_state(cx, |_, _| TransformedMediaState::default());

        let source_changed = state.read(cx).clip_id != self.clip_id
            || state.read(cx).source_path != self.source_path;
        let transform_changed = state.read(cx).properties != self.properties
            || state.read(cx).canvas_width != canvas_width
            || state.read(cx).canvas_height != canvas_height;
        let needs_frame = state.read(cx).rendered_frame.is_none()
            || source_changed
            || transform_changed
            || *frame_ready;

        let previous = needs_frame.then(|| {
            state.update(cx, |state, _| {
                if source_changed {
                    state.clip_id = self.clip_id;
                    state.source_path = self.source_path.clone();
                    state.source_frame = None;
                    state.failed = false;
                }

                let should_read_source = state.source_frame.is_none()
                    || matches!(&self.source, MediaSource::Video(_)) && *frame_ready;
                if should_read_source {
                    state.source_frame = match &self.source {
                        MediaSource::Image(path) => load_image(path).ok(),
                        MediaSource::Video(video) => {
                            current_frame_rgba(video).and_then(|(pixels, width, height)| {
                                ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, pixels)
                            })
                        }
                    };
                    if state.source_frame.is_none() && !state.failed {
                        log::warn!(
                            "Could not read preview frame from {}",
                            self.source_path.display()
                        );
                        state.failed = true;
                    }
                }

                let Some(source) = state.source_frame.as_ref() else {
                    return None;
                };
                let mut output = transform_frame(
                    source,
                    canvas_width,
                    canvas_height,
                    self.project_width,
                    self.project_height,
                    self.properties,
                );
                rgba_to_bgra(&mut output);
                let image = Arc::new(RenderImage::new([Frame::new(output)]));
                state.properties = self.properties;
                state.canvas_width = canvas_width;
                state.canvas_height = canvas_height;
                state.failed = false;
                state.rendered_frame.replace(image)
            })
        });

        if let Some(Some(previous)) = previous {
            cx.drop_image(previous, Some(window));
        }
        if let Some(image) = state.read(cx).rendered_frame.clone() {
            let _ = window.paint_image(bounds, gpui::Corners::default(), image, 0, false);
        }
    }
}

impl IntoElement for TransformedMedia {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

fn load_image(path: &Path) -> Result<RgbaImage, image::ImageError> {
    image::open(path).map(|image| image.into_rgba8())
}

fn transform_frame(
    source: &RgbaImage,
    canvas_width: u32,
    canvas_height: u32,
    project_width: u32,
    project_height: u32,
    properties: VideoClipProperties,
) -> RgbaImage {
    let mut output = RgbaImage::new(canvas_width, canvas_height);
    if source.width() == 0 || source.height() == 0 {
        return output;
    }

    let project_width = project_width.max(1) as f64;
    let project_height = project_height.max(1) as f64;
    let canvas_width_f = canvas_width as f64;
    let canvas_height_f = canvas_height as f64;
    let project_scale = (canvas_width_f / project_width).min(canvas_height_f / project_height);
    let fitted_scale = (project_width * project_scale / source.width() as f64)
        .min(project_height * project_scale / source.height() as f64);
    let clip_scale = finite_or(properties.scale, 1.0).max(0.0);
    let scale = fitted_scale * clip_scale;
    if scale <= f64::EPSILON {
        return output;
    }

    let center_x = canvas_width_f * 0.5 + finite_or(properties.position_x, 0.0) * project_scale;
    let center_y = canvas_height_f * 0.5 + finite_or(properties.position_y, 0.0) * project_scale;
    let angle = finite_or(properties.rotation_degrees, 0.0).to_radians();
    let cosine = angle.cos();
    let sine = angle.sin();
    let half_width = source.width() as f64 * scale * 0.5;
    let half_height = source.height() as f64 * scale * 0.5;
    let extent_x = cosine.abs() * half_width + sine.abs() * half_height;
    let extent_y = sine.abs() * half_width + cosine.abs() * half_height;
    let min_x = (center_x - extent_x).floor().max(0.0) as u32;
    let max_x = (center_x + extent_x).ceil().min(canvas_width_f) as u32;
    let min_y = (center_y - extent_y).floor().max(0.0) as u32;
    let max_y = (center_y + extent_y).ceil().min(canvas_height_f) as u32;
    let opacity = finite_or(properties.opacity, 1.0).clamp(0.0, 1.0);

    for y in min_y..max_y {
        for x in min_x..max_x {
            let delta_x = x as f64 + 0.5 - center_x;
            let delta_y = y as f64 + 0.5 - center_y;
            let source_x =
                (cosine * delta_x + sine * delta_y) / scale + source.width() as f64 * 0.5 - 0.5;
            let source_y =
                (-sine * delta_x + cosine * delta_y) / scale + source.height() as f64 * 0.5 - 0.5;
            if let Some(pixel) = bilinear_sample(source, source_x, source_y) {
                let mut pixel = pixel;
                pixel[3] = (pixel[3] as f64 * opacity).round() as u8;
                output.put_pixel(x, y, pixel);
            }
        }
    }
    output
}

fn bilinear_sample(image: &RgbaImage, x: f64, y: f64) -> Option<Rgba<u8>> {
    let max_x = image.width() as f64 - 0.5;
    let max_y = image.height() as f64 - 0.5;
    if x < -0.5 || y < -0.5 || x > max_x || y > max_y {
        return None;
    }
    let x = x.clamp(0.0, (image.width() - 1) as f64);
    let y = y.clamp(0.0, (image.height() - 1) as f64);
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(image.width() - 1);
    let y1 = (y0 + 1).min(image.height() - 1);
    let tx = x - x0 as f64;
    let ty = y - y0 as f64;
    let pixels = [
        image.get_pixel(x0, y0),
        image.get_pixel(x1, y0),
        image.get_pixel(x0, y1),
        image.get_pixel(x1, y1),
    ];
    let mut output = [0; 4];
    for channel in 0..4 {
        let top = pixels[0][channel] as f64 * (1.0 - tx) + pixels[1][channel] as f64 * tx;
        let bottom = pixels[2][channel] as f64 * (1.0 - tx) + pixels[3][channel] as f64 * tx;
        output[channel] = (top * (1.0 - ty) + bottom * ty).round() as u8;
    }
    Some(Rgba(output))
}

fn rgba_to_bgra(image: &mut RgbaImage) {
    for pixel in image.pixels_mut() {
        pixel.0.swap(0, 2);
    }
}

fn finite_or(value: f64, fallback: f64) -> f64 {
    value.is_finite().then_some(value).unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opacity_is_applied_to_transformed_pixels() {
        let source = RgbaImage::from_pixel(1, 1, Rgba([10, 20, 30, 200]));
        let output = transform_frame(
            &source,
            1,
            1,
            1,
            1,
            VideoClipProperties {
                opacity: 0.5,
                ..VideoClipProperties::default()
            },
        );
        assert_eq!(output.get_pixel(0, 0), &Rgba([10, 20, 30, 100]));
    }

    #[test]
    fn position_uses_project_pixels() {
        let source = RgbaImage::from_pixel(1, 1, Rgba([255, 0, 0, 255]));
        let output = transform_frame(
            &source,
            4,
            2,
            4,
            2,
            VideoClipProperties {
                position_x: 1.0,
                ..VideoClipProperties::default()
            },
        );
        assert_eq!(output.get_pixel(3, 1), &Rgba([255, 0, 0, 255]));
        assert_eq!(output.get_pixel(1, 1), &Rgba([0, 0, 0, 0]));
    }
}
