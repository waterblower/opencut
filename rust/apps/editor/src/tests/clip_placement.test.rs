use super::*;
use crate::editor::tests::TimelineTestExt;
use crate::editor::timeline_clip::AudioClipProperties;
use ::timeline::TimelineEditingState;
use anyhow::Result;

#[test]
fn validates_one_clip_placement() {
    let mut timeline = TimelineEditingState::with_test_tracks();

    assert_eq!(
        placement_rejection(validate_clip_placement(
            &timeline,
            ulid(2),
            MediaKind::Audio,
            TimelineFrame::from_frames(10),
            TimelineFrame::from_frames(-1),
            &HashSet::new(),
        )),
        ClipPlacementRejection::BeforeTimelineStart
    );
    assert_eq!(
        placement_rejection(validate_clip_placement(
            &timeline,
            ulid(2),
            MediaKind::Audio,
            TimelineFrame::ZERO,
            TimelineFrame::ZERO,
            &HashSet::new(),
        )),
        ClipPlacementRejection::DurationTooShort
    );
    assert_eq!(
        placement_rejection(validate_clip_placement(
            &timeline,
            ulid(99),
            MediaKind::Audio,
            TimelineFrame::from_frames(10),
            TimelineFrame::ZERO,
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
            TimelineFrame::from_frames(10),
            TimelineFrame::ZERO,
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
            TimelineFrame::from_frames(10),
            TimelineFrame::ZERO,
            &HashSet::new(),
        )),
        ClipPlacementRejection::IncompatibleTrack
    );

    timeline.clips.push(Clip::Audio(AudioClip {
        id: ulid(20),
        track_id: ulid(2),
        asset_id: ulid(100),
        timeline_start: TimelineFrame::from_frames(10),
        source_in: TimelineFrame::ZERO,
        source_out: TimelineFrame::from_frames(10),
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties::default(),
    }));
    assert_eq!(
        placement_rejection(validate_clip_placement(
            &timeline,
            ulid(2),
            MediaKind::Audio,
            TimelineFrame::from_frames(10),
            TimelineFrame::from_frames(15),
            &HashSet::new(),
        )),
        ClipPlacementRejection::ExistingClipOverlap
    );
    assert!(
        validate_clip_placement(
            &timeline,
            ulid(2),
            MediaKind::Audio,
            TimelineFrame::from_frames(10),
            TimelineFrame::from_frames(15),
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
