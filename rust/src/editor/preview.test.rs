use super::*;

#[test]
fn timeline_playhead_clock_crosses_clip_boundaries_without_a_position_query() {
    let position = timeline_playhead_from_elapsed(
        FrameRate::new(24, 1),
        TimelineTime::from_frames(11),
        Duration::from_millis(100),
    );

    assert_eq!(position, TimelineTime::from_frames(13));
}

#[test]
fn timeline_playhead_clock_stays_at_its_anchor_without_elapsed_time() {
    let origin = TimelineTime::from_frames(20);

    assert_eq!(
        timeline_playhead_from_elapsed(FrameRate::default(), origin, Duration::ZERO),
        origin
    );
}
