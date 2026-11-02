//! A decoded video picture ready for GPUI: a Metal-converted CoreVideo buffer on macOS,
//! a software-converted BGRA image elsewhere.

use gpui::{AnyElement, IntoElement, ObjectFit, Styled};
#[cfg(not(target_os = "macos"))]
use {
    crate::cpu::release_retired_surfaces,
    gpui::{ParentElement, StyledImage, canvas, div, img},
};

#[cfg(target_os = "macos")]
pub use crate::gpu::GpuResources as SurfaceConverter;
#[cfg(target_os = "macos")]
pub type VideoSurface = core_video::pixel_buffer::CVPixelBuffer;

#[cfg(not(target_os = "macos"))]
pub use crate::cpu::CpuResources as SurfaceConverter;
#[cfg(not(target_os = "macos"))]
pub type VideoSurface = crate::cpu::CpuSurface;

/// Picture size in pixels.
pub fn video_surface_size(surface: &VideoSurface) -> (f32, f32) {
    #[cfg(target_os = "macos")]
    return (surface.get_width() as f32, surface.get_height() as f32);
    #[cfg(not(target_os = "macos"))]
    {
        let size = surface.image().size(0);
        (size.width.0 as f32, size.height.0 as f32)
    }
}

/// Fills its parent with the picture.
pub fn video_surface_view(surface: &VideoSurface, object_fit: ObjectFit) -> AnyElement {
    #[cfg(target_os = "macos")]
    return gpui::surface(surface.clone())
        .object_fit(object_fit)
        .size_full()
        .into_any_element();
    #[cfg(not(target_os = "macos"))]
    div()
        .relative()
        .size_full()
        .child(
            img(surface.image_to_paint())
                .object_fit(object_fit)
                .size_full(),
        )
        // 先前帧已不再显示；在本窗口绘制前释放它们的图集块。
        .child(
            canvas(
                |_, window, cx| release_retired_surfaces(window, cx),
                |_, _, _, _| {},
            )
            .absolute(),
        )
        .into_any_element()
}
