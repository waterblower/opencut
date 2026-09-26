use crate::editor::PreviewState;
use crate::editor::editing::validate_clips_placements;
use crate::editor::preview::PreviewTarget;
use crate::editor::timeline::{TimelineEditorExt, TimelineRuntimeState};
use anyhow::{Result, anyhow, ensure};
use opencut_player::timeline::{
    Clip, FrameRate, MediaAsset, TextClipProperties, TimelineEditingState, TimelineTime, Track,
    VideoClipProperties,
};
use std::{collections::HashSet, path::PathBuf};
use ulid::Ulid;

#[derive(Clone, Debug)]
pub enum EditAction {
    AddClips {
        clips: Vec<Clip>,
        assets: Vec<MediaAsset>,
    },
    RemoveClips {
        clip_ids: HashSet<Ulid>,
        close_track_gaps: bool,
    },
    SplitClips {
        removed_clips: HashSet<Ulid>,
        added_clips: Vec<Clip>,
    },
    UpdateClip {
        clip: Clip,
    },
    MoveClips {
        placements: Vec<(Ulid, Ulid, TimelineTime)>,
    },
    SetVideoProperties {
        clip_ids: Vec<Ulid>,
        properties: VideoClipProperties,
    },
    SetTextProperties {
        clip_id: Ulid,
        properties: TextClipProperties,
    },
    AddTrack {
        track: Track,
    },
    DeleteTrack {
        track_id: Ulid,
    },
    MoveTrack {
        index: usize,
        target: usize,
    },
    ToggleTrackVisibility {
        track_id: Ulid,
    },
    ToggleTrackMute {
        track_id: Ulid,
    },
    ToggleTrackLock {
        track_id: Ulid,
    },
    SetFrameRate {
        frame_rate: FrameRate,
    },
    UpdateAssetPaths {
        paths: Vec<(Ulid, PathBuf)>,
    },
    ReplaceTimeline {
        timeline: TimelineEditingState,
    },
}

pub fn apply_timeline_edit(
    preview: &mut PreviewState,
    timeline: &mut TimelineRuntimeState,
    action: EditAction,
) -> Result<()> {
    edit_timeline(timeline, action)?;
    preview.target = PreviewTarget::Timeline;
    Ok(())
}

pub fn edit_timeline(timeline: &mut TimelineRuntimeState, action: EditAction) -> Result<()> {
    let mut data = timeline.backend.timeline().clone();
    edit_content(&mut data, action)?;
    timeline.backend.replace_timeline(data)
}

fn edit_content(data: &mut TimelineEditingState, action: EditAction) -> Result<()> {
    match action {
        EditAction::AddClips { clips, assets } => {
            data.assets.extend(assets);
            validate_clips_placements(data, &clips)?;
            data.clips.extend(clips);
        }
        EditAction::RemoveClips {
            clip_ids,
            close_track_gaps,
        } => {
            if close_track_gaps {
                let frame_rate = data.settings.frame_rate;
                ripple_clips_after_deletion(&mut data.clips, &clip_ids, frame_rate);
            }
            data.clips.retain(|clip| !clip_ids.contains(&clip.id()));
        }
        EditAction::SplitClips {
            removed_clips,
            added_clips,
        } => {
            data.clips
                .retain(|clip| !removed_clips.contains(&clip.id()));
            validate_clips_placements(data, &added_clips)?;
            data.clips.extend(added_clips);
        }
        EditAction::MoveClips { placements } => {
            let ids = placements.iter().map(|(id, _, _)| *id).collect();
            data.validate_clip_move_placements(&placements, &ids)?;
            for (id, track_id, start) in placements {
                if let Some(clip) = data.clip_mut(id) {
                    clip.set_timeline_start(start);
                    clip.set_track_id(track_id);
                }
            }
        }
        EditAction::UpdateClip { clip } => {
            let index = data
                .clip_index(clip.id())
                .ok_or_else(|| anyhow!("The clip being updated no longer exists"))?;
            if let (Clip::Text(previous), Clip::Text(updated)) = (&data.clips[index], &clip)
                && previous.track_id == updated.track_id
                && previous.timeline_start == updated.timeline_start
                && previous.length == updated.length
            {
                data.clips[index] = clip;
                return Ok(());
            }
            data.clips.remove(index);
            validate_clips_placements(data, std::slice::from_ref(&clip))?;
            data.clips.insert(index, clip);
        }
        EditAction::SetVideoProperties {
            clip_ids,
            properties,
        } => {
            for clip in &mut data.clips {
                if clip_ids.contains(&clip.id())
                    && let Some(media) = clip.media_mut()
                {
                    media.video_properties = properties;
                }
            }
        }
        EditAction::SetTextProperties {
            clip_id,
            properties,
        } => {
            if let Some(Clip::Text(clip)) = data.clip_mut(clip_id) {
                clip.properties = properties;
            }
        }
        EditAction::AddTrack { track } => data.tracks.push(track),
        EditAction::DeleteTrack { track_id } => {
            data.tracks.retain(|track| track.id != track_id);
            data.clips.retain(|clip| clip.track_id() != track_id);
        }
        EditAction::MoveTrack { index, target } => {
            ensure!(
                index < data.tracks.len() && target < data.tracks.len(),
                "The track being moved or its destination no longer exists"
            );
            data.tracks.swap(index, target);
        }
        EditAction::ToggleTrackVisibility { track_id } => {
            if let Some(track) = data.track_mut(track_id) {
                track.visible = !track.visible;
            }
        }
        EditAction::ToggleTrackMute { track_id } => {
            if let Some(track) = data.track_mut(track_id) {
                track.muted = !track.muted;
            }
        }
        EditAction::ToggleTrackLock { track_id } => {
            if let Some(track) = data.track_mut(track_id) {
                track.locked = !track.locked;
            }
        }
        EditAction::SetFrameRate { frame_rate } => data.set_frame_rate(frame_rate),
        EditAction::UpdateAssetPaths { paths } => {
            for (asset_id, path) in paths {
                if let Some(asset) = data.assets.iter_mut().find(|asset| asset.id == asset_id) {
                    asset.path = path;
                }
            }
        }
        EditAction::ReplaceTimeline { timeline: updated } => {
            *data = updated;
        }
    }

    Ok(())
}

fn ripple_clips_after_deletion(
    clips: &mut [Clip],
    deleted_ids: &HashSet<Ulid>,
    frame_rate: FrameRate,
) {
    if deleted_ids.len() != 1 {
        return;
    }

    let deleted = clips
        .iter()
        .filter(|clip| deleted_ids.contains(&clip.id()))
        .map(|clip| {
            (
                clip.track_id(),
                clip.timeline_end(frame_rate),
                clip.frame_length(frame_rate),
            )
        })
        .collect::<Vec<_>>();

    for clip in clips
        .iter_mut()
        .filter(|clip| !deleted_ids.contains(&clip.id()))
    {
        let shift = deleted
            .iter()
            .filter(|(track_id, deleted_end, _)| {
                *track_id == clip.track_id() && *deleted_end <= clip.timeline_start()
            })
            .fold(TimelineTime::ZERO, |total, (_, _, duration)| {
                total + *duration
            });
        clip.set_timeline_start(clip.timeline_start() - shift);
    }
}

#[cfg(test)]
#[path = "tests/editing_state.test.rs"]
mod tests;
