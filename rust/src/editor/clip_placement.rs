use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ClipPlacementRejection {
    NoPlacements,
    BeforeTimelineStart,
    DurationTooShort,
    MissingClip,
    MissingTrack,
    LockedTrack,
    MissingAsset,
    IncompatibleTrack,
    ProposedClipsOverlap,
    ExistingClipOverlap,
}

impl ClipPlacementRejection {
    pub fn message(self) -> &'static str {
        match self {
            Self::NoPlacements => "No clips were provided",
            Self::BeforeTimelineStart => "Placement starts before the timeline",
            Self::DurationTooShort => "Clip duration is too short",
            Self::MissingClip => "Clip is unavailable",
            Self::MissingTrack => "Destination track is unavailable",
            Self::LockedTrack => "Destination track is locked",
            Self::MissingAsset => "Source media is unavailable",
            Self::IncompatibleTrack => "Media is incompatible with the destination track",
            Self::ProposedClipsOverlap => "Proposed clips overlap each other",
            Self::ExistingClipOverlap => "Placement overlaps an existing clip",
        }
    }
}

pub(super) fn validate_clip_placement(
    project: &Project,
    target_track_id: u64,
    media_kind: MediaKind,
    clip_length: TimelineTime,
    target_timeline_start: TimelineTime,
    ignored_clip_ids: &HashSet<u64>,
) -> Result<(), ClipPlacementRejection> {
    if target_timeline_start < TimelineTime::ZERO {
        return Err(ClipPlacementRejection::BeforeTimelineStart);
    }
    if clip_length < TimelineTime::ONE_FRAME {
        return Err(ClipPlacementRejection::DurationTooShort);
    }
    let Some(track) = project.track(target_track_id) else {
        return Err(ClipPlacementRejection::MissingTrack);
    };
    if track.locked {
        return Err(ClipPlacementRejection::LockedTrack);
    }
    let compatible = match track.kind {
        TrackKind::Video => media_kind != MediaKind::Audio,
        TrackKind::Audio => media_kind == MediaKind::Audio,
    };
    if !compatible {
        return Err(ClipPlacementRejection::IncompatibleTrack);
    }
    if project.clips.iter().any(|clip| {
        !ignored_clip_ids.contains(&clip.id)
            && clip.track_id == target_track_id
            && timeline_ranges_overlap(
                target_timeline_start,
                target_timeline_start + clip_length,
                clip.timeline_start,
                clip.timeline_end(),
            )
    }) {
        return Err(ClipPlacementRejection::ExistingClipOverlap);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
}
