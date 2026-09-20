use crate::editor::{
    preview_timeline::TimelinePreviewFrame,
    timeline_backend::{TimelineFrame, TimelineLayer},
};
use image::{Rgba, RgbaImage};
use std::{sync::Arc, time::Duration};
use ulid::Ulid;

#[test]
fn prepares_bgra_without_changing_alpha_or_source_pixels() {
    let clip_id = Ulid::from(1_u128);
    let pixels = Arc::new(RgbaImage::from_pixel(1, 1, Rgba([10, 20, 30, 128])));
    let frame = Arc::new(TimelineFrame {
        timestamp: Duration::ZERO,
        width: 1920,
        height: 1080,
        layers: vec![TimelineLayer::Image {
            clip_id,
            pixels: Arc::clone(&pixels),
            properties: Default::default(),
        }],
    });
    let prepared = TimelinePreviewFrame::new(Arc::clone(&frame));
    assert!(Arc::ptr_eq(&prepared.frame, &frame));
    assert_eq!(
        prepared.images[&clip_id].as_bytes(0).unwrap(),
        &[30, 20, 10, 128]
    );
    assert_eq!(pixels.get_pixel(0, 0).0, [10, 20, 30, 128]);
}
