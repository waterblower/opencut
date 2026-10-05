use crate::clip_placement::ClipPlacementRejection;
use crate::explorer_drag::AssetBeingDragged;
use crate::layout::DEFAULT_TIMELINE_PIXELS_PER_SECOND;
use crate::model::MediaKind;
use crate::preview_text::PreviewTextDrag;
use crate::timeline_clip::Clip;
use crate::timeline_interactions::{TimelineInteractionState, TimelineTool};
use crate::track::TrackKind;
use ::timeline::TimelineEditingState;
pub use ::timeline::{FrameRate, TimelineFrameIndex};
use anyhow::{Result, ensure};
use gpui::ScrollHandle;
use gpui::prelude::*;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use ulid::Ulid;

pub(super) const FRAME_RATE_PRESETS: [(FrameRate, &str); 8] = [
    (FrameRate::new(24_000, 1_001), "23.976 fps"),
    (FrameRate::new(24, 1), "24 fps"),
    (FrameRate::new(25, 1), "25 fps"),
    (FrameRate::new(30_000, 1_001), "29.97 fps"),
    (FrameRate::new(30, 1), "30 fps"),
    (FrameRate::new(50, 1), "50 fps"),
    (FrameRate::new(60_000, 1_001), "59.94 fps"),
    (FrameRate::new(60, 1), "60 fps"),
];

pub struct TimelineRuntimeState {
    pub clip_trim_drag: Option<crate::timeline_trim::ClipTrimDrag>,
    /// Absolute path of the timeline file.
    pub path: PathBuf,
    pub editing_state: TimelineEditingState,
    pub text_drag: Option<PreviewTextDrag>,
    playhead: TimelineFrameIndex, // 编辑区自己的播放头；与预览播放器互不同步。
    pub h_scroll: ScrollHandle,
    pub v_scroll: ScrollHandle,
    pub pixels_per_second: f32,
    pub snapping_enabled: bool,
    pub track_magnet_enabled: bool,
    pub(super) interaction: TimelineInteractionState,
    pub(super) undo_stack: Vec<TimelineEditingState>,
    pub(super) redo_stack: Vec<TimelineEditingState>,
    pub(super) preview_drop_asset: Option<PreviewDropAsset>,
}

#[derive(Debug)]
pub struct PreviewDropAsset {
    pub track_id: Ulid,
    pub start_time: TimelineFrameIndex,
    pub asset: AssetBeingDragged,
}

pub fn timeline_ranges_overlap(
    left_start: TimelineFrameIndex,
    left_end: TimelineFrameIndex,
    right_start: TimelineFrameIndex,
    right_end: TimelineFrameIndex,
) -> bool {
    left_start < right_end && right_start < left_end
}
pub trait TimelineEditorExt: Sized {
    fn validate_clip_move_placements(
        &self,
        placements: &[(Ulid, Ulid, TimelineFrameIndex)],
        ignored_clip_ids: &HashSet<Ulid>,
    ) -> Result<()>;
    fn set_frame_rate(&mut self, frame_rate: FrameRate);
    fn repair_and_prune_invalid_data(&mut self);
}
impl TimelineEditorExt for TimelineEditingState {
    fn validate_clip_move_placements(
        &self,
        placements: &[(Ulid, Ulid, TimelineFrameIndex)],
        ignored_clip_ids: &HashSet<Ulid>,
    ) -> Result<()> {
        if placements.is_empty() {
            return Err(ClipPlacementRejection::NoPlacements.into());
        }
        let clips = self.clips.iter().map(|clip| (clip.id(), clip)).collect::<HashMap<_, _>>();
        let tracks = self.tracks.iter().map(|track| (track.id, track)).collect::<HashMap<_, _>>();
        let assets = self.assets.iter().map(|asset| (asset.id, asset)).collect::<HashMap<_, _>>();
        let mut proposed = Vec::with_capacity(placements.len());
        for &(clip_id, track_id, start) in placements {
            let clip = clips.get(&clip_id).ok_or(ClipPlacementRejection::MissingClip)?;
            let duration = clip.frame_length(self.settings.frame_rate);
            if start < TimelineFrameIndex::ZERO {
                return Err(ClipPlacementRejection::BeforeTimelineStart.into());
            }
            if duration < TimelineFrameIndex::ONE_FRAME {
                return Err(ClipPlacementRejection::DurationTooShort.into());
            }
            let track = tracks.get(&track_id).ok_or(ClipPlacementRejection::MissingTrack)?;
            if track.locked {
                return Err(ClipPlacementRejection::LockedTrack.into());
            }
            let expected_kind = match clip {
                Clip::Video(media) | Clip::Audio(media) => {
                    let asset = assets.get(&media.asset_id).ok_or(ClipPlacementRejection::MissingAsset)?;
                    if matches!(clip, Clip::Audio(_)) || asset.kind == MediaKind::Audio {
                        TrackKind::Audio
                    } else {
                        TrackKind::Video
                    }
                }
                Clip::Text(_) => TrackKind::Text,
            };
            if track.kind != expected_kind {
                return Err(ClipPlacementRejection::IncompatibleTrack.into());
            }
            proposed.push((track_id, start, start + duration));
        }
        proposed.sort_unstable();
        if proposed.windows(2).any(|pair| pair[0].0 == pair[1].0 && pair[1].1 < pair[0].2) {
            return Err(ClipPlacementRejection::ProposedClipsOverlap.into());
        }
        let mut existing = self.clips.iter()
            .filter(|clip| !ignored_clip_ids.contains(&clip.id()))
            .map(|clip| (clip.track_id(), clip.timeline_start(), clip.timeline_end(self.settings.frame_rate)))
            .collect::<Vec<_>>();
        existing.sort_unstable();
        let mut cursor = 0;
        for (track_id, start, end) in proposed {
            while cursor < existing.len()
                && (existing[cursor].0 < track_id
                    || (existing[cursor].0 == track_id && existing[cursor].2 <= start))
            {
                cursor += 1;
            }
            if let Some(&(other_track, other_start, other_end)) = existing.get(cursor) {
                if other_track == track_id && timeline_ranges_overlap(start, end, other_start, other_end) {
                    return Err(ClipPlacementRejection::ExistingClipOverlap.into());
                }
            }
        }
        Ok(())
    }
    fn set_frame_rate(&mut self, frame_rate: FrameRate) {
        let frame_rate = FrameRate::new(frame_rate.numerator.max(1), frame_rate.denominator.max(1));
        let previous = self.settings.frame_rate;
        if previous == frame_rate {
            return;
        }

        for clip in &mut self.clips {
            let old_start = clip.timeline_start();
            let old_end = clip.timeline_end(previous);
            let timeline_start = previous.rescale_nearest(old_start, frame_rate);
            clip.set_timeline_start(timeline_start);
            let new_duration = (previous.rescale_nearest(old_end, frame_rate) - timeline_start)
                .max(TimelineFrameIndex::ONE_FRAME);
            match clip {
                Clip::Video(clip) | Clip::Audio(clip) => {
                    clip.source_in = previous.rescale_nearest(clip.source_in, frame_rate);
                    clip.source_out = clip.source_in + new_duration;
                }
                Clip::Text(_) => {}
            }
        }
        self.settings.frame_rate = frame_rate;
        self.repair_and_prune_invalid_data();
    }
    fn repair_and_prune_invalid_data(&mut self) {
        if self.settings.frame_rate.numerator == 0 {
            self.settings.frame_rate.numerator = 30;
        }
        if self.settings.frame_rate.denominator == 0 {
            self.settings.frame_rate.denominator = 1;
        }
        self.settings.width = self.settings.width.max(2);
        self.settings.height = self.settings.height.max(2);
        self.settings.audio_sample_rate = self.settings.audio_sample_rate.max(8_000);
        let frame_rate = self.settings.frame_rate;
        self.clips.retain(|clip| {
            let track = self.tracks.iter().find(|track| track.id == clip.track_id());
            let is_invalid = track.is_none()
                || match (track.map(|track| track.kind), clip) {
                    (Some(TrackKind::Text), Clip::Text(_)) => false,
                    (Some(TrackKind::Video), Clip::Video(clip))
                    | (Some(TrackKind::Audio), Clip::Audio(clip)) => {
                        !self.assets.iter().any(|asset| asset.id == clip.asset_id)
                    }
                    (Some(_), _) => true,
                    (None, _) => true,
                }
                || clip.timeline_start() < TimelineFrameIndex::ZERO
                || match clip {
                    Clip::Video(clip) | Clip::Audio(clip) => {
                        clip.source_in < TimelineFrameIndex::ZERO
                            || clip.source_out - clip.source_in < TimelineFrameIndex::ONE_FRAME
                    }
                    Clip::Text(clip) => {
                        clip.frame_length(frame_rate) < TimelineFrameIndex::ONE_FRAME
                    }
                };
            !is_invalid
        });
        for clip in &mut self.clips {
            let Some(clip) = clip.media_mut() else {
                continue;
            };
            if let Some(asset) = self.assets.iter().find(|asset| asset.id == clip.asset_id) {
                if asset.kind == MediaKind::Image {
                    // An image has no time-based source to exhaust. Its five-second
                    // asset duration is only the initial clip length, not a maximum.
                    clip.source_in = clip.source_in.max(TimelineFrameIndex::ZERO);
                    clip.source_out = clip
                        .source_out
                        .max(clip.source_in + TimelineFrameIndex::ONE_FRAME);
                } else {
                    let asset_duration = frame_rate
                        .nearest(asset.duration)
                        .max(TimelineFrameIndex::ONE_FRAME);
                    let maximum_in = (asset_duration - TimelineFrameIndex::ONE_FRAME)
                        .max(TimelineFrameIndex::ZERO);
                    clip.source_in = clip.source_in.clamp(TimelineFrameIndex::ZERO, maximum_in);
                    clip.source_out = clip.source_out.clamp(
                        clip.source_in + TimelineFrameIndex::ONE_FRAME,
                        asset_duration,
                    );
                }
            }
        }
        for track in &self.tracks {
            let mut indices = self
                .clips
                .iter()
                .enumerate()
                .filter(|(_, clip)| clip.track_id() == track.id)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            indices.sort_by(|left, right| {
                self.clips[*left]
                    .timeline_start()
                    .cmp(&self.clips[*right].timeline_start())
                    .then_with(|| self.clips[*left].id().cmp(&self.clips[*right].id()))
            });
            let mut next_available = TimelineFrameIndex::ZERO;
            for index in indices {
                let timeline_start = self.clips[index].timeline_start().max(next_available);
                self.clips[index].set_timeline_start(timeline_start);
                next_available = self.clips[index].timeline_end(frame_rate);
            }
        }
    }
}
impl TimelineRuntimeState {
    pub(super) fn new(path: PathBuf, editing_state: TimelineEditingState) -> Result<Self> {
        ensure!(path.is_absolute(), "Timeline path must be absolute");
        editing_state.validate()?;
        let selected_clip_id = editing_state.clips.first().map(Clip::id);
        let selected_clip_ids = selected_clip_id.into_iter().collect();

        Ok(Self {
            clip_trim_drag: None,
            path,
            editing_state,
            playhead: TimelineFrameIndex::ZERO,
            h_scroll: ScrollHandle::new(),
            v_scroll: ScrollHandle::new(),
            pixels_per_second: DEFAULT_TIMELINE_PIXELS_PER_SECOND,
            snapping_enabled: true,
            track_magnet_enabled: true,
            interaction: TimelineInteractionState {
                active_tool: TimelineTool::Selection,
                selected_clip_id,
                selected_clip_ids,
                blade_guide: None,
                snap_guide: None,
                clip_move_drag: None,
                marquee_selection: None,
            },
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            preview_drop_asset: None,
            text_drag: None,
        })
    }

    pub fn playhead(&self) -> TimelineFrameIndex {
        self.playhead
    }

    /// Moves the editing playhead to `frame_index`, clamped to the last frame of the content.
    pub fn set_playhead(&mut self, frame_index: TimelineFrameIndex) {
        let last_frame_index =
            self.editing_state.content_duration() - TimelineFrameIndex::ONE_FRAME;
        if frame_index < TimelineFrameIndex::ZERO {
            self.playhead = TimelineFrameIndex::ZERO;
        } else if frame_index > last_frame_index {
            self.playhead = last_frame_index;
        } else {
            self.playhead = frame_index;
        }
    }

    pub(super) fn record_editing_history(&mut self) {
        self.undo_stack.push(self.editing_state.clone());
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }
}
pub trait FrameRateLabel {
    fn label(self) -> String;
}
impl FrameRateLabel for FrameRate {
    fn label(self) -> String {
        if let Some(label) = FRAME_RATE_PRESETS
            .iter()
            .find_map(|(candidate, label)| (*candidate == self).then_some(*label))
        {
            return label.to_string();
        }

        let frames_per_second = self.frames_per_second();
        if frames_per_second.fract().abs() < f64::EPSILON {
            format!("{frames_per_second:.0} fps")
        } else {
            format!("{frames_per_second:.2} fps")
        }
    }
}
#[cfg(test)]
#[path = "tests/timeline.test.rs"]
mod tests;
