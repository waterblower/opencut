use super::*;
use anyhow::anyhow;
use ffmpeg_next as ffmpeg;
use std::{
    sync::{Mutex, mpsc},
    time::Duration,
};

pub fn rgb_frame(width: u32, height: u32, color: [u8; 3]) -> Arc<VideoFrame> {
    let mut image = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::RGB24, width, height);
    image.set_color_space(ffmpeg::color::Space::RGB);
    image.set_color_range(ffmpeg::color::Range::JPEG);
    image.data_mut(0).fill(0xa5);
    let stride = image.stride(0);
    for row in 0..height as usize {
        for pixel in
            image.data_mut(0)[row * stride..row * stride + width as usize * 3].chunks_exact_mut(3)
        {
            pixel.copy_from_slice(&color);
        }
    }
    Arc::new(VideoFrame {
        timestamp: Duration::ZERO,
        image,
    })
}

#[test]
fn constructor_defaults_builders_and_backend_errors() -> Result<()> {
    let frame = rgb_frame(67, 35, [255, 0, 0]);
    let mut state = super::super::State::new();
    state.frame = Some(Arc::clone(&frame));
    let (commands, _receiver) = mpsc::channel();
    let backend = VideoBackend {
        shared: Arc::new(Mutex::new(state)),
        commands,
        workers: Vec::new(),
    };
    let element = video(&backend)?;
    assert_eq!((element.width, element.height), (px(67.0), px(35.0)));
    assert!(element.id.is_none());
    assert!(Arc::ptr_eq(&element.frame, &frame));
    let element = element.id("video").size(px(400.0), px(300.0));
    assert_eq!(element.id, Some("video".into()));
    assert_eq!((element.width, element.height), (px(400.0), px(300.0)));
    super::super::lock(&backend.shared).fail(&anyhow!(
        "Injected failure at {}:{}",
        file!(),
        line!()
    ));
    assert!(video(&backend).is_err());
    Ok(())
}

#[test]
fn fitted_bounds_center_wide_tall_and_empty_images() {
    let bounds = Bounds::new(point(px(10.0), px(20.0)), size(px(100.0), px(100.0)));
    assert_eq!(
        fitted_bounds(bounds, 200, 100),
        Bounds::new(point(px(10.0), px(45.0)), size(px(100.0), px(50.0)))
    );
    assert_eq!(
        fitted_bounds(bounds, 100, 200),
        Bounds::new(point(px(35.0), px(20.0)), size(px(50.0), px(100.0)))
    );
    assert_eq!(fitted_bounds(bounds, 0, 100).size, size(px(0.0), px(0.0)));
    assert_eq!(
        fitted_bounds(
            Bounds::new(bounds.origin, size(px(0.0), px(100.0))),
            200,
            100
        )
        .size,
        size(px(0.0), px(0.0))
    );
}

#[test]
fn bgra_conversion_preserves_channel_order_odd_dimensions_and_input() -> Result<()> {
    let mut converter = Converter::default();
    for color in [[255, 0, 0], [0, 255, 0], [0, 0, 255]] {
        let frame = rgb_frame(67, 35, color);
        assert!(frame.image.stride(0) > 67 * 3);
        let image = converter.bgra(&frame.image)?;
        let bytes = image.as_bytes(0).expect("converted image has a frame");
        assert_eq!(bytes.len(), 67 * 35 * 4);
        for pixel in bytes.chunks_exact(4) {
            assert_eq!(pixel, [color[2], color[1], color[0], 255]);
        }
        assert_eq!(frame.image.format(), ffmpeg::format::Pixel::RGB24);
        assert_eq!(&frame.image.data(0)[..3], &color);
    }
    Ok(())
}

#[test]
fn limited_bt709_and_full_bt601_convert_to_the_same_red() -> Result<()> {
    let mut converter = Converter::default();
    for (format, space, range, values) in [
        (
            ffmpeg::format::Pixel::YUV420P,
            ffmpeg::color::Space::BT709,
            ffmpeg::color::Range::MPEG,
            [63, 102, 240],
        ),
        (
            ffmpeg::format::Pixel::NV12,
            ffmpeg::color::Space::SMPTE170M,
            ffmpeg::color::Range::JPEG,
            [76, 85, 255],
        ),
    ] {
        let mut source = ffmpeg::frame::Video::new(format, 66, 34);
        source.set_color_space(space);
        source.set_color_range(range);
        source.data_mut(0).fill(values[0]);
        if format == ffmpeg::format::Pixel::NV12 {
            for chroma in source.data_mut(1).chunks_exact_mut(2) {
                chroma.copy_from_slice(&values[1..]);
            }
        } else {
            source.data_mut(1).fill(values[1]);
            source.data_mut(2).fill(values[2]);
        }
        let image = converter.bgra(&source)?;
        let pixel = &image.as_bytes(0).expect("converted frame")[..4];
        assert!(
            pixel[0] < 6 && pixel[1] < 6 && pixel[2] > 248,
            "unexpected BGRA: {pixel:?}"
        );
        assert_eq!(source.color_space(), space);
        assert_eq!(source.color_range(), range);
    }
    Ok(())
}

#[test]
fn ten_bit_sdr_and_empty_frames_are_handled() -> Result<()> {
    let mut source = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::YUV420P10LE, 66, 34);
    source.set_color_space(ffmpeg::color::Space::BT709);
    source.set_color_range(ffmpeg::color::Range::MPEG);
    for (plane, value) in [(0, 940_u16), (1, 512), (2, 512)] {
        for sample in source.data_mut(plane).chunks_exact_mut(2) {
            sample.copy_from_slice(&value.to_le_bytes());
        }
    }
    let mut converter = Converter::default();
    let image = converter.bgra(&source)?;
    for pixel in image.as_bytes(0).expect("converted frame").chunks_exact(4) {
        // Swscale's 10-bit integer conversion/dithering can round white down.
        assert!(
            pixel[..3].iter().all(|value| *value >= 252),
            "unexpected BGRA: {pixel:?}"
        );
        assert_eq!(pixel[3], 255);
    }
    assert!(converter.prepare(&ffmpeg::frame::Video::empty()).is_err());
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn nv12_surface_copies_all_visible_rows_including_odd_chroma() -> Result<()> {
    use core_video::r#return::kCVReturnSuccess;
    let frame = rgb_frame(67, 35, [255, 0, 0]);
    let converted = Converter::default().convert(&frame.image, ffmpeg::format::Pixel::NV12)?;
    assert_eq!(converted.color_space(), ffmpeg::color::Space::SMPTE170M);
    assert_eq!(converted.color_range(), ffmpeg::color::Range::JPEG);
    assert!((70..85).contains(&converted.data(0)[0]));
    assert!((75..95).contains(&converted.data(1)[0]));
    assert!(converted.data(1)[1] > 240);
    let surface = conversion::video_surface(&converted)?;
    assert_eq!((surface.get_width(), surface.get_height()), (67, 35));
    assert_eq!(surface.lock_base_address(0), kCVReturnSuccess);
    for (plane, rows, bytes) in [(0, 35, 67), (1, 18, 68)] {
        let stride = surface.get_bytes_per_row_of_plane(plane);
        // SAFETY: The pixel buffer is locked and this plane exists.
        let pointer = unsafe { surface.get_base_address_of_plane(plane) } as *const u8;
        for row in 0..rows {
            // SAFETY: the surface is locked and its validated plane owns this row.
            let actual = unsafe { std::slice::from_raw_parts(pointer.add(row * stride), bytes) };
            let start = row * converted.stride(plane);
            assert_eq!(actual, &converted.data(plane)[start..start + bytes]);
        }
    }
    assert_eq!(surface.unlock_base_address(0), kCVReturnSuccess);
    Ok(())
}
