use anyhow::{Context as _, Result, bail};
use ffmpeg_next as ffmpeg;
use image::RgbaImage;
use media_backend::VideoFrame;

/// Converts native video frames to unpremultiplied RGBA, applying display rotation.
/// The caller owns the scaler cache; pass `None` initially.
pub fn frame_to_rgba(
    frame: &VideoFrame,
    scaler: &mut Option<ffmpeg::software::scaling::Context>,
) -> Result<RgbaImage> {
    let mut transferred = ffmpeg::frame::Video::empty();
    // SAFETY: frame owns its AVFrame; the transfer destination is exclusively owned.
    let hardware = unsafe { !(*frame.native.as_ptr()).hw_frames_ctx.is_null() };
    let source = if hardware {
        let result = unsafe {
            ffmpeg::ffi::av_hwframe_transfer_data(
                transferred.as_mut_ptr(),
                frame.native.as_ptr(),
                0,
            )
        };
        if result < 0 {
            return Err(ffmpeg::Error::from(result)).context("Transferring video frame");
        }
        &transferred
    } else {
        &frame.native
    };
    let definition = ffmpeg::software::scaling::context::Definition {
        format: source.format(),
        width: source.width(),
        height: source.height(),
    };
    let reconfigure = match scaler.as_ref() {
        Some(scaler) => *scaler.input() != definition,
        None => true,
    };
    if reconfigure {
        *scaler = Some(ffmpeg::software::scaling::Context::get(
            source.format(),
            source.width(),
            source.height(),
            ffmpeg::format::Pixel::RGBA,
            source.width(),
            source.height(),
            ffmpeg::software::scaling::Flags::BILINEAR,
        )?);
    }
    let scaler = scaler.as_mut().context("Missing video scaler")?;
    let matrix = match frame.color_space {
        ffmpeg::color::Space::BT709 => ffmpeg::ffi::SWS_CS_ITU709,
        ffmpeg::color::Space::BT2020NCL | ffmpeg::color::Space::BT2020CL => {
            ffmpeg::ffi::SWS_CS_BT2020
        }
        ffmpeg::color::Space::FCC => ffmpeg::ffi::SWS_CS_FCC,
        ffmpeg::color::Space::SMPTE240M => ffmpeg::ffi::SWS_CS_SMPTE240M,
        ffmpeg::color::Space::BT470BG | ffmpeg::color::Space::SMPTE170M => {
            ffmpeg::ffi::SWS_CS_ITU601
        }
        _ if source.height() >= 720 => ffmpeg::ffi::SWS_CS_ITU709,
        _ => ffmpeg::ffi::SWS_CS_ITU601,
    };
    // SAFETY: coefficients have static lifetime; scaler is exclusively owned.
    let result = unsafe {
        let coefficients = ffmpeg::ffi::sws_getCoefficients(matrix);
        ffmpeg::ffi::sws_setColorspaceDetails(
            scaler.as_mut_ptr(),
            coefficients,
            i32::from(frame.color_range == ffmpeg::color::Range::JPEG),
            coefficients,
            1,
            0,
            1 << 16,
            1 << 16,
        )
    };
    if result < 0 {
        return Err(ffmpeg::Error::from(result)).context("Configuring video colors");
    }
    let mut rgba = ffmpeg::frame::Video::empty();
    scaler
        .run(source, &mut rgba)
        .context("Converting video frame")?;
    let mut pixels = RgbaImage::new(rgba.width(), rgba.height());
    let row_bytes = rgba.width() as usize * 4;
    for (row, output) in pixels.as_mut().chunks_exact_mut(row_bytes).enumerate() {
        let offset = row * rgba.stride(0);
        output.copy_from_slice(&rgba.data(0)[offset..offset + row_bytes]);
    }
    let quarter = frame.rotation_degrees / 90.0;
    if !quarter.is_finite() || (quarter - quarter.round()).abs() > 0.1 / 90.0 {
        bail!(
            "Unsupported display rotation: {} degrees",
            frame.rotation_degrees
        );
    }
    pixels = match quarter.round().rem_euclid(4.0) as u32 {
        1 => image::imageops::rotate270(&pixels),
        2 => image::imageops::rotate180(&pixels),
        3 => image::imageops::rotate90(&pixels),
        _ => pixels,
    };
    Ok(pixels)
}
