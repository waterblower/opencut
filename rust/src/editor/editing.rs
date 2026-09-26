use super::*;
use anyhow::Result;
use opencut_player::timeline::TimelineEditingState;

#[derive(Clone)]
pub(super) struct ClipClipboard {
    source_timeline: PathBuf,
    source_frame_rate: FrameRate,
    clips: Vec<Clip>,
    assets: Vec<MediaAsset>,
    tracks: Vec<(Ulid, TrackKind, usize)>,
    selection_start: TimelineTime,
    primary_index: Option<usize>,
}

impl ClipClipboard {
    fn from_selection(
        source_timeline: PathBuf,
        timeline: &TimelineEditingState,
        selected_clip_ids: &HashSet<Ulid>,
        primary_clip_id: Option<Ulid>,
    ) -> Option<Self> {
        let clips = timeline
            .clips
            .iter()
            .filter(|clip| selected_clip_ids.contains(&clip.id()))
            .cloned()
            .collect::<Vec<_>>();
        if clips.is_empty() || clips.len() != selected_clip_ids.len() {
            return None;
        }
        let asset_ids = clips
            .iter()
            .filter_map(|clip| clip.media().map(|clip| clip.asset_id))
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
        let track_ids = clips.iter().map(Clip::track_id).collect::<HashSet<_>>();
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
            .map(Clip::timeline_start)
            .min()
            .unwrap_or(TimelineTime::ZERO);
        let primary_index =
            primary_clip_id.and_then(|clip_id| clips.iter().position(|clip| clip.id() == clip_id));
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

    fn clips_at(&self, position: TimelineTime, frame_rate: FrameRate) -> Vec<Clip> {
        self.clips
            .iter()
            .cloned()
            .map(|mut clip| {
                let relative_start = clip.timeline_start() - self.selection_start;
                clip.set_timeline_start(
                    position
                        + self
                            .source_frame_rate
                            .rescale_nearest(relative_start, frame_rate),
                );
                match &mut clip {
                    Clip::Video(clip) | Clip::Audio(clip) => {
                        clip.source_in = self
                            .source_frame_rate
                            .rescale_nearest(clip.source_in, frame_rate);
                        clip.source_out = self
                            .source_frame_rate
                            .rescale_nearest(clip.source_out, frame_rate)
                            .max(clip.source_in + TimelineTime::ONE_FRAME);
                    }
                    Clip::Text(_) => {}
                }
                clip
            })
            .collect()
    }

    fn prepare_paste(
        &self,
        destination_path: &std::path::Path,
        destination: &TimelineEditingState,
        position: TimelineTime,
    ) -> Result<(Vec<Clip>, Vec<MediaAsset>)> {
        let mut clips = self.clips_at(position, destination.settings.frame_rate);
        let same_timeline = self.source_timeline == destination_path;

        if same_timeline {
            if self
                .tracks
                .iter()
                .any(|(track_id, _, _)| destination.track(*track_id).is_none())
            {
                return Err(ClipPlacementRejection::MissingTrack.into());
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
                    return Err(ClipPlacementRejection::MissingTrack.into());
                };
                track_ids.insert(*source_track_id, destination_track.id);
            }
            for clip in &mut clips {
                let track_id = *track_ids
                    .get(&clip.track_id())
                    .ok_or(ClipPlacementRejection::MissingTrack)?;
                clip.set_track_id(track_id);
            }
        }

        let mut new_assets: Vec<MediaAsset> = Vec::new();
        if same_timeline {
            if self
                .assets
                .iter()
                .any(|asset| destination.asset(asset.id).is_none())
            {
                return Err(ClipPlacementRejection::MissingAsset.into());
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
                let Some(clip) = clip.media_mut() else {
                    continue;
                };
                clip.asset_id = *asset_ids
                    .get(&clip.asset_id)
                    .ok_or(ClipPlacementRejection::MissingAsset)?;
            }
        }

        let mut validation_timeline = destination.clone();
        validation_timeline
            .assets
            .extend(new_assets.iter().cloned());
        validate_clips_placements(&validation_timeline, &clips)?;
        Ok((clips, new_assets))
    }
}

impl TimelineRuntimeState {
    pub(super) fn blade_at_playhead(&mut self, preview: &mut PreviewState) -> Result<()> {
        let clips_to_split = self
            .backend
            .timeline()
            .clips
            .iter()
            .filter(|clip| {
                let local = self.playhead() - clip.timeline_start();
                let crosses_playhead = local >= TimelineTime::ONE_FRAME
                    && local
                        <= clip.frame_length(self.backend.timeline().settings.frame_rate)
                            - TimelineTime::ONE_FRAME;
                let track_is_editable = self
                    .backend
                    .timeline()
                    .track(clip.track_id())
                    .is_some_and(|track| !track.locked);
                crosses_playhead && track_is_editable
            })
            .cloned()
            .collect::<Vec<_>>();
        if clips_to_split.is_empty() {
            return Ok(());
        }
        let removed_clip_ids = clips_to_split.iter().map(Clip::id).collect();
        let split_clips = clips_to_split
            .into_iter()
            .flat_map(|clip| {
                let (left, right) = clip
                    .split_at(self.playhead(), self.backend.timeline().settings.frame_rate)
                    .expect("clips at the playhead must be splittable");
                [left, right]
            })
            .collect();

        self.record_editing_history();
        apply_timeline_edit(
            preview,
            self,
            EditAction::SplitClips {
                removed_clips: removed_clip_ids,
                added_clips: split_clips,
            },
        )?;
        self.save()
    }
}

impl Editor {
    pub(super) fn delete_selected(&mut self) -> Result<()> {
        let Some(timeline) = self.timeline.as_ref() else {
            return Ok(());
        };
        if !timeline.selected_clips_editable() {
            return Ok(());
        }
        let clip_ids = timeline.interaction.selected_clip_ids.clone();
        let magnet_enabled = timeline.track_magnet_enabled;
        let Some(timeline) = self.timeline.as_mut() else {
            return Ok(());
        };
        timeline.record_editing_history();
        self.remove_clips(&clip_ids, magnet_enabled)
    }

    pub(super) fn copy_selected_clips(&mut self) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let Some(clipboard) = ClipClipboard::from_selection(
            timeline.path.clone(),
            timeline.backend.timeline(),
            &timeline.interaction.selected_clip_ids,
            timeline.interaction.selected_clip_id,
        ) else {
            return;
        };
        let count = clipboard.clips.len();
        self.clipboard = Some(clipboard);
        self.status = Some(format!("Copied {count} clip{}.", plural_suffix(count)));
    }

    pub(super) fn cut_selected_clips(&mut self) -> Result<()> {
        let Some(timeline) = self.timeline.as_ref() else {
            return Ok(());
        };
        if !timeline.selected_clips_editable() {
            eprintln!("Cannot cut clips from a locked track.");
            return Ok(());
        }
        let Some(clipboard) = ClipClipboard::from_selection(
            timeline.path.clone(),
            timeline.backend.timeline(),
            &timeline.interaction.selected_clip_ids,
            timeline.interaction.selected_clip_id,
        ) else {
            return Ok(());
        };
        let count = clipboard.clips.len();
        let clip_ids = timeline.interaction.selected_clip_ids.clone();
        let Some(timeline) = self.timeline.as_mut() else {
            return Ok(());
        };
        timeline.record_editing_history();
        self.clipboard = Some(clipboard);
        self.remove_clips(&clip_ids, false)?;
        self.status = Some(format!("Cut {count} clip{}.", plural_suffix(count)));
        Ok(())
    }

    pub(super) fn paste_clips(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let Some(clipboard) = self.clipboard.clone() else {
            return Ok(());
        };
        let Some(timeline) = self.timeline.as_ref() else {
            return Ok(());
        };
        let playhead = timeline.playhead();
        let (mut clips, assets) =
            match clipboard.prepare_paste(&timeline.path, timeline.backend.timeline(), playhead) {
                Ok(paste) => paste,
                Err(rejection) => {
                    eprintln!("Cannot paste clips: {rejection}.");
                    return Ok(());
                }
            };

        let Some(timeline) = self.timeline.as_mut() else {
            return Ok(());
        };
        timeline.record_editing_history();
        for clip in &mut clips {
            clip.set_id(Ulid::generate());
        }
        let count = clips.len();
        timeline.interaction.selected_clip_ids = clips.iter().map(Clip::id).collect();
        timeline.interaction.selected_clip_id = clipboard
            .primary_index
            .and_then(|index| clips.get(index))
            .or_else(|| clips.first())
            .map(Clip::id);

        apply_timeline_edit(
            &mut self.preview,
            timeline,
            EditAction::AddClips { clips, assets },
        )?;

        self.status = Some(format!("Pasted {count} clip{}.", plural_suffix(count)));
        timeline.save()?;

        self.schedule_active_timeline_waveforms(cx);
        Ok(())
    }

    fn remove_clips(&mut self, clip_ids: &HashSet<Ulid>, close_track_gaps: bool) -> Result<()> {
        let Some(timeline) = self.timeline.as_mut() else {
            return Ok(());
        };
        apply_timeline_edit(
            &mut self.preview,
            timeline,
            EditAction::RemoveClips {
                clip_ids: clip_ids.clone(),
                close_track_gaps,
            },
        )?;
        timeline.interaction.selected_clip_ids.clear();
        timeline.interaction.selected_clip_id = None;
        self.properties.transform_input_clip_id = None;
        self.properties.text_input_clip_id = None;

        let Some(timeline) = self.timeline.as_ref() else {
            return Ok(());
        };
        timeline.save()
    }

    pub(super) fn duplicate_selected(&mut self) -> Result<()> {
        let Some(timeline) = self.timeline.as_ref() else {
            return Ok(());
        };
        let clip_ids = timeline.selected_clip_ids_in_timeline_order();
        if clip_ids.is_empty() || !timeline.selected_clips_editable() {
            return Ok(());
        }
        let clips = clip_ids
            .iter()
            .filter_map(|clip_id| timeline.backend.timeline().clip(*clip_id).cloned())
            .collect::<Vec<_>>();
        if clips.len() != clip_ids.len() {
            return Ok(());
        }
        let selection_start = clips
            .iter()
            .map(Clip::timeline_start)
            .min()
            .unwrap_or(TimelineTime::ZERO);
        let selection_end = clips
            .iter()
            .map(|clip| clip.timeline_end(timeline.backend.timeline().settings.frame_rate))
            .max()
            .unwrap_or(selection_start);
        let mut delta = selection_end - selection_start;
        let placements = loop {
            let candidate = clips
                .iter()
                .map(|clip| (clip.id(), clip.track_id(), clip.timeline_start() + delta))
                .collect::<Vec<_>>();
            if timeline
                .backend
                .timeline()
                .validate_clip_move_placements(&candidate, &HashSet::new())
                .is_ok()
            {
                break candidate;
            }
            let mut next_delta = delta + TimelineTime::ONE_FRAME;
            for (clip, (_, track_id, start)) in clips.iter().zip(&candidate) {
                for other in timeline
                    .backend
                    .timeline()
                    .clips
                    .iter()
                    .filter(|other| other.track_id() == *track_id)
                {
                    if timeline_ranges_overlap(
                        *start,
                        *start + clip.frame_length(timeline.backend.timeline().settings.frame_rate),
                        other.timeline_start(),
                        other.timeline_end(timeline.backend.timeline().settings.frame_rate),
                    ) {
                        next_delta = next_delta.max(
                            other.timeline_end(timeline.backend.timeline().settings.frame_rate)
                                - clip.timeline_start(),
                        );
                    }
                }
            }
            delta = next_delta;
        };

        let primary_index = timeline
            .interaction
            .selected_clip_id
            .and_then(|id| clips.iter().position(|clip| clip.id() == id));
        let mut duplicates = Vec::with_capacity(clips.len());
        for (mut clip, (_, _, start)) in clips.into_iter().zip(placements) {
            clip.set_id(Ulid::generate());
            clip.set_timeline_start(start);
            duplicates.push(clip);
        }
        let Some(timeline) = self.timeline.as_mut() else {
            return Ok(());
        };
        timeline.record_editing_history();
        timeline.interaction.selected_clip_ids = duplicates.iter().map(Clip::id).collect();
        timeline.interaction.selected_clip_id = primary_index
            .and_then(|index| duplicates.get(index))
            .or_else(|| duplicates.first())
            .map(Clip::id);
        apply_timeline_edit(
            &mut self.preview,
            timeline,
            EditAction::AddClips {
                clips: duplicates,
                assets: Vec::new(),
            },
        )?;
        timeline.save()
    }

    pub(super) fn add_track(&mut self, kind: TrackKind) -> Result<()> {
        let Some(timeline) = self.timeline.as_ref() else {
            eprintln!("Create or select a timeline before adding tracks.");
            return Ok(());
        };
        let number = timeline
            .backend
            .timeline()
            .tracks
            .iter()
            .filter(|track| track.kind == kind)
            .count()
            + 1;
        let prefix = match kind {
            TrackKind::Video => "Video",
            TrackKind::Audio => "Audio",
            TrackKind::Text => "Text",
        };
        let id = Ulid::generate();
        let Some(timeline) = self.timeline.as_mut() else {
            return Ok(());
        };
        timeline.record_editing_history();
        apply_timeline_edit(
            &mut self.preview,
            timeline,
            EditAction::AddTrack {
                track: Track {
                    id,
                    name: format!("{prefix} {number}"),
                    kind,
                    locked: false,
                    muted: false,
                    visible: true,
                },
            },
        )?;
        timeline.save()
    }

    pub(super) fn toggle_track_lock(&mut self, track_id: Ulid) -> Result<()> {
        let Some(timeline) = self.timeline.as_mut() else {
            return Ok(());
        };
        timeline.record_editing_history();
        apply_timeline_edit(
            &mut self.preview,
            timeline,
            EditAction::ToggleTrackLock { track_id },
        )?;
        timeline.save()
    }

    pub(super) fn toggle_track_visibility(&mut self, track_id: Ulid) -> Result<()> {
        let Some(timeline) = self.timeline.as_mut() else {
            return Ok(());
        };
        timeline.record_editing_history();
        apply_timeline_edit(
            &mut self.preview,
            timeline,
            EditAction::ToggleTrackVisibility { track_id },
        )?;
        timeline.save()
    }

    pub(super) fn toggle_track_mute(&mut self, track_id: Ulid) -> Result<()> {
        let Some(timeline) = self.timeline.as_mut() else {
            return Ok(());
        };
        timeline.record_editing_history();
        apply_timeline_edit(
            &mut self.preview,
            timeline,
            EditAction::ToggleTrackMute { track_id },
        )?;
        timeline.save()
    }

    pub(super) fn move_track(&mut self, track_id: Ulid, direction: i8) -> Result<()> {
        let Some(timeline) = self.timeline.as_mut() else {
            return Ok(());
        };
        let Some(index) = timeline
            .backend
            .timeline()
            .tracks
            .iter()
            .position(|track| track.id == track_id)
        else {
            return Ok(());
        };
        let target = if direction < 0 {
            index.checked_sub(1)
        } else if index + 1 < timeline.backend.timeline().tracks.len() {
            Some(index + 1)
        } else {
            None
        };
        let Some(target) = target else {
            return Ok(());
        };

        timeline.record_editing_history();
        apply_timeline_edit(
            &mut self.preview,
            timeline,
            EditAction::MoveTrack { index, target },
        )?;
        timeline.save()
    }

    pub(super) fn delete_track(&mut self, track_id: Ulid) -> Result<()> {
        let Some(timeline) = self.timeline.as_mut() else {
            return Ok(());
        };
        let Some(index) = timeline
            .backend
            .timeline()
            .tracks
            .iter()
            .position(|track| track.id == track_id)
        else {
            return Ok(());
        };
        if timeline.backend.timeline().tracks[index].locked {
            return Ok(());
        }
        timeline.record_editing_history();
        apply_timeline_edit(
            &mut self.preview,
            timeline,
            EditAction::DeleteTrack { track_id },
        )?;
        let remaining_clip_ids = timeline
            .backend
            .timeline()
            .clips
            .iter()
            .map(Clip::id)
            .collect::<HashSet<_>>();
        timeline
            .interaction
            .selected_clip_ids
            .retain(|id| remaining_clip_ids.contains(id));
        if timeline
            .interaction
            .selected_clip_id
            .is_some_and(|id| timeline.backend.timeline().clip(id).is_none())
        {
            timeline.interaction.selected_clip_id = timeline
                .backend
                .timeline()
                .clips
                .iter()
                .find(|clip| timeline.interaction.selected_clip_ids.contains(&clip.id()))
                .map(Clip::id);
        }
        timeline.save()
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
        self.properties.text_input_clip_id = None;
    }

    pub(super) fn select_all_unlocked_clips(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.interaction.selected_clip_ids = unlocked_clip_ids(timeline.backend.timeline());
        timeline.interaction.selected_clip_id = timeline
            .backend
            .timeline()
            .clips
            .iter()
            .find(|clip| timeline.interaction.selected_clip_ids.contains(&clip.id()))
            .map(Clip::id);
        self.properties.transform_input_clip_id = None;
        self.properties.text_input_clip_id = None;
    }

    pub(super) fn toggle_clip_selection(&mut self, clip_id: Ulid) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        if timeline.interaction.selected_clip_ids.remove(&clip_id) {
            if timeline.interaction.selected_clip_id == Some(clip_id) {
                timeline.interaction.selected_clip_id = timeline
                    .backend
                    .timeline()
                    .clips
                    .iter()
                    .find(|clip| timeline.interaction.selected_clip_ids.contains(&clip.id()))
                    .map(Clip::id);
            }
        } else if timeline.backend.timeline().clip(clip_id).is_some() {
            timeline.interaction.selected_clip_ids.insert(clip_id);
            timeline.interaction.selected_clip_id = Some(clip_id);
        }
        self.properties.transform_input_clip_id = None;
        self.properties.text_input_clip_id = None;
    }

    pub(super) fn undo(&mut self) -> Result<()> {
        let Some(timeline) = self.timeline.as_mut() else {
            return Ok(());
        };
        let Some(snapshot) = timeline.undo_stack.last().cloned() else {
            return Ok(());
        };
        let current = timeline.backend.timeline().clone();
        apply_timeline_edit(
            &mut self.preview,
            timeline,
            EditAction::ReplaceTimeline { timeline: snapshot },
        )?;
        timeline.undo_stack.pop();
        timeline.redo_stack.push(current);
        self.reset_after_history_change()
    }

    pub(super) fn redo(&mut self) -> Result<()> {
        let Some(timeline) = self.timeline.as_mut() else {
            return Ok(());
        };
        let Some(snapshot) = timeline.redo_stack.last().cloned() else {
            return Ok(());
        };
        let current = timeline.backend.timeline().clone();
        apply_timeline_edit(
            &mut self.preview,
            timeline,
            EditAction::ReplaceTimeline { timeline: snapshot },
        )?;
        timeline.redo_stack.pop();
        timeline.undo_stack.push(current);
        self.reset_after_history_change()
    }

    pub(super) fn reset_after_history_change(&mut self) -> Result<()> {
        let Some(timeline) = self.timeline.as_mut() else {
            return Ok(());
        };

        let available_clip_ids = timeline
            .backend
            .timeline()
            .clips
            .iter()
            .map(Clip::id)
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
                    .backend
                    .timeline()
                    .clips
                    .iter()
                    .find(|clip| timeline.interaction.selected_clip_ids.contains(&clip.id()))
                    .map(Clip::id)
            });
        self.properties.transform_input_clip_id = None;
        self.properties.text_input_clip_id = None;
        if !timeline.backend.timeline().clips.is_empty() {
            set_timeline_position(&mut self.preview, &timeline.backend, timeline.playhead())?;
        }
        let Some(timeline) = self.timeline.as_ref() else {
            return Ok(());
        };
        timeline.save()
    }

    pub(super) fn toggle_track_magnet(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.track_magnet_enabled = !timeline.track_magnet_enabled;
    }
}

pub(super) fn validate_clips_placements(
    timeline: &TimelineEditingState,
    clips: &[Clip],
) -> Result<()> {
    if clips.is_empty() {
        return Err(ClipPlacementRejection::NoPlacements.into());
    }
    for clip in clips {
        match clip {
            Clip::Video(clip) | Clip::Audio(clip) => {
                let Some(asset) = timeline.asset(clip.asset_id) else {
                    return Err(ClipPlacementRejection::MissingAsset.into());
                };
                validate_clip_placement(
                    timeline,
                    clip.track_id,
                    asset.kind,
                    clip.source_out - clip.source_in,
                    clip.timeline_start,
                    &HashSet::new(),
                )?;
            }
            Clip::Text(clip) => validate_text_clip_placement(
                timeline,
                clip.track_id,
                clip.frame_length(timeline.settings.frame_rate),
                clip.timeline_start,
                &HashSet::new(),
            )?,
        }
    }
    for (index, clip) in clips.iter().enumerate() {
        if clips[index + 1..].iter().any(|other| {
            clip.track_id() == other.track_id()
                && timeline_ranges_overlap(
                    clip.timeline_start(),
                    clip.timeline_end(timeline.settings.frame_rate),
                    other.timeline_start(),
                    other.timeline_end(timeline.settings.frame_rate),
                )
        }) {
            return Err(ClipPlacementRejection::ProposedClipsOverlap.into());
        }
    }
    Ok(())
}

fn unlocked_clip_ids(timeline: &TimelineEditingState) -> HashSet<Ulid> {
    timeline
        .clips
        .iter()
        .filter(|clip| {
            timeline
                .track(clip.track_id())
                .is_some_and(|track| !track.locked)
        })
        .map(Clip::id)
        .collect()
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[cfg(test)]
#[path = "tests/editing.test.rs"]
mod tests;
