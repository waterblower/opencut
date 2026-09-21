use anyhow::{Context, Result, anyhow, bail};
use core_foundation::{
    base::{CFType, TCFType},
    boolean::CFBoolean,
    dictionary::CFDictionary,
    number::CFNumber,
    string::CFString,
};
use core_video::{
    metal_texture::{CVMetalTextureGetTexture, CVMetalTextureKeys},
    metal_texture_cache::CVMetalTextureCache,
    pixel_buffer::{
        CVPixelBuffer, CVPixelBufferKeys, CVPixelBufferRef,
        kCVPixelFormatType_420YpCbCr8BiPlanarFullRange,
        kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange,
        kCVPixelFormatType_420YpCbCr10BiPlanarFullRange,
        kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange,
    },
    pixel_buffer_io_surface::CVPixelBufferGetIOSurface,
    pixel_buffer_pool::CVPixelBufferPool,
};
use ffmpeg_next::{format::Pixel, util::color};
use metal::{
    CommandQueue, CompileOptions, ComputePipelineState, Device, MTLCommandBufferStatus,
    MTLPixelFormat, MTLSize, MTLTextureUsage, TextureRef, foreign_types::ForeignTypeRef,
};
use opencut_player::video3::VideoFrame;

/// Reusable native resources only; playback state stays in the caller.
pub struct GpuResources {
    queue: CommandQueue,
    pipeline: ComputePipelineState,
    cache: CVMetalTextureCache,
    pool: CVPixelBufferPool,
    dimensions: (usize, usize),
}

/// Return a completed surface for unmodified GPUI, or None for CPU fallback.
/// Conversion stays on the GPU; waiting here deliberately keeps the loop serial.
pub fn convert(
    resources: &mut Option<GpuResources>,
    frame: &VideoFrame,
) -> Result<Option<CVPixelBuffer>> {
    if frame.native.format() != Pixel::VIDEOTOOLBOX
        || frame.rotation_degrees.rem_euclid(360.0) != 0.0
    {
        return Ok(None);
    }
    let (kr, kb) = match frame.color_space {
        color::Space::BT709 => (0.2126, 0.0722),
        color::Space::BT470BG | color::Space::SMPTE170M => (0.299, 0.114),
        color::Space::BT2020NCL => (0.2627, 0.0593),
        color::Space::Unspecified if frame.native.height() >= 720 => (0.2126, 0.0722),
        color::Space::Unspecified => (0.299, 0.114),
        _ => return Ok(None),
    };
    // SAFETY: FFmpeg owns the CVPixelBuffer in data[3]. Retain it with the get rule.
    let source = unsafe { (*frame.native.as_ptr()).data[3] as CVPixelBufferRef };
    if source.is_null() {
        return Ok(None);
    }
    let source = unsafe { CVPixelBuffer::wrap_under_get_rule(source) };
    let (bits, full_range, y_format, uv_format) = match source.get_pixel_format() {
        f if f == kCVPixelFormatType_420YpCbCr8BiPlanarFullRange => {
            (8, true, MTLPixelFormat::R8Unorm, MTLPixelFormat::RG8Unorm)
        }
        f if f == kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange => {
            (8, false, MTLPixelFormat::R8Unorm, MTLPixelFormat::RG8Unorm)
        }
        f if f == kCVPixelFormatType_420YpCbCr10BiPlanarFullRange => (
            10,
            true,
            MTLPixelFormat::R16Unorm,
            MTLPixelFormat::RG16Unorm,
        ),
        f if f == kCVPixelFormatType_420YpCbCr10BiPlanarVideoRange => (
            10,
            false,
            MTLPixelFormat::R16Unorm,
            MTLPixelFormat::RG16Unorm,
        ),
        _ => return Ok(None),
    };
    let dimensions = (source.get_width(), source.get_height());
    // SAFETY: inspect a borrowed IOSurface without releasing it. The dependency's
    // get_io_surface() incorrectly wraps this Get result with the create rule.
    let has_surface = unsafe { !CVPixelBufferGetIOSurface(source.as_concrete_TypeRef()).is_null() };
    if !has_surface
        || source.get_plane_count() != 2
        || dimensions
            != (
                frame.native.width() as usize,
                frame.native.height() as usize,
            )
        || dimensions.0 % 2 != 0
        || dimensions.1 % 2 != 0
    {
        return Ok(None);
    }
    // Already matches GPUI's fixed full-range BT.601 surface shader.
    if bits == 8
        && full_range
        && matches!(
            frame.color_space,
            color::Space::BT470BG | color::Space::SMPTE170M
        )
    {
        return Ok(Some(source));
    }
    if resources.is_none() {
        let Some(device) = Device::system_default() else {
            return Ok(None);
        };
        let library = device
            .new_library_with_source(include_str!("convert.metal"), &CompileOptions::new())
            .map_err(|error| anyhow!("compiling player Metal conversion: {error}"))?;
        let function = library
            .get_function("prepare_surface", None)
            .map_err(|error| anyhow!("loading player Metal conversion: {error}"))?;
        let pipeline = device
            .new_compute_pipeline_state_with_function(&function)
            .map_err(|error| anyhow!("creating player Metal pipeline: {error}"))?;
        let queue = device.new_command_queue();
        let cache = CVMetalTextureCache::new(None, device, None)
            .map_err(|status| anyhow!("creating player Metal texture cache: {status}"))?;
        *resources = Some(GpuResources {
            queue,
            pipeline,
            cache,
            pool: create_pool(dimensions)?,
            dimensions,
        });
    }
    let resources = resources
        .as_mut()
        .context("missing Metal conversion resources")?;
    if resources.dimensions != dimensions {
        resources.pool = create_pool(dimensions)?;
        resources.dimensions = dimensions;
    }
    let output = resources
        .pool
        .create_pixel_buffer()
        .map_err(|status| anyhow!("allocating player surface: {status}"))?;
    let write_attributes = CFDictionary::from_CFType_pairs(&[(
        CFString::from(CVMetalTextureKeys::Usage),
        CFNumber::from((MTLTextureUsage::ShaderRead | MTLTextureUsage::ShaderWrite).bits() as i64)
            .as_CFType(),
    )]);
    let command = resources.queue.new_command_buffer();
    let encoder = command.new_compute_command_encoder();
    encoder.set_compute_pipeline_state(&resources.pipeline);
    // Keep the CVMetalTexture owners alive through GPU completion, not just the
    // borrowed MTLTexture pointers. No pixel buffer is locked or read by the CPU.
    let mut textures = Vec::with_capacity(4);
    for (buffer, plane, format, writing) in [
        (&source, 0, y_format, false),
        (&source, 1, uv_format, false),
        (&output, 0, MTLPixelFormat::R8Unorm, true),
        (&output, 1, MTLPixelFormat::RG8Unorm, true),
    ] {
        let texture = resources.cache.create_texture_from_image(
            buffer.as_concrete_TypeRef(),
            if writing {
                Some(&write_attributes)
            } else {
                None
            },
            format,
            buffer.get_width_of_plane(plane),
            buffer.get_height_of_plane(plane),
            plane,
        );
        let texture = match texture {
            Ok(texture) => texture,
            Err(status) => {
                encoder.end_encoding();
                bail!("mapping player surface plane {plane} to Metal: {status}");
            }
        };
        // SAFETY: texture owns this borrowed Metal object until after completion.
        let native = unsafe { CVMetalTextureGetTexture(texture.as_concrete_TypeRef()) };
        if native.is_null() {
            encoder.end_encoding();
            bail!("CoreVideo returned an empty player Metal texture");
        }
        encoder.set_texture(
            textures.len() as u64,
            Some(unsafe { TextureRef::from_ptr(native.cast()) }),
        );
        textures.push(texture);
    }
    let transform = color_transform(kr, kb, bits, full_range);
    encoder.set_bytes(
        0,
        std::mem::size_of_val(&transform) as u64,
        transform.as_ptr().cast(),
    );
    encoder.dispatch_threads(
        MTLSize::new((dimensions.0 / 2) as u64, (dimensions.1 / 2) as u64, 1),
        MTLSize::new(16, 16, 1),
    );
    encoder.end_encoding();
    command.commit();
    command.wait_until_completed();
    if command.status() != MTLCommandBufferStatus::Completed {
        bail!("player Metal conversion failed: {:?}", command.status());
    }
    drop(textures);
    resources.cache.flush(0);
    Ok(Some(output))
}

fn create_pool((width, height): (usize, usize)) -> Result<CVPixelBufferPool> {
    let empty = CFDictionary::<CFString, CFType>::from_CFType_pairs(&[]);
    let attributes = CFDictionary::from_CFType_pairs(&[
        (
            CFString::from(CVPixelBufferKeys::Width),
            CFNumber::from(width as i64).as_CFType(),
        ),
        (
            CFString::from(CVPixelBufferKeys::Height),
            CFNumber::from(height as i64).as_CFType(),
        ),
        (
            CFString::from(CVPixelBufferKeys::PixelFormatType),
            CFNumber::from(kCVPixelFormatType_420YpCbCr8BiPlanarFullRange as i64).as_CFType(),
        ),
        (
            CFString::from(CVPixelBufferKeys::IOSurfaceProperties),
            empty.as_CFType(),
        ),
        (
            CFString::from(CVPixelBufferKeys::MetalCompatibility),
            CFBoolean::true_value().as_CFType(),
        ),
    ]);
    CVPixelBufferPool::new(None, Some(&attributes))
        .map_err(|status| anyhow!("creating player surface pool: {status}"))
}

// Input YUV -> nonlinear RGB. P010 codes occupy the high ten bits of R16Unorm.
// Neither this transform nor the output shader performs HDR tone mapping.
fn color_transform(kr: f32, kb: f32, bits: u32, full_range: bool) -> [[f32; 4]; 3] {
    let kg = 1.0 - kr - kb;
    let code_scale = if bits == 10 { 65535.0 / 64.0 } else { 255.0 };
    let level_scale = (1_u32 << (bits - 8)) as f32;
    let maximum = ((1_u32 << bits) - 1) as f32;
    let (black, y_range, uv_range) = if full_range {
        (0.0, maximum, maximum)
    } else {
        (16.0 * level_scale, 219.0 * level_scale, 224.0 * level_scale)
    };
    let ys = code_scale / y_range;
    let yo = -black / y_range;
    let us = code_scale / uv_range;
    let uo = -(128.0 * level_scale) / uv_range;
    let rv = 2.0 * (1.0 - kr);
    let bu = 2.0 * (1.0 - kb);
    let gu = -kb * bu / kg;
    let gv = -kr * rv / kg;
    [
        [ys, 0.0, rv * us, yo + rv * uo],
        [ys, gu * us, gv * us, yo + (gu + gv) * uo],
        [ys, bu * us, 0.0, yo + bu * uo],
    ]
}
