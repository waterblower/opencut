use super::nearest_canvas_snap;
use gpui::{Bounds, point, size};

#[test]
fn snaps_clip_centers_and_edges_to_canvas_centers_and_edges() {
    let canvas_guides = [50.0, 0.0, 100.0];

    assert_eq!(
        nearest_canvas_snap([3.0, -10.0, 20.0], &canvas_guides),
        Some((-3.0, 0.0))
    );
    assert_eq!(
        nearest_canvas_snap([30.0, 48.0, 60.0], &canvas_guides),
        Some((2.0, 50.0))
    );
    assert_eq!(
        nearest_canvas_snap([97.0, 80.0, 110.0], &canvas_guides),
        Some((3.0, 100.0))
    );
    assert_eq!(
        nearest_canvas_snap([20.0, 10.0, 30.0], &canvas_guides),
        None
    );
}

#[test]
fn resizing_each_corner_preserves_aspect_ratio_and_opposite_corner() {
    use super::{VideoClipProperties, resized_preview_properties};
    let original = VideoClipProperties {
        position_x: 20.0,
        position_y: -10.0,
        scale: 1.0,
    };
    let rect = Bounds::new(point(10.0, 30.0), size(200.0, 100.0));
    for corner in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let resized = resized_preview_properties(
            original,
            rect,
            corner,
            (corner.0 * 100.0, corner.1 * 50.0),
            0.5,
        );
        assert_eq!(resized.scale, 1.5);
        let center_delta_x = (resized.position_x - original.position_x) * 0.5;
        let center_delta_y = (resized.position_y - original.position_y) * 0.5;
        assert_eq!(
            center_delta_x - corner.0 * rect.size.width * resized.scale / 2.0,
            -corner.0 * rect.size.width / 2.0
        );
        assert_eq!(
            center_delta_y - corner.1 * rect.size.height * resized.scale / 2.0,
            -corner.1 * rect.size.height / 2.0
        );
    }
}

#[test]
fn resizing_past_the_opposite_corner_does_not_flip_the_clip() {
    use super::{VideoClipProperties, resized_preview_properties};
    let resized = resized_preview_properties(
        VideoClipProperties::default(),
        Bounds::new(point(0.0, 0.0), size(200.0, 100.0)),
        (1.0, 1.0),
        (-400.0, -200.0),
        1.0,
    );
    assert_eq!(resized.scale, 0.01);
}

#[test]
fn resize_snaps_all_corners_without_moving_the_opposite_corner() {
    use super::{VideoClipProperties, snap_preview_resize};
    for corner in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let rect = Bounds::new(point(100.0, 100.0), size(200.0, 100.0));
        let fixed_x = rect.origin.x + (1.0 - corner.0) * rect.size.width / 2.0;
        let target = fixed_x + corner.0 * 204.0;
        let (result, x, y) = snap_preview_resize(
            VideoClipProperties::default(),
            rect,
            corner,
            1.0,
            &[target],
            &[],
        );
        assert!((result.scale - 1.02).abs() < 1e-9);
        assert!((result.position_x - corner.0 * 2.0).abs() < 1e-9);
        assert!((result.position_y - corner.1).abs() < 1e-9);
        assert_eq!(x, Some(target));
        assert_eq!(y, None);
    }
}

#[test]
fn resize_snaps_centers_and_only_shows_guides_that_match_final_geometry() {
    use super::{VideoClipProperties, snap_preview_resize};
    let rect = Bounds::new(point(0.0, 0.0), size(200.0, 100.0));
    // Snap the clip center to x=102, which also aligns its bottom with y=102.
    let (result, x, y) = snap_preview_resize(
        VideoClipProperties::default(),
        rect,
        (1.0, 1.0),
        1.0,
        &[102.0],
        &[102.0],
    );
    assert!((result.scale - 1.02).abs() < 1e-9);
    assert_eq!((x, y), (Some(102.0), Some(102.0)));
    // A conflicting y target needs more scale correction, so x wins.
    let (_, x, y) = snap_preview_resize(
        VideoClipProperties::default(),
        rect,
        (1.0, 1.0),
        1.0,
        &[202.0],
        &[104.0],
    );
    assert_eq!((x, y), (Some(202.0), None));
    let original = VideoClipProperties::default();
    assert_eq!(
        snap_preview_resize(original, rect, (1.0, 1.0), 1.0, &[400.0], &[400.0]),
        (original, None, None)
    );
}

#[test]
fn preview_does_not_snap_five_pixels_away() {
    assert_eq!(nearest_canvas_snap([5.0, 15.0, 25.0], &[0.0]), None);
    use super::{VideoClipProperties, snap_preview_resize};
    let properties = VideoClipProperties::default();
    let rect = Bounds::new(point(0.0, 0.0), size(200.0, 100.0));
    assert_eq!(
        snap_preview_resize(properties, rect, (1.0, 1.0), 1.0, &[205.0], &[]),
        (properties, None, None)
    );
}

#[test]
fn hit_testing_uses_frontmost_rectangle_and_excludes_letterboxing() {
    use super::{TimelinePreviewCanvas, hit_preview_clip};
    let canvas = TimelinePreviewCanvas {
        bounds: Bounds::new(point(100.0, 50.0), size(400.0, 200.0)),
        project_scale: 0.5,
    };
    let front = super::Ulid::from(1_u128);
    let back = super::Ulid::from(2_u128);
    let rects = [
        (front, Bounds::new(point(120.0, 60.0), size(80.0, 30.0))),
        (back, Bounds::new(point(0.0, 0.0), size(600.0, 400.0))),
    ];
    assert_eq!(hit_preview_clip(&rects, canvas, 140.0, 70.0), Some(front));
    assert_eq!(hit_preview_clip(&rects, canvas, 200.0, 90.0), Some(front));
    assert_eq!(hit_preview_clip(&rects, canvas, 500.0, 250.0), Some(back));
    assert_eq!(hit_preview_clip(&rects, canvas, 300.0, 150.0), Some(back));
    assert_eq!(hit_preview_clip(&rects, canvas, 30.0, 70.0), None);
    assert_eq!(hit_preview_clip(&rects, canvas, 140.0, 20.0), None);
    assert_eq!(hit_preview_clip(&[], canvas, 140.0, 70.0), None);
    assert_eq!(
        hit_preview_clip(
            &[(front, Bounds::new(point(500.0, 60.0), size(20.0, 30.0)))],
            canvas,
            500.0,
            70.0,
        ),
        None,
    );
}

#[test]
fn rendered_text_bounds_follow_real_pixels_and_preview_scaling() {
    use super::{TimelinePreviewCanvas, rendered_text_rect};
    use gst::prelude::*;
    use gstreamer as gst;
    let _guard = crate::editor::tests::lock_gstreamer_test();
    gst::init().unwrap();
    for text in ["Title", "Two\nlines", "你好 世界", ""] {
        let pipeline = gst::parse::launch(
            "videotestsrc pattern=black num-buffers=1 ! video/x-raw,width=640,height=360 ! textoverlay name=title auto-resize=false font-desc=\"Sans 32px\" ! fakesink",
        ).unwrap().downcast::<gst::Pipeline>().unwrap();
        let overlay = pipeline.by_name("title").unwrap();
        overlay.set_property("text", text);
        let canvas = TimelinePreviewCanvas {
            bounds: Bounds::new(point(70.0, 20.0), size(320.0, 180.0)),
            project_scale: 0.5,
        };
        assert!(rendered_text_rect(overlay.upcast_ref(), canvas).is_none());
        pipeline.set_state(gst::State::Paused).unwrap();
        let ready = pipeline.state(gst::ClockTime::from_seconds(5));
        let rect = rendered_text_rect(overlay.upcast_ref(), canvas);
        let x = overlay.property::<i32>("text-x");
        let y = overlay.property::<i32>("text-y");
        let width = overlay.property::<u32>("text-width");
        let height = overlay.property::<u32>("text-height");
        pipeline.set_state(gst::State::Null).unwrap();
        ready.0.unwrap();
        assert_eq!(ready.1, gst::State::Paused);
        if text.is_empty() {
            continue;
        }
        let rect = rect.expect("rendered text has bounds");
        assert_eq!(rect.origin.x, 70.0 + f64::from(x) * 0.5);
        assert_eq!(rect.origin.y, 20.0 + f64::from(y) * 0.5);
        assert_eq!(rect.size.width, f64::from(width) * 0.5);
        assert_eq!(rect.size.height, f64::from(height) * 0.5);
        assert!(rect.size.width > 0.0 && rect.size.height > 0.0);
    }
}

#[test]
fn ges_text_rectangles_respect_visibility_time_and_locked_selection() {
    use super::*;
    use crate::editor::tests::{TimelineTestExt, lock_gstreamer_test, ulid};
    use ges::prelude::*;
    use gstreamer as gst;
    use gstreamer_editing_services as ges;
    let _guard = lock_gstreamer_test();
    ges::init().unwrap();
    let mut data = TimelineSerialization::with_test_tracks();
    data.settings.width = 640;
    data.settings.height = 360;
    let id = ulid(920);
    data.tracks.push(Track {
        id,
        name: "Text".into(),
        kind: TrackKind::Text,
        locked: true,
        muted: false,
        visible: true,
    });
    let clip_id = ulid(921);
    data.clips.push(Clip::Text(TextClip {
        id: clip_id,
        track_id: id,
        timeline_start: TimelineTime::ZERO,
        length: Duration::from_secs(2),
        properties: TextClipProperties::default(),
    }));
    let timeline = export_gstreamer::build_ges_timeline(
        &data,
        std::path::Path::new("."),
        export::ExportOptions::from_timeline(&data),
        false,
    )
    .unwrap();
    let canvas = TimelinePreviewCanvas {
        bounds: Bounds::new(point(20.0, 10.0), size(320.0, 180.0)),
        project_scale: 0.5,
    };
    assert!(timeline_preview_clip_rects(&data, &timeline, TimelineTime::ZERO, canvas).is_empty());
    let pipeline = ges::Pipeline::new();
    pipeline.preview_set_video_sink(Some(
        &gst::ElementFactory::make("fakesink").build().unwrap(),
    ));
    pipeline.preview_set_audio_sink(Some(
        &gst::ElementFactory::make("fakesink").build().unwrap(),
    ));
    pipeline.set_timeline(&timeline).unwrap();
    pipeline.set_mode(ges::PipelineFlags::FULL_PREVIEW).unwrap();
    pipeline.set_state(gst::State::Paused).unwrap();
    let ready = pipeline.state(gst::ClockTime::from_seconds(5));
    let rects = timeline_preview_clip_rects(&data, &timeline, TimelineTime::ZERO, canvas);
    let end = data.clips[0].timeline_end(data.settings.frame_rate);
    let ended = timeline_preview_clip_rects(&data, &timeline, end, canvas);
    data.tracks.last_mut().unwrap().visible = false;
    let hidden = timeline_preview_clip_rects(&data, &timeline, TimelineTime::ZERO, canvas);
    data.tracks.last_mut().unwrap().visible = true;
    let Clip::Text(text) = &mut data.clips[0] else {
        unreachable!()
    };
    text.properties.text.clear();
    let empty = timeline_preview_clip_rects(&data, &timeline, TimelineTime::ZERO, canvas);
    pipeline.set_state(gst::State::Null).unwrap();
    ready.0.unwrap();
    assert_eq!(ready.1, gst::State::Paused);
    assert_eq!(rects.len(), 1);
    let rect = rects[0].1;
    assert_eq!(
        hit_preview_clip(
            &rects,
            canvas,
            rect.origin.x + rect.size.width / 2.0,
            rect.origin.y + rect.size.height / 2.0
        ),
        Some(clip_id)
    );
    assert!(ended.is_empty());
    assert!(hidden.is_empty());
    assert!(empty.is_empty());
}
