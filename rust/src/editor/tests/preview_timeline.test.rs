use super::nearest_canvas_snap;

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
    use super::{RenderRect, VideoClipProperties, resized_preview_properties};
    let original = VideoClipProperties {
        position_x: 20.0,
        position_y: -10.0,
        scale: 1.0,
    };
    let rect = RenderRect {
        left: 10.0,
        top: 30.0,
        width: 200.0,
        height: 100.0,
    };
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
            center_delta_x - corner.0 * rect.width * resized.scale / 2.0,
            -corner.0 * rect.width / 2.0
        );
        assert_eq!(
            center_delta_y - corner.1 * rect.height * resized.scale / 2.0,
            -corner.1 * rect.height / 2.0
        );
    }
}

#[test]
fn resizing_past_the_opposite_corner_does_not_flip_the_clip() {
    use super::{RenderRect, VideoClipProperties, resized_preview_properties};
    let resized = resized_preview_properties(
        VideoClipProperties::default(),
        RenderRect {
            left: 0.0,
            top: 0.0,
            width: 200.0,
            height: 100.0,
        },
        (1.0, 1.0),
        (-400.0, -200.0),
        1.0,
    );
    assert_eq!(resized.scale, 0.01);
}

#[test]
fn resize_snaps_all_corners_without_moving_the_opposite_corner() {
    use super::{RenderRect, VideoClipProperties, snap_preview_resize};
    for corner in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let rect = RenderRect {
            left: 100.0,
            top: 100.0,
            width: 200.0,
            height: 100.0,
        };
        let fixed_x = rect.left + (1.0 - corner.0) * rect.width / 2.0;
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
    use super::{RenderRect, VideoClipProperties, snap_preview_resize};
    let rect = RenderRect {
        left: 0.0,
        top: 0.0,
        width: 200.0,
        height: 100.0,
    };
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
    use super::{RenderRect, VideoClipProperties, snap_preview_resize};
    let properties = VideoClipProperties::default();
    let rect = RenderRect {
        left: 0.0,
        top: 0.0,
        width: 200.0,
        height: 100.0,
    };
    assert_eq!(
        snap_preview_resize(properties, rect, (1.0, 1.0), 1.0, &[205.0], &[]),
        (properties, None, None)
    );
}
