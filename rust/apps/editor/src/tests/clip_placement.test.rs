use super::*;
use crate::model::MediaKind;
use crate::test_support::{TimelineTestExt, ulid};
use crate::timeline::TimelineFrameIndex;
use crate::timeline_clip::{AudioClip, AudioClipProperties, Clip, VideoClipProperties};
use ::timeline::TimelineEditingState;
use anyhow::Result;
use std::collections::HashSet;

#[test]
fn validates_one_clip_placement() {
    let mut timeline = TimelineEditingState::with_test_tracks();

    assert_eq!(
        placement_rejection(validate_clip_placement(
            &timeline,
            ulid(2),
            MediaKind::Audio,
            TimelineFrameIndex::from(10),
            TimelineFrameIndex::from(-1),
            &HashSet::new(),
        )),
        ClipPlacementRejection::BeforeTimelineStart
    );
    assert_eq!(
        placement_rejection(validate_clip_placement(
            &timeline,
            ulid(2),
            MediaKind::Audio,
            TimelineFrameIndex::ZERO,
            TimelineFrameIndex::ZERO,
            &HashSet::new(),
        )),
        ClipPlacementRejection::DurationTooShort
    );
    assert_eq!(
        placement_rejection(validate_clip_placement(
            &timeline,
            ulid(99),
            MediaKind::Audio,
            TimelineFrameIndex::from(10),
            TimelineFrameIndex::ZERO,
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
            TimelineFrameIndex::from(10),
            TimelineFrameIndex::ZERO,
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
            TimelineFrameIndex::from(10),
            TimelineFrameIndex::ZERO,
            &HashSet::new(),
        )),
        ClipPlacementRejection::IncompatibleTrack
    );

    timeline.clips.push(Clip::Audio(AudioClip::new(
        ulid(20),
        ulid(2),
        ulid(100),
        TimelineFrameIndex::from(10),
        TimelineFrameIndex::ZERO,
        TimelineFrameIndex::from(10),
        AudioClipProperties::default(),
    )));
    assert_eq!(
        placement_rejection(validate_clip_placement(
            &timeline,
            ulid(2),
            MediaKind::Audio,
            TimelineFrameIndex::from(10),
            TimelineFrameIndex::from(15),
            &HashSet::new(),
        )),
        ClipPlacementRejection::ExistingClipOverlap
    );
    assert!(
        validate_clip_placement(
            &timeline,
            ulid(2),
            MediaKind::Audio,
            TimelineFrameIndex::from(10),
            TimelineFrameIndex::from(15),
            &HashSet::from([ulid(20)]),
        )
        .is_ok()
    );
}

fn placement_rejection(result: Result<()>) -> ClipPlacementRejection {
    result
        .unwrap_err()
        .downcast::<ClipPlacementRejection>()
        .unwrap()
}
