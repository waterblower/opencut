use super::*;

#[derive(Clone)]
pub(super) struct ClipClipboard {
    clips: Vec<TimelineClip>,
    selection_start: TimelineTime,
    primary_index: Option<usize>,
}

impl ClipClipboard {
    fn from_selection(
        project: &Project,
        selected_clip_ids: &HashSet<u64>,
        primary_clip_id: Option<u64>,
    ) -> Option<Self> {
        let clips = project
            .clips
            .iter()
            .filter(|clip| selected_clip_ids.contains(&clip.id))
            .cloned()
            .collect::<Vec<_>>();
        if clips.is_empty() || clips.len() != selected_clip_ids.len() {
            return None;
        }
        let selection_start = clips
            .iter()
            .map(|clip| clip.timeline_start)
            .min()
            .unwrap_or(TimelineTime::ZERO);
        let primary_index =
            primary_clip_id.and_then(|clip_id| clips.iter().position(|clip| clip.id == clip_id));
        Some(Self {
            clips,
            selection_start,
            primary_index,
        })
    }

    fn clips_at(&self, position: TimelineTime) -> Vec<TimelineClip> {
        self.clips
            .iter()
            .cloned()
            .map(|mut clip| {
                clip.timeline_start = position + clip.timeline_start - self.selection_start;
                clip
            })
            .collect()
    }
}

impl Editor {
    pub(super) fn append_asset_clip(&mut self, asset_id: u64) {
        let Some(asset) = self.project.asset(asset_id).cloned() else {
            return;
        };
        let track_id = match self.find_append_track_for_asset(&asset) {
            Ok(track_id) => track_id,
            Err(error) => {
                self.status = None;
                self.error = Some(error);
                return;
            }
        };
        self.checkpoint();
        self.append_asset_clip_without_checkpoint(asset_id, track_id, asset.duration);
        self.save_project();
    }

    pub(super) fn find_append_track_for_asset(&self, asset: &MediaAsset) -> Result<u64, String> {
        find_append_track(&self.project, asset)
    }

    pub(super) fn append_asset_clip_without_checkpoint(
        &mut self,
        asset_id: u64,
        track_id: u64,
        duration: f64,
    ) {
        let id = self.take_id();
        self.project.clips.push(TimelineClip {
            id,
            track_id,
            asset_id,
            timeline_start: self.project.content_duration(),
            source_in: TimelineTime::ZERO,
            source_out: self.project.ceil_time(duration),
            video_properties: VideoClipProperties::default(),
            audio_properties: AudioClipProperties::default(),
        });
        self.selected_asset_id = Some(asset_id);
        self.select_only_clip(Some(id));
        if self.video.is_none() {
            self.load_timeline_position(TimelineTime::ZERO, false);
        }
    }

    pub(super) fn blade_at_playhead(&mut self) {
        let clips = clips_crossing_playhead(&self.project, self.playhead);
        if clips.is_empty() {
            self.error = Some("No unlocked clip crosses the playhead.".into());
            return;
        }

        self.checkpoint();
        let mut right_halves = Vec::with_capacity(clips.len());
        for clip in clips {
            let Some(index) = self.project.clip_index(clip.id) else {
                continue;
            };
            let new_id = self.take_id();
            let Some((left, right)) = clip.split_at(self.playhead, new_id) else {
                continue;
            };
            self.project.clips[index] = left;
            right_halves.push(right);
        }
        self.selected_clip_ids = right_halves.iter().map(|clip| clip.id).collect();
        self.selected_clip_id = right_halves.first().map(|clip| clip.id);
        let split_count = right_halves.len();
        self.project.clips.extend(right_halves);
        self.error = None;
        self.status = Some(format!(
            "Bladed {split_count} clip{} at the playhead.",
            plural_suffix(split_count)
        ));
        self.save_project();
        self.load_timeline_position(self.playhead, false);
    }

    pub(super) fn blade_split_clip_at(&mut self, clip_id: u64, position: TimelineTime) {
        let Some(index) = self.project.clip_index(clip_id) else {
            return;
        };
        if self.clip_locked(clip_id) {
            return;
        }
        let clip = self.project.clips[index].clone();
        let right_clip_id = self.take_id();
        let Some((left, right)) = clip.split_at(position, right_clip_id) else {
            return;
        };

        self.checkpoint();
        self.project.clips[index] = left;
        self.project.clips.push(right);
        self.select_only_clip(Some(right_clip_id));
        self.error = None;
        self.save_project();
        self.load_timeline_position(self.playhead, false);
    }

    pub(super) fn delete_selected(&mut self) {
        if self.selected_clip_ids.is_empty() || !self.selected_clips_editable() {
            return;
        }
        let clip_ids = self.selected_clip_ids.clone();
        let clip_count = clip_ids.len();
        self.checkpoint();
        self.remove_clips(&clip_ids, self.track_magnet_enabled);
        self.status = Some(if self.track_magnet_enabled {
            format!(
                "Deleted {clip_count} clip{} and closed the track gap{}.",
                plural_suffix(clip_count),
                plural_suffix(clip_count)
            )
        } else {
            format!("Deleted {clip_count} clip{}.", plural_suffix(clip_count))
        });
    }

    pub(super) fn copy_selected_clips(&mut self) {
        let Some(clipboard) = ClipClipboard::from_selection(
            &self.project,
            &self.selected_clip_ids,
            self.selected_clip_id,
        ) else {
            return;
        };
        let count = clipboard.clips.len();
        self.clip_clipboard = Some(clipboard);
        self.error = None;
        self.status = Some(format!("Copied {count} clip{}.", plural_suffix(count)));
    }

    pub(super) fn cut_selected_clips(&mut self) {
        if self.selected_clip_ids.is_empty() {
            return;
        }
        if !self.selected_clips_editable() {
            self.error = Some("Cannot cut clips from a locked track.".to_string());
            return;
        }
        let Some(clipboard) = ClipClipboard::from_selection(
            &self.project,
            &self.selected_clip_ids,
            self.selected_clip_id,
        ) else {
            return;
        };
        let count = clipboard.clips.len();
        let clip_ids = self.selected_clip_ids.clone();
        self.checkpoint();
        self.clip_clipboard = Some(clipboard);
        self.remove_clips(&clip_ids, false);
        self.error = None;
        self.status = Some(format!("Cut {count} clip{}.", plural_suffix(count)));
    }

    pub(super) fn paste_clips(&mut self) {
        let Some(clipboard) = self.clip_clipboard.clone() else {
            return;
        };
        let mut clips = clipboard.clips_at(self.playhead);
        if let Err(rejection) = validate_clipboard_placements(&self.project, &clips) {
            self.error = Some(format!("Cannot paste clips: {}.", rejection.message()));
            return;
        }

        self.checkpoint();
        for clip in &mut clips {
            clip.id = self.take_id();
        }
        let count = clips.len();
        self.selected_clip_ids = clips.iter().map(|clip| clip.id).collect();
        self.selected_clip_id = clipboard
            .primary_index
            .and_then(|index| clips.get(index))
            .or_else(|| clips.first())
            .map(|clip| clip.id);
        self.project.clips.extend(clips);
        self.preview_target = PreviewTarget::Timeline;
        self.error = None;
        self.status = Some(format!("Pasted {count} clip{}.", plural_suffix(count)));
        self.save_project();
        self.load_timeline_position(self.playhead, false);
    }

    fn remove_clips(&mut self, clip_ids: &HashSet<u64>, close_track_gaps: bool) {
        if close_track_gaps {
            ripple_clips_after_deletion(&mut self.project.clips, clip_ids);
        }
        self.project
            .clips
            .retain(|clip| !clip_ids.contains(&clip.id));
        self.select_only_clip(None);
        self.preview_target = PreviewTarget::Timeline;
        self.video = None;
        self.standalone_audio = None;
        self.timeline_preview_needs_rebuild = true;
        self.playing = false;
        self.timeline_playback_clock = None;
        if self.project.clips.is_empty() {
            self.playhead = TimelineTime::ZERO;
        } else {
            self.load_timeline_position(self.playhead, false);
        }
        self.save_project();
    }

    pub(super) fn duplicate_selected(&mut self) {
        let clip_ids = self.selected_clip_ids_in_project_order();
        if clip_ids.is_empty() || !self.selected_clips_editable() {
            return;
        }
        let clips = clip_ids
            .iter()
            .filter_map(|clip_id| self.project.clip(*clip_id).cloned())
            .collect::<Vec<_>>();
        if clips.len() != clip_ids.len() {
            return;
        }
        let selection_start = clips
            .iter()
            .map(|clip| clip.timeline_start)
            .min()
            .unwrap_or(TimelineTime::ZERO);
        let selection_end = clips
            .iter()
            .map(TimelineClip::timeline_end)
            .max()
            .unwrap_or(selection_start);
        let mut delta = selection_end - selection_start;
        let placements = loop {
            let candidate = clips
                .iter()
                .map(|clip| (clip.id, clip.track_id, clip.timeline_start + delta))
                .collect::<Vec<_>>();
            if self.clip_placements_fit(&candidate, &HashSet::new()) {
                break candidate;
            }
            let mut next_delta = delta + TimelineTime::ONE_FRAME;
            for (clip, (_, track_id, start)) in clips.iter().zip(&candidate) {
                for other in self
                    .project
                    .clips
                    .iter()
                    .filter(|other| other.track_id == *track_id)
                {
                    if timeline_ranges_overlap(
                        *start,
                        *start + clip.duration(),
                        other.timeline_start,
                        other.timeline_end(),
                    ) {
                        next_delta = next_delta.max(other.timeline_end() - clip.timeline_start);
                    }
                }
            }
            delta = next_delta;
        };

        self.checkpoint();
        let primary_index = self
            .selected_clip_id
            .and_then(|id| clips.iter().position(|clip| clip.id == id));
        let mut duplicates = Vec::with_capacity(clips.len());
        for (mut clip, (_, _, start)) in clips.into_iter().zip(placements) {
            clip.id = self.take_id();
            clip.timeline_start = start;
            duplicates.push(clip);
        }
        self.selected_clip_ids = duplicates.iter().map(|clip| clip.id).collect();
        self.selected_clip_id = primary_index
            .and_then(|index| duplicates.get(index))
            .or_else(|| duplicates.first())
            .map(|clip| clip.id);
        self.project.clips.extend(duplicates);
        self.save_project();
    }

    pub(super) fn add_track(&mut self, kind: TrackKind) {
        self.checkpoint();
        let number = self
            .project
            .tracks
            .iter()
            .filter(|track| track.kind == kind)
            .count()
            + 1;
        let prefix = match kind {
            TrackKind::Video => "Video",
            TrackKind::Audio => "Audio",
        };
        let id = self.take_id();
        self.project.tracks.push(TimelineTrack {
            id,
            name: format!("{prefix} {number}"),
            kind,
            locked: false,
            muted: false,
            visible: true,
        });
        self.save_project();
    }

    pub(super) fn toggle_track_lock(&mut self, track_id: u64) {
        self.checkpoint();
        if let Some(track) = self.project.track_mut(track_id) {
            track.locked = !track.locked;
        }
        self.save_project();
    }

    pub(super) fn toggle_track_visibility(&mut self, track_id: u64) {
        self.checkpoint();
        if let Some(track) = self.project.track_mut(track_id) {
            track.visible = !track.visible;
        }
        self.save_project();
        self.load_timeline_position(self.playhead, false);
    }

    pub(super) fn toggle_track_mute(&mut self, track_id: u64) {
        self.checkpoint();
        if let Some(track) = self.project.track_mut(track_id) {
            track.muted = !track.muted;
        }
        self.save_project();
        self.load_timeline_position(self.playhead, self.playing);
    }

    pub(super) fn move_track(&mut self, track_id: u64, direction: i8) {
        let Some(index) = self
            .project
            .tracks
            .iter()
            .position(|track| track.id == track_id)
        else {
            return;
        };
        let target = if direction < 0 {
            index.checked_sub(1)
        } else if index + 1 < self.project.tracks.len() {
            Some(index + 1)
        } else {
            None
        };
        let Some(target) = target else {
            return;
        };
        self.checkpoint();
        self.project.tracks.swap(index, target);
        self.save_project();
        self.load_timeline_position(self.playhead, false);
    }

    pub(super) fn delete_track(&mut self, track_id: u64) {
        let Some(index) = self
            .project
            .tracks
            .iter()
            .position(|track| track.id == track_id)
        else {
            return;
        };
        if self.project.tracks[index].locked {
            return;
        }
        self.checkpoint();
        self.project.tracks.remove(index);
        self.project.clips.retain(|clip| clip.track_id != track_id);
        self.selected_clip_ids
            .retain(|id| self.project.clip(*id).is_some());
        if self
            .selected_clip_id
            .is_some_and(|id| self.project.clip(id).is_none())
        {
            self.selected_clip_id = self
                .project
                .clips
                .iter()
                .find(|clip| self.selected_clip_ids.contains(&clip.id))
                .map(|clip| clip.id);
        }
        self.save_project();
        self.load_timeline_position(self.playhead, false);
    }

    pub(super) fn select_only_clip(&mut self, clip_id: Option<u64>) {
        self.selected_clip_ids.clear();
        if let Some(clip_id) = clip_id {
            self.selected_clip_ids.insert(clip_id);
        }
        self.selected_clip_id = clip_id;
        self.video_transform_input_clip_id = None;
    }

    pub(super) fn select_all_unlocked_clips(&mut self) {
        self.selected_clip_ids = unlocked_clip_ids(&self.project);
        self.selected_clip_id = self
            .project
            .clips
            .iter()
            .find(|clip| self.selected_clip_ids.contains(&clip.id))
            .map(|clip| clip.id);
        self.video_transform_input_clip_id = None;
    }

    pub(super) fn toggle_clip_selection(&mut self, clip_id: u64) {
        if self.selected_clip_ids.remove(&clip_id) {
            if self.selected_clip_id == Some(clip_id) {
                self.selected_clip_id = self
                    .project
                    .clips
                    .iter()
                    .find(|clip| self.selected_clip_ids.contains(&clip.id))
                    .map(|clip| clip.id);
            }
        } else if self.project.clip(clip_id).is_some() {
            self.selected_clip_ids.insert(clip_id);
            self.selected_clip_id = Some(clip_id);
        }
        self.video_transform_input_clip_id = None;
    }

    pub(super) fn selected_clip_ids_in_project_order(&self) -> Vec<u64> {
        self.project
            .clips
            .iter()
            .filter(|clip| self.selected_clip_ids.contains(&clip.id))
            .map(|clip| clip.id)
            .collect()
    }

    pub(super) fn selected_clips_editable(&self) -> bool {
        !self.selected_clip_ids.is_empty()
            && self
                .selected_clip_ids
                .iter()
                .all(|clip_id| self.project.clip(*clip_id).is_some() && !self.clip_locked(*clip_id))
    }

    pub(super) fn clip_placements_fit(
        &self,
        placements: &[(u64, u64, TimelineTime)],
        ignored_clip_ids: &HashSet<u64>,
    ) -> bool {
        self.validate_clip_move_placements(placements, ignored_clip_ids)
            .is_ok()
    }

    pub(super) fn validate_clip_move_placements(
        &self,
        placements: &[(u64, u64, TimelineTime)],
        ignored_clip_ids: &HashSet<u64>,
    ) -> Result<(), ClipPlacementRejection> {
        if placements.is_empty() {
            return Err(ClipPlacementRejection::NoPlacements);
        }
        for (clip_id, track_id, start) in placements {
            let Some(clip) = self.project.clip(*clip_id) else {
                return Err(ClipPlacementRejection::MissingClip);
            };
            let Some(asset) = self.project.asset(clip.asset_id) else {
                return Err(ClipPlacementRejection::MissingAsset);
            };
            validate_clip_placement(
                &self.project,
                *track_id,
                asset.kind,
                clip.duration(),
                *start,
                ignored_clip_ids,
            )?;
        }
        for (index, (clip_id, track_id, start)) in placements.iter().enumerate() {
            let duration = self
                .project
                .clip(*clip_id)
                .map(TimelineClip::duration)
                .ok_or(ClipPlacementRejection::MissingClip)?;
            if placements[index + 1..]
                .iter()
                .any(|(other_id, other_track_id, other_start)| {
                    let other_duration = self
                        .project
                        .clip(*other_id)
                        .map(TimelineClip::duration)
                        .unwrap_or(TimelineTime::ZERO);
                    track_id == other_track_id
                        && timeline_ranges_overlap(
                            *start,
                            *start + duration,
                            *other_start,
                            *other_start + other_duration,
                        )
                })
            {
                return Err(ClipPlacementRejection::ProposedClipsOverlap);
            }
        }
        Ok(())
    }

    pub(super) fn checkpoint(&mut self) {
        self.undo_stack.push(self.project.clone());
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
        self.timeline_preview_needs_rebuild = true;
    }

    pub(super) fn undo(&mut self) {
        let Some(project) = self.undo_stack.pop() else {
            return;
        };
        self.redo_stack
            .push(std::mem::replace(&mut self.project, project));
        self.reset_after_history_change();
    }

    pub(super) fn redo(&mut self) {
        let Some(project) = self.redo_stack.pop() else {
            return;
        };
        self.undo_stack
            .push(std::mem::replace(&mut self.project, project));
        self.reset_after_history_change();
    }

    pub(super) fn reset_after_history_change(&mut self) {
        self.preview_target = PreviewTarget::Timeline;
        self.video = None;
        self.standalone_audio = None;
        self.timeline_preview_needs_rebuild = true;
        self.playing = false;
        self.timeline_playback_clock = None;
        self.playhead = TimelineTime::ZERO;
        self.select_only_clip(self.project.clips.first().map(|clip| clip.id));
        if !self.project.clips.is_empty() {
            self.load_timeline_position(TimelineTime::ZERO, false);
        }
        self.next_id = self.next_id.max(self.project.next_id());
        self.save_project();
    }

    pub(super) fn save_project(&mut self) {
        if let Err(error) = self.project.save(&self.timeline_file_path()) {
            self.error = Some(format!("Could not autosave timeline: {error}"));
            return;
        }
        if self.timeline_preview_needs_rebuild && self.preview_target == PreviewTarget::Timeline {
            self.load_timeline_position(self.playhead, self.playing);
        }
    }

    pub(super) fn take_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub(super) fn toggle_track_magnet(&mut self) {
        self.track_magnet_enabled = !self.track_magnet_enabled;
    }
}

fn unlocked_clip_ids(project: &Project) -> HashSet<u64> {
    project
        .clips
        .iter()
        .filter(|clip| {
            project
                .track(clip.track_id)
                .is_some_and(|track| !track.locked)
        })
        .map(|clip| clip.id)
        .collect()
}

fn find_append_track(project: &Project, asset: &MediaAsset) -> Result<u64, String> {
    let target_kind = if asset.kind == MediaKind::Audio {
        TrackKind::Audio
    } else {
        TrackKind::Video
    };
    project
        .tracks
        .iter()
        .find(|track| track.kind == target_kind && !track.locked)
        .map(|track| track.id)
        .ok_or_else(|| {
            let kind = match target_kind {
                TrackKind::Video => "video",
                TrackKind::Audio => "audio",
            };
            format!("Add an unlocked {kind} track before adding media to the timeline.")
        })
}

fn ripple_clips_after_deletion(clips: &mut [TimelineClip], deleted_ids: &HashSet<u64>) {
    let deleted = clips
        .iter()
        .filter(|clip| deleted_ids.contains(&clip.id))
        .map(|clip| (clip.track_id, clip.timeline_end(), clip.duration()))
        .collect::<Vec<_>>();

    for clip in clips
        .iter_mut()
        .filter(|clip| !deleted_ids.contains(&clip.id))
    {
        let shift = deleted
            .iter()
            .filter(|(track_id, deleted_end, _)| {
                *track_id == clip.track_id && *deleted_end <= clip.timeline_start
            })
            .fold(TimelineTime::ZERO, |total, (_, _, duration)| {
                total + *duration
            });
        clip.timeline_start -= shift;
    }
}

fn clips_crossing_playhead(project: &Project, playhead: TimelineTime) -> Vec<TimelineClip> {
    project
        .clips
        .iter()
        .filter(|clip| {
            let local = playhead - clip.timeline_start;
            let crosses_playhead = local >= TimelineTime::ONE_FRAME
                && local <= clip.duration() - TimelineTime::ONE_FRAME;
            let track_is_editable = project
                .track(clip.track_id)
                .is_some_and(|track| !track.locked);
            crosses_playhead && track_is_editable
        })
        .cloned()
        .collect()
}

fn validate_clipboard_placements(
    project: &Project,
    clips: &[TimelineClip],
) -> Result<(), ClipPlacementRejection> {
    if clips.is_empty() {
        return Err(ClipPlacementRejection::NoPlacements);
    }
    for clip in clips {
        let Some(asset) = project.asset(clip.asset_id) else {
            return Err(ClipPlacementRejection::MissingAsset);
        };
        validate_clip_placement(
            project,
            clip.track_id,
            asset.kind,
            clip.duration(),
            clip.timeline_start,
            &HashSet::new(),
        )?;
    }
    for (index, clip) in clips.iter().enumerate() {
        if clips[index + 1..].iter().any(|other| {
            clip.track_id == other.track_id
                && timeline_ranges_overlap(
                    clip.timeline_start,
                    clip.timeline_end(),
                    other.timeline_start,
                    other.timeline_end(),
                )
        }) {
            return Err(ClipPlacementRejection::ProposedClipsOverlap);
        }
    }
    Ok(())
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[cfg(test)]
#[path = "editing.test.rs"]
mod tests;
