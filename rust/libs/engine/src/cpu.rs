//! Software conversion of decoded video frames into GPUI images, for platforms without
//! the Metal surface path.

use anyhow::{Context as _, Result, bail};
use ffmpeg_next::{
    ffi::{
        SWS_CS_BT2020, SWS_CS_ITU601, SWS_CS_ITU709, sws_getCoefficients, sws_setColorspaceDetails,
    },
    format::Pixel,
    frame::Video,
    software::scaling,
    util::color,
};
use gpui::{App, RenderImage, Window};
use image::{Frame, RgbaImage};
use media_backend::VideoFrame;
use smallvec::smallvec;
use std::{
    cell::RefCell,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// A converted picture. Clones share one GPUI image. When the last clone drops, a painted
/// image waits for [`release_retired_surfaces`] to free its sprite atlas tile; an image
/// that was never painted has no tile and is freed at once.
#[derive(Clone)]
pub struct CpuSurface {
    image: Arc<RetireOnDrop>,
}

impl CpuSurface {
    pub(crate) fn image(&self) -> Arc<RenderImage> {
        Arc::clone(&self.image.image)
    }

    /// Call from the painting lifecycle, so dropping the surface frees its atlas tile.
    pub(crate) fn mark_painted(&self) {
        self.image.painted.store(true, Ordering::Relaxed);
    }
}

/// Reusable scaler only; playback state stays in the caller.
pub struct CpuResources {
    scaler: Option<(ScalerKey, scaling::Context)>, // 源格式变化时重建。
}

impl CpuResources {
    /// Matches the Metal converter's constructor; the scaler follows each frame's own size.
    pub fn new(_dimensions: (usize, usize)) -> Result<Self> {
        Ok(Self { scaler: None })
    }

    /// Converts the decoded picture to BGRA, which GPUI images expect.
    pub fn convert(&mut self, frame: &VideoFrame) -> Result<CpuSurface> {
        if frame.rotation_degrees.rem_euclid(360.0) != 0.0 {
            bail!(
                "Rotated video is not supported: {} degrees",
                frame.rotation_degrees
            );
        }
        let source = &frame.native;
        let (width, height) = (source.width(), source.height());
        let coefficients = match frame.color_space {
            color::Space::BT709 => SWS_CS_ITU709,
            color::Space::BT470BG | color::Space::SMPTE170M => SWS_CS_ITU601,
            color::Space::BT2020NCL => SWS_CS_BT2020,
            color::Space::Unspecified if height >= 720 => SWS_CS_ITU709,
            color::Space::Unspecified => SWS_CS_ITU601,
            other => bail!("Unsupported video color space: {other:?}"),
        };
        let key = ScalerKey {
            format: source.format(),
            width,
            height,
            coefficients,
            full_range: source.color_range() == color::Range::JPEG,
        };
        let scaler = match &mut self.scaler {
            Some((cached, scaler)) if *cached == key => scaler,
            slot => &mut slot.insert((key, create_scaler(key)?)).1,
        };
        let mut output = Video::empty();
        scaler
            .run(source, &mut output)
            .context("converting video frame to BGRA")?;

        // 输出行可能带对齐填充，逐行拷贝成紧密排列。
        let row_bytes = width as usize * 4;
        let stride = output.stride(0);
        let mut bytes = Vec::with_capacity(row_bytes * height as usize);
        for row in output.data(0).chunks(stride).take(height as usize) {
            bytes.extend_from_slice(&row[..row_bytes]);
        }
        let pixels = RgbaImage::from_raw(width, height, bytes)
            .context("BGRA frame size does not match its dimensions")?;
        Ok(CpuSurface {
            image: Arc::new(RetireOnDrop {
                image: Arc::new(RenderImage::new(smallvec![Frame::new(pixels)])),
                painted: AtomicBool::new(false),
            }),
        })
    }
}

/// Frees atlas tiles of surfaces dropped on this thread. Call while painting in the
/// window that showed them.
pub(crate) fn release_retired_surfaces(window: &mut Window, cx: &mut App) {
    let retired = RETIRED.take();
    for image in retired {
        cx.drop_image(image, Some(window));
    }
}

thread_local! {
    // 已绘制、已无人持有、等待从图集释放的图像；GPUI 不会自动回收 Arc<RenderImage> 的图集块。
    // 只有绘制会入队，而绘制也会清空队列；窗口最小化不绘制时队列不会增长。
    static RETIRED: RefCell<Vec<Arc<RenderImage>>> = const { RefCell::new(Vec::new()) };
}

#[rustfmt::skip]
struct RetireOnDrop {
    image: Arc<RenderImage>,
    painted: AtomicBool, // 交给过绘制，图集里可能有它的图块；未绘制的图像没有图块。
}

impl Drop for RetireOnDrop {
    fn drop(&mut self) {
        if !self.painted.load(Ordering::Relaxed) {
            return;
        }
        let image = Arc::clone(&self.image);
        // 只在线程退出、RETIRED 已销毁时失败；此时窗口和图集已一并释放，无需回收。
        let _ = RETIRED.try_with(|retired| retired.borrow_mut().push(image));
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct ScalerKey {
    format: Pixel,
    width: u32,
    height: u32,
    coefficients: i32,
    full_range: bool,
}

fn create_scaler(key: ScalerKey) -> Result<scaling::Context> {
    let mut scaler = scaling::Context::get(
        key.format,
        key.width,
        key.height,
        Pixel::BGRA,
        key.width,
        key.height,
        scaling::Flags::BILINEAR,
    )
    .context("creating video frame scaler")?;
    // SAFETY: the context is open; the coefficient tables are static FFmpeg data.
    let status = unsafe {
        sws_setColorspaceDetails(
            scaler.as_mut_ptr(),
            sws_getCoefficients(key.coefficients),
            i32::from(key.full_range),
            sws_getCoefficients(SWS_CS_ITU601),
            1,
            0,
            1 << 16,
            1 << 16,
        )
    };
    if status < 0 {
        bail!("setting video frame scaler colorspace failed: {status}");
    }
    Ok(scaler)
}
