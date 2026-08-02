use super::*;

impl Editor {
    pub(super) fn append_asset_clip(&mut self, asset_id: u64) {
        self.checkpoint();
        self.append_asset_clip_without_checkpoint(asset_id);
        self.save_project();
    }

    pub(super) fn append_asset_clip_without_checkpoint(&mut self, asset_id: u64) {
        let Some(duration) = self.project.asset(asset_id).map(|asset| asset.duration) else {
            return;
        };
        let target_kind = if self
            .project
            .asset(asset_id)
            .is_some_and(|asset| asset.kind == MediaKind::Audio)
        {
            TrackKind::Audio
        } else {
            TrackKind::Video
        };
        let track_id = self
            .project
            .tracks
            .iter()
            .find(|track| track.kind == target_kind && !track.locked)
            .map(|track| track.id)
            .unwrap_or_else(|| {
                let id = self.take_id();
                let number = self
                    .project
                    .tracks
                    .iter()
                    .filter(|track| track.kind == target_kind)
                    .count()
                    + 1;
                let prefix = match target_kind {
                    TrackKind::Video => "Video",
                    TrackKind::Audio => "Audio",
                };
                self.project.tracks.push(TimelineTrack {
                    id,
                    name: format!("{prefix} {number}"),
                    kind: target_kind,
                    locked: false,
                    muted: false,
                    visible: true,
                });
                id
            });
        let id = self.take_id();
        self.project.clips.push(TimelineClip {
            id,
            track_id,
            asset_id: Some(asset_id),
            timeline_start: self.project.content_duration(),
            source_in: TimelineTime::ZERO,
            source_out: self.project.ceil_time(duration),
        });
        self.selected_asset_id = Some(asset_id);
        self.select_only_clip(Some(id));
        if self.loaded_clip_id.is_none() {
            self.load_timeline_position(TimelineTime::ZERO, false);
        }
    }

    pub(super) fn split_selected(&mut self) {
        let clip_ids = self.selected_clip_ids_in_project_order();
        if clip_ids.is_empty() || !self.selected_clips_editable() {
            return;
        }
        let clips = clip_ids
            .iter()
            .filter_map(|clip_id| self.project.clip(*clip_id).cloned())
            .collect::<Vec<_>>();
        let all_contain_playhead = clips.iter().all(|clip| {
            let local = self.playhead - clip.timeline_start;
            local >= TimelineTime::ONE_FRAME && local <= clip.duration() - TimelineTime::ONE_FRAME
        });
        if !all_contain_playhead {
            self.error =
                Some("The playhead must be inside every selected clip before splitting.".into());
            return;
        }

        self.checkpoint();
        let mut right_halves = Vec::with_capacity(clips.len());
        for clip in clips {
            let source_split = clip.source_in + self.playhead - clip.timeline_start;
            if let Some(index) = self.project.clip_index(clip.id) {
                self.project.clips[index].source_out = source_split;
            }
            let new_id = self.take_id();
            right_halves.push(TimelineClip {
                id: new_id,
                track_id: clip.track_id,
                asset_id: clip.asset_id,
                timeline_start: self.playhead,
                source_in: source_split,
                source_out: clip.source_out,
            });
        }
        self.selected_clip_ids = right_halves.iter().map(|clip| clip.id).collect();
        self.selected_clip_id = right_halves.first().map(|clip| clip.id);
        self.project.clips.extend(right_halves);
        self.error = None;
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
        let local = position - clip.timeline_start;
        if local < TimelineTime::ONE_FRAME || local > clip.duration() - TimelineTime::ONE_FRAME {
            return;
        }

        let source_split = clip.source_in + local;
        self.checkpoint();
        self.project.clips[index].source_out = source_split;
        let right_clip_id = self.take_id();
        self.project.clips.push(TimelineClip {
            id: right_clip_id,
            track_id: clip.track_id,
            asset_id: clip.asset_id,
            timeline_start: position,
            source_in: source_split,
            source_out: clip.source_out,
        });
        self.select_only_clip(Some(right_clip_id));
        self.error = None;
        self.save_project();
        self.load_timeline_position(self.playhead, false);
    }

    pub(super) fn delete_selected(&mut self) {
        if self.selected_clip_ids.is_empty() || !self.selected_clips_editable() {
            return;
        }
        self.checkpoint();
        self.project
            .clips
            .retain(|clip| !self.selected_clip_ids.contains(&clip.id));
        self.select_only_clip(None);
        self.preview_target = PreviewTarget::Timeline;
        self.video = None;
        self.standalone_audio = None;
        self.audio_previews.clear();
        self.loaded_clip_id = None;
        self.still_playback_started = None;
        self.playing = false;
        if self.project.clips.is_empty() {
            self.playhead = TimelineTime::ZERO;
        } else {
            self.load_timeline_position(self.playhead, false);
        }
        self.save_project();
    }

    pub(super) fn move_selected(&mut self, direction: i8) {
        let clip_ids = self.selected_clip_ids_in_project_order();
        if clip_ids.is_empty() || !self.selected_clips_editable() {
            return;
        }
        let delta = TimelineTime::from_frames(i64::from(direction));
        let placements = clip_ids
            .iter()
            .filter_map(|clip_id| {
                let clip = self.project.clip(*clip_id)?;
                Some(ClipPlacement {
                    clip_id: *clip_id,
                    track_id: clip.track_id,
                    start: clip.timeline_start + delta,
                    duration: clip.duration(),
                })
            })
            .collect::<Vec<_>>();
        if placements.len() != clip_ids.len()
            || placements
                .iter()
                .any(|placement| placement.start < TimelineTime::ZERO)
            || !self.clip_placements_fit(&placements, &self.selected_clip_ids)
        {
            return;
        }
        self.checkpoint();
        for placement in placements {
            if let Some(clip) = self.project.clip_mut(placement.clip_id) {
                clip.timeline_start = placement.start;
            }
        }
        self.save_project();
        self.load_timeline_position(self.playhead, false);
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
                .map(|clip| ClipPlacement {
                    clip_id: clip.id,
                    track_id: clip.track_id,
                    start: clip.timeline_start + delta,
                    duration: clip.duration(),
                })
                .collect::<Vec<_>>();
            if self.clip_placements_fit(&candidate, &HashSet::new()) {
                break candidate;
            }
            let mut next_delta = delta + TimelineTime::ONE_FRAME;
            for (clip, placement) in clips.iter().zip(&candidate) {
                for other in self
                    .project
                    .clips
                    .iter()
                    .filter(|other| other.track_id == placement.track_id)
                {
                    if placement.start < other.timeline_end()
                        && other.timeline_start < placement.start + placement.duration
                    {
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
        for (mut clip, placement) in clips.into_iter().zip(placements) {
            clip.id = self.take_id();
            clip.timeline_start = placement.start;
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
        let muted = self
            .project
            .track(track_id)
            .is_some_and(|track| track.muted);
        if self
            .loaded_clip_id
            .and_then(|id| self.project.clip(id))
            .is_some_and(|clip| clip.track_id == track_id)
            && let Some(video) = &self.video
        {
            video.set_muted(muted);
        }
        self.sync_audio_previews(self.playhead, self.playing);
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

    pub(super) fn can_split_selected(&self) -> bool {
        self.selected_clips_editable()
            && self.selected_clip_ids.iter().all(|clip_id| {
                self.project.clip(*clip_id).is_some_and(|clip| {
                    let local = self.playhead - clip.timeline_start;
                    local >= TimelineTime::ONE_FRAME
                        && local <= clip.duration() - TimelineTime::ONE_FRAME
                })
            })
    }

    pub(super) fn clip_placements_fit(
        &self,
        placements: &[ClipPlacement],
        ignored_clip_ids: &HashSet<u64>,
    ) -> bool {
        self.clip_placement_error(placements, ignored_clip_ids)
            .is_none()
    }

    pub(super) fn clip_placement_error(
        &self,
        placements: &[ClipPlacement],
        ignored_clip_ids: &HashSet<u64>,
    ) -> Option<&'static str> {
        for placement in placements {
            if placement.start < TimelineTime::ZERO {
                return Some("Cannot move before the timeline start");
            }
            if placement.duration < TimelineTime::ONE_FRAME {
                return Some("Clip duration is invalid");
            }
            let Some(clip) = self.project.clip(placement.clip_id) else {
                return Some("Clip is no longer available");
            };
            let Some(track) = self.project.track(placement.track_id) else {
                return Some("Destination track is unavailable");
            };
            if track.locked {
                return Some("Destination track is locked");
            }
            let Some(asset) = clip.asset_id.and_then(|id| self.project.asset(id)) else {
                return Some("Source media is unavailable");
            };
            let compatible = match track.kind {
                TrackKind::Video => asset.kind != MediaKind::Audio,
                TrackKind::Audio => asset.has_audio,
            };
            if !compatible {
                return Some("Clip type is incompatible with this track");
            }
        }

        for (index, placement) in placements.iter().enumerate() {
            let end = placement.start + placement.duration;
            if placements[index + 1..].iter().any(|other| {
                placement.track_id == other.track_id
                    && placement.start < other.start + other.duration
                    && other.start < end
            }) {
                return Some("Selected clips would overlap each other");
            }
            if self.project.clips.iter().any(|other| {
                !ignored_clip_ids.contains(&other.id)
                    && placement.track_id == other.track_id
                    && placement.start < other.timeline_end()
                    && other.timeline_start < end
            }) {
                return Some("Overlaps another clip");
            }
        }
        None
    }

    pub(super) fn checkpoint(&mut self) {
        self.undo_stack.push(self.project.clone());
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
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
        self.audio_previews.clear();
        self.loaded_clip_id = None;
        self.still_playback_started = None;
        self.playing = false;
        self.playhead = TimelineTime::ZERO;
        self.select_only_clip(self.project.clips.first().map(|clip| clip.id));
        if !self.project.clips.is_empty() {
            self.load_timeline_position(TimelineTime::ZERO, false);
        }
        self.next_id = self.next_id.max(self.project.next_id());
        self.save_project();
    }

    pub(super) fn save_project(&mut self) {
        if let Err(error) = self.project.save(&self.project_root) {
            self.error = Some(format!("Could not autosave project: {error}"));
        }
    }

    pub(super) fn take_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}
