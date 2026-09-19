use super::*;
use anyhow::{Result, anyhow};
use std::path::Path;

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
        timeline: &TimelineSerialization,
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
        destination: &TimelineSerialization,
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
    pub(super) fn blade_at_playhead(
        &mut self,
        preview: &mut PreviewState,
        project_root: &Path,
    ) -> Result<()> {
        let clips_to_split = self
            .data
            .clips
            .iter()
            .filter(|clip| {
                let local = self.playhead() - clip.timeline_start();
                let crosses_playhead = local >= TimelineTime::ONE_FRAME
                    && local
                        <= clip.frame_length(self.data.settings.frame_rate)
                            - TimelineTime::ONE_FRAME;
                let track_is_editable = self
                    .data
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
                    .split_at(self.playhead(), self.data.settings.frame_rate)
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
        )
        .expect("split clip placements were validated before recording history");
        self.data.save(&project_root.join(&self.path))
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
        let magnet_enabled = timeline.interaction.magnet_enabled;
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
            &timeline.data,
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
            match clipboard.prepare_paste(&timeline.path, &timeline.data, playhead) {
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
        )
        .expect("clipboard placements were validated before recording history");

        self.status = Some(format!("Pasted {count} clip{}.", plural_suffix(count)));
        timeline
            .data
            .save(&self.project_root.join(&timeline.path))?;

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
        )
        .expect("removing clips cannot be rejected");
        timeline.interaction.selected_clip_ids.clear();
        timeline.interaction.selected_clip_id = None;
        self.properties.transform_input_clip_id = None;
        self.properties.text_input_clip_id = None;

        let Some(timeline) = self.timeline.as_ref() else {
            return Ok(());
        };
        timeline.data.save(&self.project_root.join(&timeline.path))
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
            .filter_map(|clip_id| timeline.data.clip(*clip_id).cloned())
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
            .map(|clip| clip.timeline_end(timeline.data.settings.frame_rate))
            .max()
            .unwrap_or(selection_start);
        let mut delta = selection_end - selection_start;
        let placements = loop {
            let candidate = clips
                .iter()
                .map(|clip| (clip.id(), clip.track_id(), clip.timeline_start() + delta))
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
                    .filter(|other| other.track_id() == *track_id)
                {
                    if timeline_ranges_overlap(
                        *start,
                        *start + clip.frame_length(timeline.data.settings.frame_rate),
                        other.timeline_start(),
                        other.timeline_end(timeline.data.settings.frame_rate),
                    ) {
                        next_delta = next_delta.max(
                            other.timeline_end(timeline.data.settings.frame_rate)
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
        )
        .expect("duplicate placements were validated before recording history");
        timeline.data.save(&self.project_root.join(&timeline.path))
    }

    pub(super) fn add_track(&mut self, kind: TrackKind) -> Result<()> {
        let Some(timeline) = self.timeline.as_ref() else {
            eprintln!("Create or select a timeline before adding tracks.");
            return Ok(());
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
        )
        .expect("adding a track cannot be rejected");
        timeline.data.save(&self.project_root.join(&timeline.path))
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
        )
        .expect("toggling a track lock cannot be rejected");
        timeline.data.save(&self.project_root.join(&timeline.path))
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
        )
        .expect("toggling track visibility cannot be rejected");
        timeline.data.save(&self.project_root.join(&timeline.path))
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
        )
        .expect("toggling track mute cannot be rejected");
        timeline.data.save(&self.project_root.join(&timeline.path))
    }

    pub(super) fn move_track(&mut self, track_id: Ulid, direction: i8) -> Result<()> {
        let Some(timeline) = self.timeline.as_mut() else {
            return Ok(());
        };
        let Some(index) = timeline
            .data
            .tracks
            .iter()
            .position(|track| track.id == track_id)
        else {
            return Ok(());
        };
        let target = if direction < 0 {
            index.checked_sub(1)
        } else if index + 1 < timeline.data.tracks.len() {
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
        )
        .expect("moving a track cannot be rejected");
        timeline.data.save(&self.project_root.join(&timeline.path))
    }

    pub(super) fn delete_track(&mut self, track_id: Ulid) -> Result<()> {
        let Some(timeline) = self.timeline.as_mut() else {
            return Ok(());
        };
        let Some(index) = timeline
            .data
            .tracks
            .iter()
            .position(|track| track.id == track_id)
        else {
            return Ok(());
        };
        if timeline.data.tracks[index].locked {
            return Ok(());
        }
        timeline.record_editing_history();
        apply_timeline_edit(
            &mut self.preview,
            timeline,
            EditAction::DeleteTrack { track_id },
        )
        .expect("deleting a track cannot be rejected");
        let remaining_clip_ids = timeline
            .data
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
            .is_some_and(|id| timeline.data.clip(id).is_none())
        {
            timeline.interaction.selected_clip_id = timeline
                .data
                .clips
                .iter()
                .find(|clip| timeline.interaction.selected_clip_ids.contains(&clip.id()))
                .map(Clip::id);
        }
        timeline.data.save(&self.project_root.join(&timeline.path))
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
        timeline.interaction.selected_clip_ids = unlocked_clip_ids(&timeline.data);
        timeline.interaction.selected_clip_id = timeline
            .data
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
                    .data
                    .clips
                    .iter()
                    .find(|clip| timeline.interaction.selected_clip_ids.contains(&clip.id()))
                    .map(Clip::id);
            }
        } else if timeline.data.clip(clip_id).is_some() {
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
        let Some(mut snapshot) = timeline.undo_stack.pop() else {
            return Ok(());
        };
        snapshot.view = timeline.data.view.clone();
        let current = timeline.data.clone();
        apply_timeline_edit(
            &mut self.preview,
            timeline,
            EditAction::ReplaceTimeline { timeline: snapshot },
        )
        .expect("restoring history cannot be rejected");
        timeline.redo_stack.push(current);
        self.reset_after_history_change()
    }

    pub(super) fn redo(&mut self) -> Result<()> {
        let Some(timeline) = self.timeline.as_mut() else {
            return Ok(());
        };
        let Some(mut snapshot) = timeline.redo_stack.pop() else {
            return Ok(());
        };
        snapshot.view = timeline.data.view.clone();
        let current = timeline.data.clone();
        apply_timeline_edit(
            &mut self.preview,
            timeline,
            EditAction::ReplaceTimeline { timeline: snapshot },
        )
        .expect("restoring history cannot be rejected");
        timeline.undo_stack.push(current);
        self.reset_after_history_change()
    }

    pub(super) fn reset_after_history_change(&mut self) -> Result<()> {
        let Some(timeline) = self.timeline.as_mut() else {
            return Ok(());
        };

        let available_clip_ids = timeline
            .data
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
                    .data
                    .clips
                    .iter()
                    .find(|clip| timeline.interaction.selected_clip_ids.contains(&clip.id()))
                    .map(Clip::id)
            });
        self.properties.transform_input_clip_id = None;
        self.properties.text_input_clip_id = None;
        if !timeline.data.clips.is_empty() {
            set_timeline_position(&mut self.preview, timeline, timeline.playhead());
        }
        let Some(timeline) = self.timeline.as_ref() else {
            return Ok(());
        };
        timeline.data.save(&self.project_root.join(&timeline.path))
    }

    pub(super) fn toggle_track_magnet(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        apply_timeline_edit(
            &mut self.preview,
            timeline,
            EditAction::SetTrackMagnet {
                enabled: !timeline.interaction.magnet_enabled,
            },
        )
        .expect("changing the track magnet preference cannot be rejected");
    }
}

pub(super) fn validate_clips_placements(
    timeline: &TimelineSerialization,
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
    SetSavedPlayhead {
        playhead: TimelineTime,
    },
    SetScroll {
        horizontal: f32,
        vertical: f32,
    },
    SetTimelineZoom {
        pixels_per_second: f32,
    },
    SetSnapping {
        enabled: bool,
    },
    SetTrackMagnet {
        enabled: bool,
    },
    UpdateAssetPaths {
        paths: Vec<(Ulid, PathBuf)>,
    },
    ReplaceTimeline {
        timeline: TimelineSerialization,
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
    match action {
        EditAction::AddClips { clips, assets } => {
            let mut updated = timeline.data.clone();
            updated.assets.extend(assets);
            validate_clips_placements(&updated, &clips)?;
            updated.clips.extend(clips);
            timeline.data = updated;
        }
        EditAction::RemoveClips {
            clip_ids,
            close_track_gaps,
        } => {
            if close_track_gaps {
                let frame_rate = timeline.data.settings.frame_rate;
                ripple_clips_after_deletion(&mut timeline.data.clips, &clip_ids, frame_rate);
            }
            timeline
                .data
                .clips
                .retain(|clip| !clip_ids.contains(&clip.id()));
        }
        EditAction::SplitClips {
            removed_clips,
            added_clips,
        } => {
            let mut updated = timeline.data.clone();
            updated
                .clips
                .retain(|clip| !removed_clips.contains(&clip.id()));
            validate_clips_placements(&updated, &added_clips)?;
            updated.clips.extend(added_clips);
            timeline.data = updated;
        }
        EditAction::MoveClips { placements } => {
            let ids = placements.iter().map(|(id, _, _)| *id).collect();
            timeline
                .data
                .validate_clip_move_placements(&placements, &ids)?;
            for (id, track_id, start) in placements {
                if let Some(clip) = timeline.data.clip_mut(id) {
                    clip.set_timeline_start(start);
                    clip.set_track_id(track_id);
                }
            }
        }
        EditAction::UpdateClip { clip } => {
            let index = timeline
                .data
                .clip_index(clip.id())
                .ok_or_else(|| anyhow!("The clip being updated no longer exists"))?;
            if let (Clip::Text(previous), Clip::Text(updated)) =
                (&timeline.data.clips[index], &clip)
                && previous.track_id == updated.track_id
                && previous.timeline_start == updated.timeline_start
                && previous.length == updated.length
            {
                timeline.data.clips[index] = clip;
                return Ok(());
            }
            let mut updated = timeline.data.clone();
            updated.clips.remove(index);
            validate_clips_placements(&updated, std::slice::from_ref(&clip))?;
            updated.clips.insert(index, clip);
            timeline.data = updated;
        }
        EditAction::SetVideoProperties {
            clip_ids,
            properties,
        } => {
            for clip in &mut timeline.data.clips {
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
            if let Some(Clip::Text(clip)) = timeline.data.clip_mut(clip_id) {
                clip.properties = properties;
            }
        }
        EditAction::AddTrack { track } => timeline.data.tracks.push(track),
        EditAction::DeleteTrack { track_id } => {
            timeline.data.tracks.retain(|track| track.id != track_id);
            timeline
                .data
                .clips
                .retain(|clip| clip.track_id() != track_id);
        }
        EditAction::MoveTrack { index, target } => timeline.data.tracks.swap(index, target),
        EditAction::ToggleTrackVisibility { track_id } => {
            if let Some(track) = timeline.data.track_mut(track_id) {
                track.visible = !track.visible;
            }
        }
        EditAction::ToggleTrackMute { track_id } => {
            if let Some(track) = timeline.data.track_mut(track_id) {
                track.muted = !track.muted;
            }
        }
        EditAction::ToggleTrackLock { track_id } => {
            if let Some(track) = timeline.data.track_mut(track_id) {
                track.locked = !track.locked;
            }
        }
        EditAction::SetFrameRate { frame_rate } => timeline.data.set_frame_rate(frame_rate),
        EditAction::SetSavedPlayhead { playhead } => {
            timeline.data.view.saved_playhead_frame = playhead.max(TimelineTime::ZERO);
        }
        EditAction::SetScroll {
            horizontal,
            vertical,
        } => {
            timeline.data.view.horizontal_scroll = if horizontal.is_finite() {
                horizontal.max(0.0)
            } else {
                0.0
            };
            timeline.data.view.vertical_scroll = if vertical.is_finite() {
                vertical.max(0.0)
            } else {
                0.0
            };
        }
        EditAction::SetTimelineZoom { pixels_per_second } => {
            timeline.data.view.pixels_per_second = pixels_per_second;
        }
        EditAction::SetSnapping { enabled } => {
            timeline.interaction.snap_guide = None;
            timeline.interaction.snapping_enabled = enabled;
            timeline.data.view.snapping_enabled = enabled;
            return Ok(());
        }
        EditAction::SetTrackMagnet { enabled } => {
            timeline.interaction.magnet_enabled = enabled;
            timeline.data.view.track_magnet_enabled = enabled;
            return Ok(());
        }
        EditAction::UpdateAssetPaths { paths } => {
            for (asset_id, path) in paths {
                if let Some(asset) = timeline
                    .data
                    .assets
                    .iter_mut()
                    .find(|asset| asset.id == asset_id)
                {
                    asset.path = path;
                }
            }
        }
        EditAction::ReplaceTimeline { timeline: data } => {
            timeline.data = data;
        }
    }

    timeline.data.view.saved_playhead_frame = timeline
        .playhead()
        .clamp(TimelineTime::ZERO, timeline.data.content_duration());
    Ok(())
}

fn unlocked_clip_ids(timeline: &TimelineSerialization) -> HashSet<Ulid> {
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

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

#[cfg(test)]
#[path = "tests/editing.test.rs"]
mod tests;
