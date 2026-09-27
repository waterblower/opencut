use anyhow::{Context, Result, bail};
use ffmpeg_next::{Error as FfmpegError, codec, decoder, ffi};
use std::ptr;

/// The codec owns the device reference after successful creation. No opaque
/// application pointer or cross-thread native owner is needed by get_format.
pub fn configure(context: &mut codec::context::Context) -> Result<()> {
    if !cfg!(target_os = "macos") {
        bail!("VideoToolbox is only available on macOS");
    }
    let codec = decoder::find(context.id()).context("finding video codec")?;
    // SAFETY: codec is registered for the process lifetime; configuration entries
    // are immutable and terminated by a null entry. context is exclusively owned.
    unsafe {
        let mut index = 0;
        loop {
            let config = ffi::avcodec_get_hw_config(codec.as_ptr(), index);
            if config.is_null() {
                bail!("codec has no VideoToolbox device configuration");
            }
            if (*config).device_type == ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_VIDEOTOOLBOX
                && (*config).pix_fmt == ffi::AVPixelFormat::AV_PIX_FMT_VIDEOTOOLBOX
                && (*config).methods & ffi::AV_CODEC_HW_CONFIG_METHOD_HW_DEVICE_CTX as i32 != 0
            {
                break;
            }
            index += 1;
        }
        let mut device = ptr::null_mut();
        let result = ffi::av_hwdevice_ctx_create(
            &mut device,
            ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_VIDEOTOOLBOX,
            ptr::null(),
            ptr::null_mut(),
            0,
        );
        if result < 0 {
            return Err(FfmpegError::from(result)).context("creating VideoToolbox device");
        }
        (*context.as_mut_ptr()).hw_device_ctx = device;
        (*context.as_mut_ptr()).get_format = Some(select_format);
    }
    Ok(())
}

unsafe extern "C" fn select_format(
    _context: *mut ffi::AVCodecContext,
    formats: *const ffi::AVPixelFormat,
) -> ffi::AVPixelFormat {
    // SAFETY: FFmpeg supplies a NONE-terminated array for this callback. Refuse
    // implicit software selection so hardware failures propagate to the caller.
    unsafe {
        let mut candidate = formats;
        while !candidate.is_null() && *candidate != ffi::AVPixelFormat::AV_PIX_FMT_NONE {
            if *candidate == ffi::AVPixelFormat::AV_PIX_FMT_VIDEOTOOLBOX {
                return *candidate;
            }
            candidate = candidate.add(1);
        }
    }
    ffi::AVPixelFormat::AV_PIX_FMT_NONE
}
