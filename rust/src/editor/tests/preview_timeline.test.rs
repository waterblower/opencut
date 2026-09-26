use crate::editor::{
    preview_timeline::TimelinePreviewFrame,
    timeline_backend::{TimelineFrame, TimelineLayer},
};
use image::{Rgba, RgbaImage};
use std::{sync::Arc, time::Duration};
use ulid::Ulid;

#[cfg(feature = "ffmpeg-video-tests")]
#[gpui::test]
fn canvas_requests_another_frame_only_while_loading(cx: &mut gpui::TestAppContext) {
    use crate::editor::preview_timeline::TimelinePreviewCanvasElement;
    use anyhow::anyhow;
    use gpui::{AppContext, Context, IntoElement, Render, point, px, size};

    struct PreviewProbe(u8);
    impl Render for PreviewProbe {
        fn render(&mut self, _: &mut gpui::Window, _: &mut Context<Self>) -> impl IntoElement {
            let frame = match self.0 {
                0 => Ok(None),
                1 => Ok(Some(Arc::new(TimelinePreviewFrame::new(Arc::new(
                    TimelineFrame {
                        timestamp: Duration::ZERO,
                        width: 160,
                        height: 90,
                        layers: Vec::new(),
                    },
                ))))),
                _ => Err(anyhow!("decode failed")),
            };
            TimelinePreviewCanvasElement {
                frame,
                id: "preview-test".into(),
                size: size(px(320.0), px(240.0)),
            }
        }
    }

    let window = cx.add_empty_window();
    let view = window.new(|_| PreviewProbe(0));
    for state in 0..3 {
        view.update(window, |view, _| view.0 = state);
        window.draw(
            point(px(0.0), px(0.0)),
            size(px(320.0), px(240.0)),
            |_, _| view.clone().into_element(),
        );
        window.update(|window, cx| {
            assert_eq!(window.simulate_next_frame(cx) > 0, state == 0);
        });
    }
}

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
