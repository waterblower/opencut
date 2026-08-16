use super::Video;
#[cfg(target_os = "macos")]
use core_foundation::{
    base::TCFType,
    boolean::CFBoolean,
    dictionary::{CFDictionary, CFMutableDictionary},
    string::CFString,
};
#[cfg(target_os = "macos")]
use core_video::{
    pixel_buffer::{CVPixelBuffer, kCVPixelFormatType_420YpCbCr8BiPlanarFullRange},
    r#return::kCVReturnSuccess,
};
use gpui::{
    Element, ElementId, GlobalElementId, InspectorElementId, IntoElement, LayoutId, Window,
};
use gst_video::VideoFrameExt as _;
use gstreamer as gst;
use gstreamer_video as gst_video;
use std::sync::Arc;
use yuv::{YuvBiPlanarImage, YuvConversionMode, YuvRange, YuvStandardMatrix, yuv_nv12_to_bgra};

//////////////////
// VideoElement //
//////////////////
pub(crate) struct VideoElement {
    pub frame: Option<gst::Sample>,
    pub width: gpui::Pixels,
    pub height: gpui::Pixels,
    pub id: Option<ElementId>,
}

impl VideoElement {
    pub(crate) fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = Some(id.into());
        self
    }

    pub(crate) fn size(mut self, width: gpui::Pixels, height: gpui::Pixels) -> Self {
        self.width = width;
        self.height = height;
        self
    }

    fn fitted_bounds(
        bounds: gpui::Bounds<gpui::Pixels>,
        frame_width: u32,
        frame_height: u32,
    ) -> gpui::Bounds<gpui::Pixels> {
        let container_width = f32::from(bounds.size.width);
        let container_height = f32::from(bounds.size.height);
        let scale = (container_width / frame_width.max(1) as f32)
            .min(container_height / frame_height.max(1) as f32);
        let width = frame_width as f32 * scale;
        let height = frame_height as f32 * scale;
        gpui::Bounds::new(
            gpui::point(
                bounds.origin.x + gpui::px((container_width - width) * 0.5),
                bounds.origin.y + gpui::px((container_height - height) * 0.5),
            ),
            gpui::size(gpui::px(width), gpui::px(height)),
        )
    }

    fn current_frame_data(&self) -> Option<(Vec<u8>, u32, u32)> {
        pack_nv12(self.frame.as_ref()?)
    }

    #[cfg(target_os = "macos")]
    fn paint_macos(
        &self,
        window: &mut Window,
        bounds: gpui::Bounds<gpui::Pixels>,
        nv12: &[u8],
        width: u32,
        height: u32,
    ) -> bool {
        let width = width as usize;
        let height = height as usize;
        let y_size = width * height;
        if width == 0 || height == 0 || nv12.len() < y_size + width * height.div_ceil(2) {
            return false;
        }
        let mut attributes = CFMutableDictionary::<CFString, core_foundation::base::CFType>::new();
        attributes.add(
            &core_video::pixel_buffer::CVPixelBufferKeys::MetalCompatibility.into(),
            &CFBoolean::true_value().as_CFType(),
        );
        let iosurface =
            CFDictionary::<CFString, core_foundation::base::CFType>::from_CFType_pairs(&[]);
        attributes.add(
            &core_video::pixel_buffer::CVPixelBufferKeys::IOSurfaceProperties.into(),
            &iosurface.as_CFType(),
        );
        let Ok(pixel_buffer) = CVPixelBuffer::new(
            kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
            width,
            height,
            Some(&attributes.to_immutable()),
        ) else {
            return false;
        };
        if !pixel_buffer.is_planar()
            || pixel_buffer.get_plane_count() != 2
            || pixel_buffer.get_width_of_plane(0) != width
            || pixel_buffer.get_height_of_plane(0) != height
            || pixel_buffer.get_bytes_per_row_of_plane(0) < width
            || pixel_buffer.get_bytes_per_row_of_plane(1) < width
            || pixel_buffer.lock_base_address(0) != kCVReturnSuccess
        {
            return false;
        }
        unsafe {
            let y_destination = pixel_buffer.get_base_address_of_plane(0) as *mut u8;
            let uv_destination = pixel_buffer.get_base_address_of_plane(1) as *mut u8;
            let y_stride = pixel_buffer.get_bytes_per_row_of_plane(0);
            let uv_stride = pixel_buffer.get_bytes_per_row_of_plane(1);
            for row in 0..height {
                std::ptr::copy_nonoverlapping(
                    nv12.as_ptr().add(row * width),
                    y_destination.add(row * y_stride),
                    width,
                );
            }
            for row in 0..height.div_ceil(2) {
                std::ptr::copy_nonoverlapping(
                    nv12.as_ptr().add(y_size + row * width),
                    uv_destination.add(row * uv_stride),
                    width,
                );
            }
        }
        let _ = pixel_buffer.unlock_base_address(0);
        window.paint_surface(
            Self::fitted_bounds(bounds, width as u32, height as u32),
            pixel_buffer,
        );
        true
    }

    fn paint_fallback(
        &self,
        window: &mut Window,
        cx: &mut gpui::App,
        bounds: gpui::Bounds<gpui::Pixels>,
        nv12: &[u8],
        width: u32,
        height: u32,
    ) {
        use image::{ImageBuffer, Rgba};
        use smallvec::SmallVec;

        let y_size = width as usize * height as usize;
        let uv_size = width as usize * (height as usize).div_ceil(2);
        let Some(y_plane) = nv12.get(..y_size) else {
            return;
        };
        let Some(uv_plane) = nv12.get(y_size..y_size + uv_size) else {
            return;
        };
        let image = YuvBiPlanarImage {
            y_plane,
            y_stride: width,
            uv_plane,
            uv_stride: width,
            width,
            height,
        };
        let mut bgra = vec![0; y_size * 4];
        if yuv_nv12_to_bgra(
            &image,
            &mut bgra,
            width * 4,
            YuvRange::Full,
            YuvStandardMatrix::Bt709,
            YuvConversionMode::Balanced,
        )
        .is_err()
        {
            return;
        }
        let Some(image) = ImageBuffer::<Rgba<u8>, _>::from_raw(width, height, bgra) else {
            return;
        };
        let previous: gpui::Entity<Option<Arc<gpui::RenderImage>>> =
            window.use_state(cx, |_, _| None);
        let rendered = Arc::new(gpui::RenderImage::new(SmallVec::from_elem(
            image::Frame::new(image),
            1,
        )));
        let old = previous.update(cx, |current, _| current.replace(rendered.clone()));
        let _ = window.paint_image(
            Self::fitted_bounds(bounds, width, height),
            gpui::Corners::default(),
            rendered,
            0,
            false,
        );
        if let Some(old) = old {
            cx.drop_image(old, Some(window));
        }
    }
}

impl Element for VideoElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        self.id.clone()
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut gpui::App,
    ) -> (LayoutId, ()) {
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
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: gpui::Bounds<gpui::Pixels>,
        _: &mut (),
        window: &mut Window,
        _: &mut gpui::App,
    ) {
        window.request_animation_frame();
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: gpui::Bounds<gpui::Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut gpui::App,
    ) {
        let Some((nv12, width, height)) = self.current_frame_data() else {
            return;
        };
        #[cfg(target_os = "macos")]
        if self.paint_macos(window, bounds, &nv12, width, height) {
            return;
        }
        self.paint_fallback(window, cx, bounds, &nv12, width, height);
    }
}

impl IntoElement for VideoElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

pub(crate) fn video(video: &Video) -> VideoElement {
    let Some((width, height)) = video.frame_size() else {
        panic!("impossible")
    };
    let frame = video.get_current_frame();
    VideoElement {
        frame: frame,
        width: gpui::px(width as f32),
        height: gpui::px(height as f32),
        id: None,
    }
}

fn pack_nv12(sample: &gst::Sample) -> Option<(Vec<u8>, u32, u32)> {
    let info = gst_video::VideoInfo::from_caps(sample.caps()?).ok()?;
    if info.format() != gst_video::VideoFormat::Nv12 {
        return None;
    }
    let buffer = sample.buffer()?;
    let frame = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info).ok()?;
    let width = frame.width() as usize;
    let height = frame.height() as usize;
    let uv_rows = height.div_ceil(2);
    let mut packed = Vec::with_capacity(width.checked_mul(height.checked_add(uv_rows)?)?);
    for (plane, rows) in [(0_u32, height), (1_u32, uv_rows)] {
        let stride = usize::try_from(*frame.info().stride().get(plane as usize)?).ok()?;
        let source = frame.plane_data(plane).ok()?;
        if stride < width {
            return None;
        }
        for row in 0..rows {
            let start = row.checked_mul(stride)?;
            packed.extend_from_slice(source.get(start..start.checked_add(width)?)?);
        }
    }
    Some((packed, frame.width(), frame.height()))
}
