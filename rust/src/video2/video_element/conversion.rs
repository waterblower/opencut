use std::{ptr, sync::Arc};

use anyhow::{Context as _, Result, bail};
use ffmpeg_next as ffmpeg;
use gpui::RenderImage;

#[cfg(target_os = "macos")]
use core_video::pixel_buffer::CVPixelBuffer;

pub enum Prepared {
    #[cfg(target_os = "macos")]
    Surface(CVPixelBuffer),
    Image(Arc<RenderImage>),
}

pub struct Converter(*mut ffmpeg::ffi::SwsContext);

impl Default for Converter {
    fn default() -> Self {
        Self(ptr::null_mut())
    }
}

impl Drop for Converter {
    fn drop(&mut self) {
        // SAFETY: this converter exclusively owns the context; null is accepted.
        unsafe {
            ffmpeg::ffi::sws_free_context(&mut self.0);
        }
    }
}

impl Converter {
    pub fn prepare(&mut self, source: &ffmpeg::frame::Video) -> Result<Prepared> {
        #[cfg(target_os = "macos")]
        {
            let surface = (|| {
                let converted = self.convert(source, ffmpeg::format::Pixel::NV12)?;
                video_surface(&converted)
            })();
            match surface {
                Ok(surface) => Ok(Prepared::Surface(surface)),
                Err(surface_error) => {
                    let image = self.bgra(source).context(format!(
                        "Falling back after surface preparation failed: {surface_error:#} at {}:{}",
                        file!(),
                        line!()
                    ))?;
                    Ok(Prepared::Image(image))
                }
            }
        }
        #[cfg(not(target_os = "macos"))]
        Ok(Prepared::Image(self.bgra(source)?))
    }

    pub fn bgra(&mut self, source: &ffmpeg::frame::Video) -> Result<Arc<RenderImage>> {
        let converted = self.convert(source, ffmpeg::format::Pixel::BGRA)?;
        let row_bytes = (converted.width() as usize)
            .checked_mul(4)
            .context(format!(
                "Video row size overflow at {}:{}",
                file!(),
                line!()
            ))?;
        let capacity = row_bytes
            .checked_mul(converted.height() as usize)
            .context(format!(
                "Video image size overflow at {}:{}",
                file!(),
                line!()
            ))?;
        let mut pixels = Vec::with_capacity(capacity);
        for row in 0..converted.height() as usize {
            let start = row * converted.stride(0);
            let bytes = converted
                .data(0)
                .get(start..start + row_bytes)
                .context(format!(
                    "Truncated BGRA video row at {}:{}",
                    file!(),
                    line!()
                ))?;
            pixels.extend_from_slice(bytes);
        }
        // RenderImage explicitly expects BGRA bytes despite image's Rgba type.
        let image =
            image::RgbaImage::from_raw(converted.width(), converted.height(), pixels).context(
                format!("Invalid video image dimensions at {}:{}", file!(), line!()),
            )?;
        Ok(Arc::new(RenderImage::new(smallvec::smallvec![
            image::Frame::new(image)
        ])))
    }

    pub fn convert(
        &mut self,
        source: &ffmpeg::frame::Video,
        format: ffmpeg::format::Pixel,
    ) -> Result<ffmpeg::frame::Video> {
        if source.width() == 0 || source.height() == 0 || source.planes() == 0 {
            bail!("Video frame has no pixels at {}:{}", file!(), line!());
        }
        if self.0.is_null() {
            // SAFETY: allocation has no preconditions; ownership stays in self.
            self.0 = unsafe { ffmpeg::ffi::sws_alloc_context() };
            if self.0.is_null() {
                bail!("Allocating video scaler at {}:{}", file!(), line!());
            }
        }
        let mut converted = ffmpeg::frame::Video::empty();
        converted.set_width(source.width());
        converted.set_height(source.height());
        converted.set_format(format);
        converted.set_color_primaries(source.color_primaries());
        converted.set_color_transfer_characteristic(source.color_transfer_characteristic());
        converted.set_color_range(ffmpeg::color::Range::JPEG);
        if format == ffmpeg::format::Pixel::NV12 {
            // The vendored GPUI surface shader uses full-range BT.601.
            converted.set_color_space(ffmpeg::color::Space::SMPTE170M);
        } else {
            converted.set_color_space(ffmpeg::color::Space::RGB);
        }
        // SAFETY: both frames remain alive; output and scaler are exclusively
        // borrowed. FFmpeg allocates the output planes and reads input metadata.
        let result = unsafe {
            ffmpeg::ffi::sws_scale_frame(self.0, converted.as_mut_ptr(), source.as_ptr())
        };
        if result < 0 {
            bail!(
                "Converting video pixels: {} at {}:{}",
                ffmpeg::Error::from(result),
                file!(),
                line!()
            );
        }
        Ok(converted)
    }
}

#[cfg(target_os = "macos")]
pub fn video_surface(frame: &ffmpeg::frame::Video) -> Result<CVPixelBuffer> {
    use core_foundation::{
        base::TCFType,
        boolean::CFBoolean,
        dictionary::{CFDictionary, CFMutableDictionary},
        string::CFString,
    };
    use core_video::{
        pixel_buffer::{CVPixelBufferKeys, kCVPixelFormatType_420YpCbCr8BiPlanarFullRange},
        r#return::kCVReturnSuccess,
    };

    if frame.format() != ffmpeg::format::Pixel::NV12 || frame.planes() != 2 {
        bail!("Expected a two-plane NV12 frame at {}:{}", file!(), line!());
    }
    let width = frame.width() as usize;
    let height = frame.height() as usize;
    let mut attributes = CFMutableDictionary::<CFString, core_foundation::base::CFType>::new();
    attributes.add(
        &CVPixelBufferKeys::MetalCompatibility.into(),
        &CFBoolean::true_value().as_CFType(),
    );
    let iosurface = CFDictionary::<CFString, core_foundation::base::CFType>::from_CFType_pairs(&[]);
    attributes.add(
        &CVPixelBufferKeys::IOSurfaceProperties.into(),
        &iosurface.as_CFType(),
    );
    let surface = match CVPixelBuffer::new(
        kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
        width,
        height,
        Some(&attributes.to_immutable()),
    ) {
        Ok(surface) => surface,
        Err(error) => bail!(
            "Allocating video surface: {error} at {}:{}",
            file!(),
            line!()
        ),
    };
    if surface.get_plane_count() != 2 {
        bail!(
            "Expected a two-plane video surface at {}:{}",
            file!(),
            line!()
        );
    }
    if surface.lock_base_address(0) != kCVReturnSuccess {
        bail!("Locking video surface at {}:{}", file!(), line!());
    }
    let copied = (|| -> Result<()> {
        for (plane, rows, bytes) in [
            (0, height, width),
            (1, height.div_ceil(2), width.div_ceil(2) * 2),
        ] {
            let stride = surface.get_bytes_per_row_of_plane(plane);
            // SAFETY: The pixel buffer is locked and this plane exists.
            let destination = unsafe { surface.get_base_address_of_plane(plane) } as *mut u8;
            if destination.is_null()
                || stride < bytes
                || surface.get_height_of_plane(plane) < rows
                || frame.stride(plane) < bytes
            {
                bail!("Invalid video surface plane at {}:{}", file!(), line!());
            }
            for row in 0..rows {
                let start = row * frame.stride(plane);
                let source = frame
                    .data(plane)
                    .get(start..start + bytes)
                    .context(format!(
                        "Truncated NV12 video row at {}:{}",
                        file!(),
                        line!()
                    ))?;
                // SAFETY: the owned surface is locked; validated strides/heights
                // cover this row. Source is a checked, separately owned slice.
                unsafe {
                    ptr::copy_nonoverlapping(source.as_ptr(), destination.add(row * stride), bytes);
                }
            }
        }
        Ok(())
    })();
    let unlocked = surface.unlock_base_address(0);
    copied?;
    if unlocked != kCVReturnSuccess {
        bail!(
            "Unlocking video surface: {unlocked} at {}:{}",
            file!(),
            line!()
        );
    }
    Ok(surface)
}
