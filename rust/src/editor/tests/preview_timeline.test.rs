use super::nearest_canvas_snap;

#[test]
fn snaps_clip_centers_and_edges_to_canvas_centers_and_edges() {
    let canvas_guides = [50.0, 0.0, 100.0];

    assert_eq!(
        nearest_canvas_snap([5.0, -10.0, 20.0], canvas_guides),
        Some((-5.0, 0.0))
    );
    assert_eq!(
        nearest_canvas_snap([30.0, 48.0, 60.0], canvas_guides),
        Some((2.0, 50.0))
    );
    assert_eq!(
        nearest_canvas_snap([95.0, 80.0, 110.0], canvas_guides),
        Some((5.0, 100.0))
    );
    assert_eq!(nearest_canvas_snap([20.0, 10.0, 30.0], canvas_guides), None);
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
