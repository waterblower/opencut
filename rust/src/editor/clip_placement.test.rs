use super::*;
use crate::editor::timeline_clip::AudioClipProperties;

#[test]
fn validates_one_clip_placement() {
    let mut timeline = TimelineSerialization::with_test_tracks();

    assert_eq!(
        placement_rejection(validate_clip_placement(
            &timeline,
            ulid(2),
            MediaKind::Audio,
            TimelineTime::from_frames(10),
            TimelineTime::from_frames(-1),
            &HashSet::new(),
        )),
        ClipPlacementRejection::BeforeTimelineStart
    );
    assert_eq!(
        placement_rejection(validate_clip_placement(
            &timeline,
            ulid(2),
            MediaKind::Audio,
            TimelineTime::ZERO,
            TimelineTime::ZERO,
            &HashSet::new(),
        )),
        ClipPlacementRejection::DurationTooShort
    );
    assert_eq!(
        placement_rejection(validate_clip_placement(
            &timeline,
            ulid(99),
            MediaKind::Audio,
            TimelineTime::from_frames(10),
            TimelineTime::ZERO,
            &HashSet::new(),
        )),
        ClipPlacementRejection::MissingTrack
    );

    timeline.track_mut(ulid(2)).unwrap().locked = true;
    assert_eq!(
        placement_rejection(validate_clip_placement(
            &timeline,
            ulid(2),
            MediaKind::Audio,
            TimelineTime::from_frames(10),
            TimelineTime::ZERO,
            &HashSet::new(),
        )),
        ClipPlacementRejection::LockedTrack
    );
    timeline.track_mut(ulid(2)).unwrap().locked = false;
    assert_eq!(
        placement_rejection(validate_clip_placement(
            &timeline,
            ulid(2),
            MediaKind::Video,
            TimelineTime::from_frames(10),
            TimelineTime::ZERO,
            &HashSet::new(),
        )),
        ClipPlacementRejection::IncompatibleTrack
    );

    timeline.clips.push(Clip::Audio(AudioClip {
        id: ulid(20),
        track_id: ulid(2),
        asset_id: ulid(100),
        timeline_start: TimelineTime::from_frames(10),
        source_in: TimelineTime::ZERO,
        source_out: TimelineTime::from_frames(10),
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    }));
    assert_eq!(
        placement_rejection(validate_clip_placement(
            &timeline,
            ulid(2),
            MediaKind::Audio,
            TimelineTime::from_frames(10),
            TimelineTime::from_frames(15),
            &HashSet::new(),
        )),
        ClipPlacementRejection::ExistingClipOverlap
    );
    assert!(
        validate_clip_placement(
            &timeline,
            ulid(2),
            MediaKind::Audio,
            TimelineTime::from_frames(10),
            TimelineTime::from_frames(15),
            &HashSet::from([ulid(20)]),
        )
        .is_ok()
    );
}

fn placement_rejection(result: anyhow::Result<()>) -> ClipPlacementRejection {
    result
        .unwrap_err()
        .downcast::<ClipPlacementRejection>()
        .unwrap()
}
