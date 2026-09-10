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
