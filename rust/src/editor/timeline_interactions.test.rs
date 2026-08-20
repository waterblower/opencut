use super::*;

#[test]
fn zoom_keeps_the_playhead_at_the_same_viewport_position() {
    let playhead_seconds = 10.0;
    let previous_zoom = 50.0;
    let new_zoom = 100.0;
    let previous_offset = -200.0;
    let previous_viewport_x = playhead_seconds as f32 * previous_zoom + previous_offset;

    let new_offset = zoom_scroll_offset(previous_offset, playhead_seconds, previous_zoom, new_zoom);
    let new_viewport_x = playhead_seconds as f32 * new_zoom + new_offset;

    assert_eq!(new_viewport_x, previous_viewport_x);
}

#[test]
fn zoom_does_not_scroll_past_the_timeline_origin() {
    assert_eq!(zoom_scroll_offset(-20.0, 10.0, 100.0, 10.0), 0.0);
}

#[test]
fn snaps_a_moving_clip_end_when_its_start_has_no_target() {
    let original_start = TimelineTime::from_frames(100);
    let unsnapped_start = original_start;
    let snapped_start_from_end = TimelineTime::from_frames(102);
    let target_edge = TimelineTime::from_frames(202);

    assert_eq!(
        choose_clip_snap(
            original_start,
            unsnapped_start,
            None,
            snapped_start_from_end,
            Some(target_edge),
        ),
        (snapped_start_from_end, Some(target_edge))
    );
}
