#[cfg(target_os = "macos")]
use core_video::pixel_buffer::CVPixelBuffer;
use gpui::{AnyElement, IntoElement, RenderImage, Styled, img};
use std::sync::Arc;

#[derive(Clone)]
pub enum DisplayedFrame {
    #[cfg(target_os = "macos")]
    Surface(CVPixelBuffer),
    GpuiImage(Arc<RenderImage>),
}

impl DisplayedFrame {
    pub fn dimensions(&self) -> (f32, f32) {
        match self {
            #[cfg(target_os = "macos")]
            Self::Surface(buffer) => (buffer.get_width() as f32, buffer.get_height() as f32),
            Self::GpuiImage(image) => {
                let size = image.size(0);
                (size.width.0 as f32, size.height.0 as f32)
            }
        }
    }

    pub fn element(&self) -> AnyElement {
        match self {
            #[cfg(target_os = "macos")]
            Self::Surface(buffer) => gpui::surface(buffer.clone()).size_full().into_any_element(),
            Self::GpuiImage(image) => img(Arc::clone(image)).size_full().into_any_element(),
        }
    }
}
