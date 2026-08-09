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
    timeline: &Timeline,
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
    let Some(track) = timeline.track(target_track_id) else {
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
    if timeline.clips.iter().any(|clip| {
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
#[path = "clip_placement.test.rs"]
mod tests;
