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

impl std::fmt::Display for ClipPlacementRejection {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

impl std::error::Error for ClipPlacementRejection {}

pub(super) fn validate_clip_placement(
    timeline: &TimelineSerialization,
    target_track_id: Ulid,
    media_kind: MediaKind,
    clip_length: TimelineTime,
    target_timeline_start: TimelineTime,
    ignored_clip_ids: &HashSet<Ulid>,
) -> anyhow::Result<()> {
    let expected_track_kind = if media_kind == MediaKind::Audio {
        TrackKind::Audio
    } else {
        TrackKind::Video
    };
    validate_clip_placement_on_track(
        timeline,
        target_track_id,
        expected_track_kind,
        clip_length,
        target_timeline_start,
        ignored_clip_ids,
    )
}

pub(super) fn validate_text_clip_placement(
    timeline: &TimelineSerialization,
    target_track_id: Ulid,
    clip_length: TimelineTime,
    target_timeline_start: TimelineTime,
    ignored_clip_ids: &HashSet<Ulid>,
) -> anyhow::Result<()> {
    validate_clip_placement_on_track(
        timeline,
        target_track_id,
        TrackKind::Text,
        clip_length,
        target_timeline_start,
        ignored_clip_ids,
    )
}

fn validate_clip_placement_on_track(
    timeline: &TimelineSerialization,
    target_track_id: Ulid,
    expected_track_kind: TrackKind,
    clip_length: TimelineTime,
    target_timeline_start: TimelineTime,
    ignored_clip_ids: &HashSet<Ulid>,
) -> anyhow::Result<()> {
    if target_timeline_start < TimelineTime::ZERO {
        return Err(ClipPlacementRejection::BeforeTimelineStart.into());
    }
    if clip_length < TimelineTime::ONE_FRAME {
        return Err(ClipPlacementRejection::DurationTooShort.into());
    }
    let Some(track) = timeline.track(target_track_id) else {
        return Err(ClipPlacementRejection::MissingTrack.into());
    };
    if track.locked {
        return Err(ClipPlacementRejection::LockedTrack.into());
    }
    if track.kind != expected_track_kind {
        return Err(ClipPlacementRejection::IncompatibleTrack.into());
    }
    if timeline.clips.iter().any(|clip| {
        !ignored_clip_ids.contains(&clip.id())
            && clip.track_id() == target_track_id
            && timeline_ranges_overlap(
                target_timeline_start,
                target_timeline_start + clip_length,
                clip.timeline_start(),
                clip.timeline_end(timeline.settings.frame_rate),
            )
    }) {
        return Err(ClipPlacementRejection::ExistingClipOverlap.into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "clip_placement.test.rs"]
mod tests;
