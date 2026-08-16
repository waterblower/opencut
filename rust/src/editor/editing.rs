use super::*;
use std::path::Path;

#[derive(Clone)]
pub(super) struct ClipClipboard {
    source_timeline: PathBuf,
    source_frame_rate: FrameRate,
    clips: Vec<TimelineClip>,
    assets: Vec<MediaAsset>,
    tracks: Vec<(Ulid, TrackKind, usize)>,
    selection_start: TimelineTime,
    primary_index: Option<usize>,
}

impl ClipClipboard {
    fn from_selection(
        source_timeline: PathBuf,
        timeline: &Timeline,
        selected_clip_ids: &HashSet<Ulid>,
        primary_clip_id: Option<Ulid>,
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
        let asset_ids = clips
            .iter()
            .map(|clip| clip.asset_id)
            .collect::<HashSet<_>>();
        let assets = timeline
            .assets
            .iter()
            .filter(|asset| asset_ids.contains(&asset.id))
            .cloned()
            .collect::<Vec<_>>();
        if assets.len() != asset_ids.len() {
            return None;
        }
        let track_ids = clips
            .iter()
            .map(|clip| clip.track_id)
            .collect::<HashSet<_>>();
        let tracks = timeline
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| track_ids.contains(&track.id))
            .map(|(index, track)| {
                let ordinal = timeline.tracks[..index]
                    .iter()
                    .filter(|candidate| candidate.kind == track.kind)
                    .count();
                (track.id, track.kind, ordinal)
            })
            .collect::<Vec<_>>();
        if tracks.len() != track_ids.len() {
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
            source_timeline,
            source_frame_rate: timeline.settings.frame_rate,
            clips,
            assets,
            tracks,
            selection_start,
            primary_index,
        })
    }

    fn clips_at(&self, position: TimelineTime, frame_rate: FrameRate) -> Vec<TimelineClip> {
        self.clips
            .iter()
            .cloned()
            .map(|mut clip| {
                let relative_start = clip.timeline_start - self.selection_start;
                clip.timeline_start = position
                    + self
                        .source_frame_rate
                        .rescale_nearest(relative_start, frame_rate);
                clip.source_in = self
                    .source_frame_rate
                    .rescale_nearest(clip.source_in, frame_rate);
                clip.source_out = self
                    .source_frame_rate
                    .rescale_nearest(clip.source_out, frame_rate)
                    .max(clip.source_in + TimelineTime::ONE_FRAME);
                clip
            })
            .collect()
    }

    fn prepare_paste(
        &self,
        destination_path: &std::path::Path,
        destination: &Timeline,
        position: TimelineTime,
    ) -> Result<(Vec<TimelineClip>, Vec<MediaAsset>), ClipPlacementRejection> {
        let mut clips = self.clips_at(position, destination.settings.frame_rate);
        let same_timeline = self.source_timeline == destination_path;

        if same_timeline {
            if self
                .tracks
                .iter()
                .any(|(track_id, _, _)| destination.track(*track_id).is_none())
            {
                return Err(ClipPlacementRejection::MissingTrack);
            }
        } else {
            let mut track_ids = HashMap::new();
            for (source_track_id, kind, ordinal) in &self.tracks {
                let Some(destination_track) = destination
                    .tracks
                    .iter()
                    .filter(|track| track.kind == *kind)
                    .nth(*ordinal)
                else {
                    return Err(ClipPlacementRejection::MissingTrack);
                };
                track_ids.insert(*source_track_id, destination_track.id);
            }
            for clip in &mut clips {
                clip.track_id = *track_ids
                    .get(&clip.track_id)
                    .ok_or(ClipPlacementRejection::MissingTrack)?;
            }
        }

        let mut new_assets: Vec<MediaAsset> = Vec::new();
        if same_timeline {
            if self
                .assets
                .iter()
                .any(|asset| destination.asset(asset.id).is_none())
            {
                return Err(ClipPlacementRejection::MissingAsset);
            }
        } else {
            let mut asset_ids = HashMap::new();
            for source_asset in &self.assets {
                let destination_asset_id = destination
                    .assets
                    .iter()
                    .find(|asset| asset.path == source_asset.path)
                    .or_else(|| {
                        new_assets
                            .iter()
                            .find(|asset| asset.path == source_asset.path)
                    })
                    .map(|asset| asset.id)
                    .unwrap_or_else(|| {
                        let mut asset = source_asset.clone();
                        asset.id = Ulid::generate();
                        let id = asset.id;
                        new_assets.push(asset);
                        id
                    });
                asset_ids.insert(source_asset.id, destination_asset_id);
            }
            for clip in &mut clips {
                clip.asset_id = *asset_ids
                    .get(&clip.asset_id)
                    .ok_or(ClipPlacementRejection::MissingAsset)?;
            }
        }

        let mut validation_timeline = destination.clone();
        validation_timeline
            .assets
            .extend(new_assets.iter().cloned());
        validate_clipboard_placements(&validation_timeline, &clips)?;
        Ok((clips, new_assets))
    }
}

impl TimelineState {
    pub(super) fn blade_at_playhead(&mut self, project_root: &Path) {
        let Some(updated_timeline) = blade_at_playhead(&self.data, self.playhead) else {
            return;
        };

        self.record_editing_history();
        self.data = updated_timeline.clone();
        let split_count = updated_timeline.clips.len();
        eprintln!(
            "Bladed {split_count} clip{} at the playhead.",
            plural_suffix(split_count)
        );
        self.save(project_root);
    }
}

impl Editor {
    pub(super) fn delete_selected(&mut self) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        if !timeline.selected_clips_editable() {
            return;
        }
        let clip_ids = timeline.interaction.selected_clip_ids.clone();
        let magnet_enabled = timeline.interaction.magnet_enabled;
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.record_editing_history();
        self.remove_clips(&clip_ids, magnet_enabled);
    }

    pub(super) fn copy_selected_clips(&mut self) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let Some(clipboard) = ClipClipboard::from_selection(
            timeline.path.clone(),
            &timeline.data,
            &timeline.interaction.selected_clip_ids,
            timeline.interaction.selected_clip_id,
        ) else {
            return;
        };
        let count = clipboard.clips.len();
        self.clipboard = Some(clipboard);
        self.status = Some(format!("Copied {count} clip{}.", plural_suffix(count)));
    }

    pub(super) fn cut_selected_clips(&mut self) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        if !timeline.selected_clips_editable() {
            eprintln!("Cannot cut clips from a locked track.");
            return;
        }
        let Some(clipboard) = ClipClipboard::from_selection(
            timeline.path.clone(),
            &timeline.data,
            &timeline.interaction.selected_clip_ids,
            timeline.interaction.selected_clip_id,
        ) else {
            return;
        };
        let count = clipboard.clips.len();
        let clip_ids = timeline.interaction.selected_clip_ids.clone();
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.record_editing_history();
        self.clipboard = Some(clipboard);
        self.remove_clips(&clip_ids, false);
        self.status = Some(format!("Cut {count} clip{}.", plural_suffix(count)));
    }

    pub(super) fn paste_clips(&mut self, cx: &mut Context<Self>) {
        let Some(clipboard) = self.clipboard.clone() else {
            return;
        };
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let playhead = timeline.playhead;
        let (mut clips, assets) =
            match clipboard.prepare_paste(&timeline.path, &timeline.data, playhead) {
                Ok(paste) => paste,
                Err(rejection) => {
                    eprintln!("Cannot paste clips: {}.", rejection.message());
                    return;
                }
            };

        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.record_editing_history();
        self.preview.timeline_needs_rebuild = true;
        for clip in &mut clips {
            clip.id = Ulid::generate();
        }
        let count = clips.len();
        timeline.data.assets.extend(assets);
        timeline.interaction.selected_clip_ids = clips.iter().map(|clip| clip.id).collect();
        timeline.interaction.selected_clip_id = clipboard
            .primary_index
            .and_then(|index| clips.get(index))
            .or_else(|| clips.first())
            .map(|clip| clip.id);
        timeline.data.clips.extend(clips);
        self.preview.target = PreviewTarget::Timeline;
        self.status = Some(format!("Pasted {count} clip{}.", plural_suffix(count)));
        timeline.save(&self.global_settings.project_root);
        self.rebuild_timeline_preview_if_needed();
        self.load_timeline_position_with_options(playhead, false, true);
        self.schedule_active_timeline_waveforms(cx);
    }

    fn remove_clips(&mut self, clip_ids: &HashSet<Ulid>, close_track_gaps: bool) {
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
        timeline.interaction.selected_clip_ids.clear();
        timeline.interaction.selected_clip_id = None;
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
            self.load_timeline_position_with_options(playhead, false, true);
        }
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        timeline.save(&self.global_settings.project_root);
        self.rebuild_timeline_preview_if_needed();
    }

    pub(super) fn duplicate_selected(&mut self) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let clip_ids = timeline.selected_clip_ids_in_timeline_order();
        if clip_ids.is_empty() || !timeline.selected_clips_editable() {
            return;
        }
        let clips = clip_ids
            .iter()
            .filter_map(|clip_id| timeline.data.clip(*clip_id).cloned())
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
            if timeline
                .data
                .validate_clip_move_placements(&candidate, &HashSet::new())
                .is_ok()
            {
                break candidate;
            }
            let mut next_delta = delta + TimelineTime::ONE_FRAME;
            for (clip, (_, track_id, start)) in clips.iter().zip(&candidate) {
                for other in timeline
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

        let primary_index = timeline
            .interaction
            .selected_clip_id
            .and_then(|id| clips.iter().position(|clip| clip.id == id));
        let mut duplicates = Vec::with_capacity(clips.len());
        for (mut clip, (_, _, start)) in clips.into_iter().zip(placements) {
            clip.id = Ulid::generate();
            clip.timeline_start = start;
            duplicates.push(clip);
        }
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.record_editing_history();
        self.preview.timeline_needs_rebuild = true;
        timeline.interaction.selected_clip_ids = duplicates.iter().map(|clip| clip.id).collect();
        timeline.interaction.selected_clip_id = primary_index
            .and_then(|index| duplicates.get(index))
            .or_else(|| duplicates.first())
            .map(|clip| clip.id);
        timeline.data.clips.extend(duplicates);
        timeline.save(&self.global_settings.project_root);
        self.rebuild_timeline_preview_if_needed();
    }

    pub(super) fn add_track(&mut self, kind: TrackKind) {
        let Some(timeline) = self.timeline.as_ref() else {
            eprintln!("Create or select a timeline before adding tracks.");
            return;
        };
        let number = timeline
            .data
            .tracks
            .iter()
            .filter(|track| track.kind == kind)
            .count()
            + 1;
        let prefix = match kind {
            TrackKind::Video => "Video",
            TrackKind::Audio => "Audio",
        };
        let id = Ulid::generate();
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.record_editing_history();
        self.preview.timeline_needs_rebuild = true;
        timeline.data.tracks.push(TimelineTrack {
            id,
            name: format!("{prefix} {number}"),
            kind,
            locked: false,
            muted: false,
            visible: true,
        });
        timeline.save(&self.global_settings.project_root);
        self.rebuild_timeline_preview_if_needed();
    }

    pub(super) fn toggle_track_lock(&mut self, track_id: Ulid) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.record_editing_history();
        if let Some(track) = timeline.data.track_mut(track_id) {
            track.locked = !track.locked;
        }
        timeline.save(&self.global_settings.project_root);
        self.rebuild_timeline_preview_if_needed();
    }

    pub(super) fn toggle_track_visibility(&mut self, track_id: Ulid) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        let playhead = timeline.playhead;
        timeline.record_editing_history();
        self.preview.timeline_needs_rebuild = true;
        if let Some(track) = timeline.data.track_mut(track_id) {
            track.visible = !track.visible;
        }
        timeline.save(&self.global_settings.project_root);
        self.rebuild_timeline_preview_if_needed();
        self.load_timeline_position_with_options(playhead, false, true);
    }

    pub(super) fn toggle_track_mute(&mut self, track_id: Ulid) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        let playhead = timeline.playhead;
        timeline.record_editing_history();
        self.preview.timeline_needs_rebuild = true;
        if let Some(track) = timeline.data.track_mut(track_id) {
            track.muted = !track.muted;
        }
        timeline.save(&self.global_settings.project_root);
        self.rebuild_timeline_preview_if_needed();
        self.load_timeline_position_with_options(playhead, self.preview.playing, true);
    }

    pub(super) fn move_track(&mut self, track_id: Ulid, direction: i8) {
        let Some(timeline) = self.timeline.as_mut() else {
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
        timeline.record_editing_history();
        self.preview.timeline_needs_rebuild = true;
        timeline.data.tracks.swap(index, target);
        timeline.save(&self.global_settings.project_root);
        self.rebuild_timeline_preview_if_needed();
        self.load_timeline_position_with_options(playhead, false, true);
    }

    pub(super) fn delete_track(&mut self, track_id: Ulid) {
        let Some(timeline) = self.timeline.as_mut() else {
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
        timeline.record_editing_history();
        self.preview.timeline_needs_rebuild = true;
        timeline.data.tracks.remove(index);
        timeline.data.clips.retain(|clip| clip.track_id != track_id);
        let remaining_clip_ids = timeline
            .data
            .clips
            .iter()
            .map(|clip| clip.id)
            .collect::<HashSet<_>>();
        timeline
            .interaction
            .selected_clip_ids
            .retain(|id| remaining_clip_ids.contains(id));
        if timeline
            .interaction
            .selected_clip_id
            .is_some_and(|id| timeline.data.clip(id).is_none())
        {
            timeline.interaction.selected_clip_id = timeline
                .data
                .clips
                .iter()
                .find(|clip| timeline.interaction.selected_clip_ids.contains(&clip.id))
                .map(|clip| clip.id);
        }
        timeline.save(&self.global_settings.project_root);
        self.rebuild_timeline_preview_if_needed();
        self.load_timeline_position_with_options(playhead, false, true);
    }

    pub(super) fn select_only_clip(&mut self, clip_id: Option<Ulid>) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.interaction.selected_clip_ids.clear();
        if let Some(clip_id) = clip_id {
            timeline.interaction.selected_clip_ids.insert(clip_id);
        }
        timeline.interaction.selected_clip_id = clip_id;
        self.properties.transform_input_clip_id = None;
    }

    pub(super) fn select_all_unlocked_clips(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.interaction.selected_clip_ids = unlocked_clip_ids(&timeline.data);
        timeline.interaction.selected_clip_id = timeline
            .data
            .clips
            .iter()
            .find(|clip| timeline.interaction.selected_clip_ids.contains(&clip.id))
            .map(|clip| clip.id);
        self.properties.transform_input_clip_id = None;
    }

    pub(super) fn toggle_clip_selection(&mut self, clip_id: Ulid) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        if timeline.interaction.selected_clip_ids.remove(&clip_id) {
            if timeline.interaction.selected_clip_id == Some(clip_id) {
                timeline.interaction.selected_clip_id = timeline
                    .data
                    .clips
                    .iter()
                    .find(|clip| timeline.interaction.selected_clip_ids.contains(&clip.id))
                    .map(|clip| clip.id);
            }
        } else if timeline.data.clip(clip_id).is_some() {
            timeline.interaction.selected_clip_ids.insert(clip_id);
            timeline.interaction.selected_clip_id = Some(clip_id);
        }
        self.properties.transform_input_clip_id = None;
    }

    pub(super) fn undo(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        let Some(mut snapshot) = timeline.undo_stack.pop() else {
            return;
        };
        snapshot.view = timeline.data.view.clone();
        let current = std::mem::replace(&mut timeline.data, snapshot);
        timeline.redo_stack.push(current);
        self.reset_after_history_change();
    }

    pub(super) fn redo(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        let Some(mut snapshot) = timeline.redo_stack.pop() else {
            return;
        };
        snapshot.view = timeline.data.view.clone();
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
            .clamp(TimelineTime::ZERO, timeline.data.content_duration());
        let available_clip_ids = timeline
            .data
            .clips
            .iter()
            .map(|clip| clip.id)
            .collect::<HashSet<_>>();
        timeline
            .interaction
            .selected_clip_ids
            .retain(|clip_id| available_clip_ids.contains(clip_id));
        timeline.interaction.selected_clip_id = timeline
            .interaction
            .selected_clip_id
            .filter(|clip_id| timeline.interaction.selected_clip_ids.contains(clip_id))
            .or_else(|| {
                timeline
                    .data
                    .clips
                    .iter()
                    .find(|clip| timeline.interaction.selected_clip_ids.contains(&clip.id))
                    .map(|clip| clip.id)
            });
        let has_clips = !timeline.data.clips.is_empty();
        let playhead = timeline.playhead;
        self.properties.transform_input_clip_id = None;
        if has_clips {
            self.load_timeline_position_with_options(playhead, false, true);
        }
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        timeline.save(&self.global_settings.project_root);
        self.rebuild_timeline_preview_if_needed();
    }

    pub(super) fn save_timeline_playhead(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.capture_playhead();
        timeline.save(&self.global_settings.project_root);
        self.rebuild_timeline_preview_if_needed();
    }

    pub(super) fn save_timeline_scroll(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.capture_scroll();
        timeline.save(&self.global_settings.project_root);
        self.rebuild_timeline_preview_if_needed();
    }

    pub(super) fn toggle_track_magnet(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.interaction.magnet_enabled = !timeline.interaction.magnet_enabled;
        timeline.data.view.track_magnet_enabled = timeline.interaction.magnet_enabled;
        timeline.save(&self.global_settings.project_root);
        self.rebuild_timeline_preview_if_needed();
    }
}

fn unlocked_clip_ids(timeline: &Timeline) -> HashSet<Ulid> {
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

fn ripple_clips_after_deletion(clips: &mut [TimelineClip], deleted_ids: &HashSet<Ulid>) {
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

fn blade_at_playhead(timeline: &Timeline, playhead: TimelineTime) -> Option<Timeline> {
    let clips_at_playhead = timeline
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
        .collect::<Vec<_>>();
    if clips_at_playhead.is_empty() {
        return None;
    }

    let split_clips = clips_at_playhead
        .iter()
        .flat_map(|clip| {
            let (left, right) = clip
                .split_at(playhead)
                .expect("clips at the playhead must be splittable");
            [left, right]
        })
        .collect::<Vec<_>>();
    let removed_clip_ids = clips_at_playhead
        .into_iter()
        .map(|clip| clip.id)
        .collect::<HashSet<_>>();
    let mut updated_timeline = timeline.clone();
    updated_timeline
        .clips
        .retain(|clip| !removed_clip_ids.contains(&clip.id));
    updated_timeline.clips.extend(split_clips);

    Some(updated_timeline)
}

#[cfg(test)]
#[path = "editing.test.rs"]
mod tests;
