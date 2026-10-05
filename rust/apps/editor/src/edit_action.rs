use crate::editing::validate_clips_placements;
use crate::timeline::{TimelineEditorExt, TimelineRuntimeState};
use ::timeline::{
    Clip, FrameRate, MediaAsset, TextClipProperties, TimelineEditingState, TimelineFrameIndex,
    Track, VideoClipProperties,
};
use anyhow::{Result, anyhow, ensure};
use gpui::{Pixels, Point};
use std::collections::HashSet;
use std::path::PathBuf;
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
        placements: Vec<(Ulid, Ulid, TimelineFrameIndex)>,
    },
    SetVideoProperties {
        clip_ids: Vec<Ulid>,
        properties: VideoClipProperties,
    },
    SetTextContent {
        clip_id: Ulid,
        text: String,
    },
    UpdateTextClipPosition {
        timeline_path: PathBuf,
        clip_id: Ulid,
        pointer: Point<Pixels>,
        finished: bool, // 松开时在同一事件内应用最终坐标并保存，避免提前清除拖动状态。
    },
    ApplyTextStyleToTrack {
        clip_id: Ulid,
        project_root: PathBuf,
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

/// Applies an edit and returns whether the caller should save the timeline.
/// The preview player is not synced with it.
pub fn edit_timeline(timeline: &mut TimelineRuntimeState, action: EditAction) -> Result<bool> {
    let mut data = timeline.editing_state.clone();
    let mut record_history = true;
    match action {
        EditAction::UpdateTextClipPosition {
            timeline_path,
            clip_id,
            pointer,
            finished,
        } => {
            let Some(drag) = timeline.text_drag.as_ref() else {
                return Ok(false);
            };
            if drag.timeline_path != timeline_path
                || drag.clip_id != clip_id
                || timeline.path != timeline_path
            {
                return Ok(false);
            }
            let Some(Clip::Text(clip)) = timeline.editing_state.clip(clip_id) else {
                return Ok(false);
            };
            if !timeline
                .editing_state
                .tracks
                .iter()
                .any(|track| track.id == clip.track_id && !track.locked)
            {
                return Ok(false);
            }
            let mut position_x = (drag.original.x
                + f64::from(
                    f32::from(pointer.x - drag.start.x) / f32::from(drag.canvas_size.width),
                ))
            .clamp(0.0, 1.0);
            let mut position_y = (drag.original.y
                + f64::from(
                    f32::from(pointer.y - drag.start.y) / f32::from(drag.canvas_size.height),
                ))
            .clamp(0.0, 1.0);
            if timeline.snapping_enabled {
                for (position, canvas_extent, text_extent) in [
                    (
                        &mut position_x,
                        drag.canvas_size.width,
                        drag.text_size.width,
                    ),
                    (
                        &mut position_y,
                        drag.canvas_size.height,
                        drag.text_size.height,
                    ),
                ] {
                    let extent = f64::from(f32::from(canvas_extent));
                    let half_text = f64::from(f32::from(text_extent)) / (2.0 * extent);
                    let mut nearest_distance = 8.0 / extent; // 屏幕逻辑像素阈值，不随画布缩放变化。
                    let unsnapped = *position;
                    for target in [0.5, half_text, 1.0 - half_text] {
                        if !(0.0..=1.0).contains(&target) {
                            continue;
                        }
                        let distance = (unsnapped - target).abs();
                        if distance < nearest_distance {
                            nearest_distance = distance;
                            *position = target;
                        }
                    }
                }
            }
            if (clip.properties.position.x, clip.properties.position.y) != (position_x, position_y)
            {
                if !drag.history_recorded {
                    timeline.record_editing_history();
                    timeline.text_drag.as_mut().unwrap().history_recorded = true;
                }
                if let Some(Clip::Text(clip)) = timeline.editing_state.clip_mut(clip_id) {
                    clip.properties.position.x = position_x;
                    clip.properties.position.y = position_y;
                }
            }
            if finished {
                let finished_drag = timeline.text_drag.take().unwrap();
                return Ok(finished_drag.history_recorded);
            }
            return Ok(false);
        }
        EditAction::AddClips { clips, assets } => {
            data.assets.extend(assets);
            validate_clips_placements(&data, &clips)?;
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
            validate_clips_placements(&data, &added_clips)?;
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
                && previous.duration == updated.duration
            {
                data.clips[index] = clip;
            } else {
                data.clips.remove(index);
                validate_clips_placements(&data, std::slice::from_ref(&clip))?;
                data.clips.insert(index, clip);
            }
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
        EditAction::SetTextContent { clip_id, text } => {
            if let Some(Clip::Text(clip)) = data.clip_mut(clip_id) {
                clip.properties.text = text;
            }
        }
        EditAction::ApplyTextStyleToTrack {
            clip_id,
            project_root,
        } => {
            let Some(Clip::Text(source)) = data.clip(clip_id) else {
                let relative_path = timeline.path.strip_prefix(&project_root)?;
                return Err(anyhow!(
                    "Source text clip {clip_id} not found in timeline {}",
                    relative_path.display()
                ));
            };
            let track_id = source.track_id;
            let properties = source.properties.clone();
            let mut changed = false;
            for clip in &mut data.clips {
                let Clip::Text(target) = clip else {
                    continue;
                };
                if target.track_id != track_id || target.id == clip_id {
                    continue;
                }
                let mut updated = properties.clone();
                updated.text = target.properties.text.clone();
                if target.properties != updated {
                    target.properties = updated;
                    changed = true;
                }
            }
            if !changed {
                return Ok(false);
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
            record_history = false; // 撤销/重做由调用方维护历史栈。
            data = updated;
        }
    }

    data.validate()?;
    if record_history {
        timeline.record_editing_history();
    }
    timeline.editing_state = data;
    timeline.set_playhead(timeline.playhead());
    Ok(true)
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
            .fold(TimelineFrameIndex::ZERO, |total, (_, _, duration)| {
                total + *duration
            });
        clip.set_timeline_start(clip.timeline_start() - shift);
    }
}

#[cfg(test)]
#[path = "tests/editing_state.test.rs"]
mod tests;
