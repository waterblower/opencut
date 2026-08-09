use super::*;

#[derive(Clone)]
pub(super) struct ClipClipboard {
    clips: Vec<TimelineClip>,
    selection_start: TimelineTime,
    primary_index: Option<usize>,
}

impl ClipClipboard {
    fn from_selection(
        timeline: &Timeline,
        selected_clip_ids: &HashSet<u64>,
        primary_clip_id: Option<u64>,
    ) -> Option<Self> {
        let clips = timeline
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
        let Some(asset) = self
            .timeline
            .as_ref()
            .and_then(|timeline| timeline.data.asset(asset_id))
            .cloned()
        else {
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
        self.save_timeline();
    }

    pub(super) fn find_append_track_for_asset(&self, asset: &MediaAsset) -> Result<u64, String> {
        let timeline = self
            .timeline
            .as_ref()
            .ok_or_else(|| "Create or select a timeline before adding media.".to_string())?;
        find_append_track(&timeline.data, asset)
    }

    pub(super) fn append_asset_clip_without_checkpoint(
        &mut self,
        asset_id: u64,
        track_id: u64,
        duration: f64,
    ) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let timeline_start = timeline.data.content_duration();
        let source_out = timeline.data.ceil_time(duration);
        let id = self.take_id();
        self.timeline
            .as_mut()
            .expect("timeline was checked above")
            .data
            .clips
            .push(TimelineClip {
                id,
                track_id,
                asset_id,
                timeline_start,
                source_in: TimelineTime::ZERO,
                source_out,
                video_properties: VideoClipProperties::default(),
                audio_properties: AudioClipProperties::default(),
            });
        self.select_only_clip(Some(id));
        if self.preview.video.is_none() {
            self.load_timeline_position(TimelineTime::ZERO, false);
        }
    }

    pub(super) fn blade_at_playhead(&mut self) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let playhead = timeline.playhead;
        let clips = clips_crossing_playhead(&timeline.data, playhead);
        if clips.is_empty() {
            self.error = Some("No unlocked clip crosses the playhead.".into());
            return;
        }

        self.checkpoint();
        let mut right_halves = Vec::with_capacity(clips.len());
        for clip in clips {
            let Some(index) = self
                .timeline
                .as_ref()
                .and_then(|timeline| timeline.data.clip_index(clip.id))
            else {
                continue;
            };
            let new_id = self.take_id();
            let Some((left, right)) = clip.split_at(playhead, new_id) else {
                continue;
            };
            self.timeline
                .as_mut()
                .expect("timeline was checked above")
                .data
                .clips[index] = left;
            right_halves.push(right);
        }
        let split_count = right_halves.len();
        let timeline = self.timeline.as_mut().expect("timeline was checked above");
        timeline.selected_clip_ids = right_halves.iter().map(|clip| clip.id).collect();
        timeline.selected_clip_id = right_halves.first().map(|clip| clip.id);
        timeline.data.clips.extend(right_halves);
        self.error = None;
        self.status = Some(format!(
            "Bladed {split_count} clip{} at the playhead.",
            plural_suffix(split_count)
        ));
        self.save_timeline();
        self.load_timeline_position(playhead, false);
    }

    pub(super) fn blade_split_clip_at(&mut self, clip_id: u64, position: TimelineTime) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let Some(index) = timeline.data.clip_index(clip_id) else {
            return;
        };
        if self.clip_locked(clip_id) {
            return;
        }
        let clip = timeline.data.clips[index].clone();
        let right_clip_id = self.take_id();
        let Some((left, right)) = clip.split_at(position, right_clip_id) else {
            return;
        };

        self.checkpoint();
        let timeline = self.timeline.as_mut().expect("timeline was checked above");
        timeline.data.clips[index] = left;
        timeline.data.clips.push(right);
        self.select_only_clip(Some(right_clip_id));
        self.error = None;
        self.save_timeline();
        let playhead = self
            .timeline
            .as_ref()
            .expect("timeline was checked above")
            .playhead;
        self.load_timeline_position(playhead, false);
    }

    pub(super) fn delete_selected(&mut self) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        if timeline.selected_clip_ids.is_empty() || !self.selected_clips_editable() {
            return;
        }
        let clip_ids = timeline.selected_clip_ids.clone();
        let magnet_enabled = timeline.magnet_enabled;
        let clip_count = clip_ids.len();
        self.checkpoint();
        self.remove_clips(&clip_ids, magnet_enabled);
        self.status = Some(if magnet_enabled {
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
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let Some(clipboard) = ClipClipboard::from_selection(
            &timeline.data,
            &timeline.selected_clip_ids,
            timeline.selected_clip_id,
        ) else {
            return;
        };
        let count = clipboard.clips.len();
        self.timeline
            .as_mut()
            .expect("timeline was checked above")
            .clipboard = Some(clipboard);
        self.error = None;
        self.status = Some(format!("Copied {count} clip{}.", plural_suffix(count)));
    }

    pub(super) fn cut_selected_clips(&mut self) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        if timeline.selected_clip_ids.is_empty() {
            return;
        }
        if !self.selected_clips_editable() {
            self.error = Some("Cannot cut clips from a locked track.".to_string());
            return;
        }
        let Some(clipboard) = ClipClipboard::from_selection(
            &timeline.data,
            &timeline.selected_clip_ids,
            timeline.selected_clip_id,
        ) else {
            return;
        };
        let count = clipboard.clips.len();
        let clip_ids = timeline.selected_clip_ids.clone();
        self.checkpoint();
        self.timeline
            .as_mut()
            .expect("timeline was checked above")
            .clipboard = Some(clipboard);
        self.remove_clips(&clip_ids, false);
        self.error = None;
        self.status = Some(format!("Cut {count} clip{}.", plural_suffix(count)));
    }

    pub(super) fn paste_clips(&mut self) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let Some(clipboard) = timeline.clipboard.clone() else {
            return;
        };
        let playhead = timeline.playhead;
        let mut clips = clipboard.clips_at(playhead);
        if let Err(rejection) = validate_clipboard_placements(&timeline.data, &clips) {
            self.error = Some(format!("Cannot paste clips: {}.", rejection.message()));
            return;
        }

        self.checkpoint();
        for clip in &mut clips {
            clip.id = self.take_id();
        }
        let count = clips.len();
        let timeline = self.timeline.as_mut().expect("timeline was checked above");
        timeline.selected_clip_ids = clips.iter().map(|clip| clip.id).collect();
        timeline.selected_clip_id = clipboard
            .primary_index
            .and_then(|index| clips.get(index))
            .or_else(|| clips.first())
            .map(|clip| clip.id);
        timeline.data.clips.extend(clips);
        self.preview.target = PreviewTarget::Timeline;
        self.error = None;
        self.status = Some(format!("Pasted {count} clip{}.", plural_suffix(count)));
        self.save_timeline();
        self.load_timeline_position(playhead, false);
    }

    fn remove_clips(&mut self, clip_ids: &HashSet<u64>, close_track_gaps: bool) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        if close_track_gaps {
            ripple_clips_after_deletion(&mut timeline.data.clips, clip_ids);
        }
        timeline
            .data
            .clips
            .retain(|clip| !clip_ids.contains(&clip.id));
        timeline.selected_clip_ids.clear();
        timeline.selected_clip_id = None;
        self.properties.transform_input_clip_id = None;
        self.preview.target = PreviewTarget::Timeline;
        self.preview.video = None;
        self.preview.audio = None;
        self.preview.timeline_needs_rebuild = true;
        self.preview.playing = false;
        self.preview.timeline_clock = None;
        let clips_empty = timeline.data.clips.is_empty();
        let playhead = timeline.playhead;
        if clips_empty {
            timeline.playhead = TimelineTime::ZERO;
        } else {
            self.load_timeline_position(playhead, false);
        }
        self.save_timeline();
    }

    pub(super) fn duplicate_selected(&mut self) {
        let clip_ids = self.selected_clip_ids_in_timeline_order();
        if clip_ids.is_empty() || !self.selected_clips_editable() {
            return;
        }
        let clips = clip_ids
            .iter()
            .filter_map(|clip_id| {
                self.timeline
                    .as_ref()
                    .and_then(|timeline| timeline.data.clip(*clip_id))
                    .cloned()
            })
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
                    .timeline
                    .as_ref()
                    .expect("selected clips require an active timeline")
                    .data
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
            .timeline
            .as_ref()
            .expect("selected clips require an active timeline")
            .selected_clip_id
            .and_then(|id| clips.iter().position(|clip| clip.id == id));
        let mut duplicates = Vec::with_capacity(clips.len());
        for (mut clip, (_, _, start)) in clips.into_iter().zip(placements) {
            clip.id = self.take_id();
            clip.timeline_start = start;
            duplicates.push(clip);
        }
        let timeline = self
            .timeline
            .as_mut()
            .expect("selected clips require an active timeline");
        timeline.selected_clip_ids = duplicates.iter().map(|clip| clip.id).collect();
        timeline.selected_clip_id = primary_index
            .and_then(|index| duplicates.get(index))
            .or_else(|| duplicates.first())
            .map(|clip| clip.id);
        timeline.data.clips.extend(duplicates);
        self.save_timeline();
    }

    pub(super) fn add_track(&mut self, kind: TrackKind) {
        let Some(timeline) = self.timeline.as_ref() else {
            self.error = Some("Create or select a timeline before adding tracks.".to_string());
            return;
        };
        let number = timeline
            .data
            .tracks
            .iter()
            .filter(|track| track.kind == kind)
            .count()
            + 1;
        self.checkpoint();
        let prefix = match kind {
            TrackKind::Video => "Video",
            TrackKind::Audio => "Audio",
        };
        let id = self.take_id();
        self.timeline
            .as_mut()
            .expect("timeline was checked above")
            .data
            .tracks
            .push(TimelineTrack {
                id,
                name: format!("{prefix} {number}"),
                kind,
                locked: false,
                muted: false,
                visible: true,
            });
        self.save_timeline();
    }

    pub(super) fn toggle_track_lock(&mut self, track_id: u64) {
        if self.timeline.is_none() {
            return;
        }
        self.checkpoint();
        if let Some(track) = self
            .timeline
            .as_mut()
            .and_then(|timeline| timeline.data.track_mut(track_id))
        {
            track.locked = !track.locked;
        }
        self.save_timeline();
    }

    pub(super) fn toggle_track_visibility(&mut self, track_id: u64) {
        let Some(playhead) = self.timeline.as_ref().map(|timeline| timeline.playhead) else {
            return;
        };
        self.checkpoint();
        if let Some(track) = self
            .timeline
            .as_mut()
            .and_then(|timeline| timeline.data.track_mut(track_id))
        {
            track.visible = !track.visible;
        }
        self.save_timeline();
        self.load_timeline_position(playhead, false);
    }

    pub(super) fn toggle_track_mute(&mut self, track_id: u64) {
        let Some(playhead) = self.timeline.as_ref().map(|timeline| timeline.playhead) else {
            return;
        };
        self.checkpoint();
        if let Some(track) = self
            .timeline
            .as_mut()
            .and_then(|timeline| timeline.data.track_mut(track_id))
        {
            track.muted = !track.muted;
        }
        self.save_timeline();
        self.load_timeline_position(playhead, self.preview.playing);
    }

    pub(super) fn move_track(&mut self, track_id: u64, direction: i8) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let Some(index) = timeline
            .data
            .tracks
            .iter()
            .position(|track| track.id == track_id)
        else {
            return;
        };
        let target = if direction < 0 {
            index.checked_sub(1)
        } else if index + 1 < timeline.data.tracks.len() {
            Some(index + 1)
        } else {
            None
        };
        let Some(target) = target else {
            return;
        };
        let playhead = timeline.playhead;
        self.checkpoint();
        self.timeline
            .as_mut()
            .expect("timeline was checked above")
            .data
            .tracks
            .swap(index, target);
        self.save_timeline();
        self.load_timeline_position(playhead, false);
    }

    pub(super) fn delete_track(&mut self, track_id: u64) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let Some(index) = timeline
            .data
            .tracks
            .iter()
            .position(|track| track.id == track_id)
        else {
            return;
        };
        if timeline.data.tracks[index].locked {
            return;
        }
        let playhead = timeline.playhead;
        self.checkpoint();
        let timeline = self.timeline.as_mut().expect("timeline was checked above");
        timeline.data.tracks.remove(index);
        timeline.data.clips.retain(|clip| clip.track_id != track_id);
        let remaining_clip_ids = timeline
            .data
            .clips
            .iter()
            .map(|clip| clip.id)
            .collect::<HashSet<_>>();
        timeline
            .selected_clip_ids
            .retain(|id| remaining_clip_ids.contains(id));
        if timeline
            .selected_clip_id
            .is_some_and(|id| timeline.data.clip(id).is_none())
        {
            timeline.selected_clip_id = timeline
                .data
                .clips
                .iter()
                .find(|clip| timeline.selected_clip_ids.contains(&clip.id))
                .map(|clip| clip.id);
        }
        self.save_timeline();
        self.load_timeline_position(playhead, false);
    }

    pub(super) fn select_only_clip(&mut self, clip_id: Option<u64>) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.selected_clip_ids.clear();
        if let Some(clip_id) = clip_id {
            timeline.selected_clip_ids.insert(clip_id);
        }
        timeline.selected_clip_id = clip_id;
        self.properties.transform_input_clip_id = None;
    }

    pub(super) fn select_all_unlocked_clips(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.selected_clip_ids = unlocked_clip_ids(&timeline.data);
        timeline.selected_clip_id = timeline
            .data
            .clips
            .iter()
            .find(|clip| timeline.selected_clip_ids.contains(&clip.id))
            .map(|clip| clip.id);
        self.properties.transform_input_clip_id = None;
    }

    pub(super) fn toggle_clip_selection(&mut self, clip_id: u64) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        if timeline.selected_clip_ids.remove(&clip_id) {
            if timeline.selected_clip_id == Some(clip_id) {
                timeline.selected_clip_id = timeline
                    .data
                    .clips
                    .iter()
                    .find(|clip| timeline.selected_clip_ids.contains(&clip.id))
                    .map(|clip| clip.id);
            }
        } else if timeline.data.clip(clip_id).is_some() {
            timeline.selected_clip_ids.insert(clip_id);
            timeline.selected_clip_id = Some(clip_id);
        }
        self.properties.transform_input_clip_id = None;
    }

    pub(super) fn selected_clip_ids_in_timeline_order(&self) -> Vec<u64> {
        self.timeline.as_ref().map_or_else(Vec::new, |timeline| {
            timeline
                .data
                .clips
                .iter()
                .filter(|clip| timeline.selected_clip_ids.contains(&clip.id))
                .map(|clip| clip.id)
                .collect()
        })
    }

    pub(super) fn selected_clips_editable(&self) -> bool {
        self.timeline.as_ref().is_some_and(|timeline| {
            !timeline.selected_clip_ids.is_empty()
                && timeline.selected_clip_ids.iter().all(|clip_id| {
                    timeline.data.clip(*clip_id).is_some() && !self.clip_locked(*clip_id)
                })
        })
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
        let Some(timeline) = self.timeline.as_ref() else {
            return Err(ClipPlacementRejection::MissingTrack);
        };
        if placements.is_empty() {
            return Err(ClipPlacementRejection::NoPlacements);
        }
        for (clip_id, track_id, start) in placements {
            let Some(clip) = timeline.data.clip(*clip_id) else {
                return Err(ClipPlacementRejection::MissingClip);
            };
            let Some(asset) = timeline.data.asset(clip.asset_id) else {
                return Err(ClipPlacementRejection::MissingAsset);
            };
            validate_clip_placement(
                &timeline.data,
                *track_id,
                asset.kind,
                clip.duration(),
                *start,
                ignored_clip_ids,
            )?;
        }
        for (index, (clip_id, track_id, start)) in placements.iter().enumerate() {
            let duration = self
                .timeline
                .as_ref()
                .and_then(|timeline| timeline.data.clip(*clip_id))
                .map(TimelineClip::duration)
                .ok_or(ClipPlacementRejection::MissingClip)?;
            if placements[index + 1..]
                .iter()
                .any(|(other_id, other_track_id, other_start)| {
                    let other_duration = self
                        .timeline
                        .as_ref()
                        .and_then(|timeline| timeline.data.clip(*other_id))
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
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        let snapshot = timeline.data.clone();
        timeline.undo_stack.push(snapshot);
        if timeline.undo_stack.len() > 100 {
            timeline.undo_stack.remove(0);
        }
        timeline.redo_stack.clear();
        self.preview.timeline_needs_rebuild = true;
    }

    pub(super) fn undo(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        let Some(snapshot) = timeline.undo_stack.pop() else {
            return;
        };
        let current = std::mem::replace(&mut timeline.data, snapshot);
        timeline.redo_stack.push(current);
        self.reset_after_history_change();
    }

    pub(super) fn redo(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        let Some(snapshot) = timeline.redo_stack.pop() else {
            return;
        };
        let current = std::mem::replace(&mut timeline.data, snapshot);
        timeline.undo_stack.push(current);
        self.reset_after_history_change();
    }

    pub(super) fn reset_after_history_change(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        self.preview.target = PreviewTarget::Timeline;
        self.preview.video = None;
        self.preview.audio = None;
        self.preview.timeline_needs_rebuild = true;
        self.preview.playing = false;
        self.preview.timeline_clock = None;
        timeline.playhead = timeline
            .playhead
            .clamp(TimelineTime::ZERO, timeline.data.timeline_duration());
        let available_clip_ids = timeline
            .data
            .clips
            .iter()
            .map(|clip| clip.id)
            .collect::<HashSet<_>>();
        timeline
            .selected_clip_ids
            .retain(|clip_id| available_clip_ids.contains(clip_id));
        timeline.selected_clip_id = timeline
            .selected_clip_id
            .filter(|clip_id| timeline.selected_clip_ids.contains(clip_id))
            .or_else(|| {
                timeline
                    .data
                    .clips
                    .iter()
                    .find(|clip| timeline.selected_clip_ids.contains(&clip.id))
                    .map(|clip| clip.id)
            });
        let has_clips = !timeline.data.clips.is_empty();
        let playhead = timeline.playhead;
        timeline.next_id = timeline.next_id.max(timeline.data.next_id());
        self.properties.transform_input_clip_id = None;
        if has_clips {
            self.load_timeline_position(playhead, false);
        }
        self.save_timeline();
    }

    pub(super) fn save_timeline(&mut self) {
        if let Some(timeline) = self.timeline.as_ref()
            && let Err(error) = timeline.data.save(&self.project_root.join(&timeline.path))
        {
            self.error = Some(format!("Could not autosave timeline: {error}"));
            return;
        }
        if self.preview.timeline_needs_rebuild && self.preview.target == PreviewTarget::Timeline {
            let Some(playhead) = self.timeline.as_ref().map(|timeline| timeline.playhead) else {
                return;
            };
            self.load_timeline_position(playhead, self.preview.playing);
        }
    }

    pub(super) fn take_id(&mut self) -> u64 {
        let timeline = self
            .timeline
            .as_mut()
            .expect("IDs can only be allocated for an active timeline");
        let id = timeline.next_id;
        timeline.next_id += 1;
        id
    }

    pub(super) fn toggle_track_magnet(&mut self) {
        if let Some(timeline) = self.timeline.as_mut() {
            timeline.magnet_enabled = !timeline.magnet_enabled;
        }
    }
}

fn unlocked_clip_ids(timeline: &Timeline) -> HashSet<u64> {
    timeline
        .clips
        .iter()
        .filter(|clip| {
            timeline
                .track(clip.track_id)
                .is_some_and(|track| !track.locked)
        })
        .map(|clip| clip.id)
        .collect()
}

fn find_append_track(timeline: &Timeline, asset: &MediaAsset) -> Result<u64, String> {
    let target_kind = if asset.kind == MediaKind::Audio {
        TrackKind::Audio
    } else {
        TrackKind::Video
    };
    timeline
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

fn clips_crossing_playhead(timeline: &Timeline, playhead: TimelineTime) -> Vec<TimelineClip> {
    timeline
        .clips
        .iter()
        .filter(|clip| {
            let local = playhead - clip.timeline_start;
            let crosses_playhead = local >= TimelineTime::ONE_FRAME
                && local <= clip.duration() - TimelineTime::ONE_FRAME;
            let track_is_editable = timeline
                .track(clip.track_id)
                .is_some_and(|track| !track.locked);
            crosses_playhead && track_is_editable
        })
        .cloned()
        .collect()
}

fn validate_clipboard_placements(
    timeline: &Timeline,
    clips: &[TimelineClip],
) -> Result<(), ClipPlacementRejection> {
    if clips.is_empty() {
        return Err(ClipPlacementRejection::NoPlacements);
    }
    for clip in clips {
        let Some(asset) = timeline.asset(clip.asset_id) else {
            return Err(ClipPlacementRejection::MissingAsset);
        };
        validate_clip_placement(
            timeline,
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
