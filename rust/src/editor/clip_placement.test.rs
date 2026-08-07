use super::*;

#[test]
fn validates_one_clip_placement() {
    let mut project = Project::with_test_tracks();

    assert_eq!(
        validate_clip_placement(
            &project,
            2,
            MediaKind::Audio,
            TimelineTime::from_frames(10),
            TimelineTime::from_frames(-1),
            &HashSet::new(),
        ),
        Err(ClipPlacementRejection::BeforeTimelineStart)
    );
    assert_eq!(
        validate_clip_placement(
            &project,
            2,
            MediaKind::Audio,
            TimelineTime::ZERO,
            TimelineTime::ZERO,
            &HashSet::new(),
        ),
        Err(ClipPlacementRejection::DurationTooShort)
    );
    assert_eq!(
        validate_clip_placement(
            &project,
            99,
            MediaKind::Audio,
            TimelineTime::from_frames(10),
            TimelineTime::ZERO,
            &HashSet::new(),
        ),
        Err(ClipPlacementRejection::MissingTrack)
    );

    project.track_mut(2).unwrap().locked = true;
    assert_eq!(
        validate_clip_placement(
            &project,
            2,
            MediaKind::Audio,
            TimelineTime::from_frames(10),
            TimelineTime::ZERO,
            &HashSet::new(),
        ),
        Err(ClipPlacementRejection::LockedTrack)
    );
    project.track_mut(2).unwrap().locked = false;
    assert_eq!(
        validate_clip_placement(
            &project,
            2,
            MediaKind::Video,
            TimelineTime::from_frames(10),
            TimelineTime::ZERO,
            &HashSet::new(),
        ),
        Err(ClipPlacementRejection::IncompatibleTrack)
    );

    project.clips.push(TimelineClip {
        id: 20,
        track_id: 2,
        asset_id: 100,
        timeline_start: TimelineTime::from_frames(10),
        source_in: TimelineTime::ZERO,
        source_out: TimelineTime::from_frames(10),
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    });
    assert_eq!(
        validate_clip_placement(
            &project,
            2,
            MediaKind::Audio,
            TimelineTime::from_frames(10),
            TimelineTime::from_frames(15),
            &HashSet::new(),
        ),
        Err(ClipPlacementRejection::ExistingClipOverlap)
    );
    assert_eq!(
        validate_clip_placement(
            &project,
            2,
            MediaKind::Audio,
            TimelineTime::from_frames(10),
            TimelineTime::from_frames(15),
            &HashSet::from([20]),
        ),
        Ok(())
    );
}
