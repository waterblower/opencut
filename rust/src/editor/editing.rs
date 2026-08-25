use super::*;
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
    ) -> Result<(Vec<Clip>, Vec<MediaAsset>), ClipPlacementRejection> {
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
    pub(super) fn blade_at_playhead(&mut self, preview: &mut PreviewState, project_root: &Path) {
        let clips_to_split = self
            .data
            .clips
            .iter()
            .filter(|clip| {
                let local = self.playhead - clip.timeline_start();
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
            return;
        }
        let removed_clip_ids = clips_to_split.iter().map(Clip::id).collect();
        let split_clips = clips_to_split
            .into_iter()
            .flat_map(|clip| {
                let (left, right) = clip
                    .split_at(self.playhead, self.data.settings.frame_rate)
                    .expect("clips at the playhead must be splittable");
                [left, right]
            })
            .collect();

        self.record_editing_history();
        edit_and_rebuild_timeline(
            preview,
            project_root,
            self,
            EditAction::RemoveClips {
                clip_ids: removed_clip_ids,
                close_track_gaps: false,
            },
        )
        .expect("removing clips cannot be rejected");
        edit_and_rebuild_timeline(
            preview,
            project_root,
            self,
            EditAction::AddClips {
                clips: split_clips,
                assets: Vec::new(),
            },
        )
        .expect("split clip placements were validated before recording history");
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

        edit_and_rebuild_timeline(
            &mut self.preview,
            &self.global_settings.project_root,
            timeline,
            EditAction::AddClips { clips, assets },
        )
        .expect("clipboard placements were validated before recording history");

        self.status = Some(format!("Pasted {count} clip{}.", plural_suffix(count)));
        timeline.save(&self.global_settings.project_root);

        self.load_timeline_position_with_options(playhead, true);
        self.schedule_active_timeline_waveforms(cx);
    }

    fn remove_clips(&mut self, clip_ids: &HashSet<Ulid>, close_track_gaps: bool) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        edit_and_rebuild_timeline(
            &mut self.preview,
            &self.global_settings.project_root,
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

        let clips_empty = timeline.data.clips.is_empty();
        let playhead = timeline.playhead;
        if clips_empty {
            timeline.playhead = TimelineTime::ZERO;
        } else {
            self.load_timeline_position_with_options(playhead, true);
        }
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        timeline.save(&self.global_settings.project_root);
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
            return;
        };
        timeline.record_editing_history();
        timeline.interaction.selected_clip_ids = duplicates.iter().map(Clip::id).collect();
        timeline.interaction.selected_clip_id = primary_index
            .and_then(|index| duplicates.get(index))
            .or_else(|| duplicates.first())
            .map(Clip::id);
        edit_and_rebuild_timeline(
            &mut self.preview,
            &self.global_settings.project_root,
            timeline,
            EditAction::AddClips {
                clips: duplicates,
                assets: Vec::new(),
            },
        )
        .expect("duplicate placements were validated before recording history");
        timeline.save(&self.global_settings.project_root);
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
            TrackKind::Text => "Text",
        };
        let id = Ulid::generate();
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.record_editing_history();
        edit_and_rebuild_timeline(
            &mut self.preview,
            &self.global_settings.project_root,
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
        timeline.save(&self.global_settings.project_root);
    }

    pub(super) fn toggle_track_lock(&mut self, track_id: Ulid) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.record_editing_history();
        edit_and_rebuild_timeline(
            &mut self.preview,
            &self.global_settings.project_root,
            timeline,
            EditAction::ToggleTrackLock { track_id },
        )
        .expect("toggling a track lock cannot be rejected");
        timeline.save(&self.global_settings.project_root);
    }

    pub(super) fn toggle_track_visibility(&mut self, track_id: Ulid) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        let playhead = timeline.playhead;
        timeline.record_editing_history();
        edit_and_rebuild_timeline(
            &mut self.preview,
            &self.global_settings.project_root,
            timeline,
            EditAction::ToggleTrackVisibility { track_id },
        )
        .expect("toggling track visibility cannot be rejected");
        timeline.save(&self.global_settings.project_root);

        self.load_timeline_position_with_options(playhead, true);
    }

    pub(super) fn toggle_track_mute(&mut self, track_id: Ulid) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        let playhead = timeline.playhead;
        timeline.record_editing_history();
        edit_and_rebuild_timeline(
            &mut self.preview,
            &self.global_settings.project_root,
            timeline,
            EditAction::ToggleTrackMute { track_id },
        )
        .expect("toggling track mute cannot be rejected");
        timeline.save(&self.global_settings.project_root);

        self.load_timeline_position_with_options(playhead, true);
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
        edit_and_rebuild_timeline(
            &mut self.preview,
            &self.global_settings.project_root,
            timeline,
            EditAction::MoveTrack { index, target },
        )
        .expect("moving a track cannot be rejected");
        timeline.save(&self.global_settings.project_root);

        self.load_timeline_position_with_options(playhead, true);
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
        edit_and_rebuild_timeline(
            &mut self.preview,
            &self.global_settings.project_root,
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
        timeline.save(&self.global_settings.project_root);

        self.load_timeline_position_with_options(playhead, true);
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

    pub(super) fn undo(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        let Some(mut snapshot) = timeline.undo_stack.pop() else {
            return;
        };
        snapshot.view = timeline.data.view.clone();
        let current = timeline.data.clone();
        edit_and_rebuild_timeline(
            &mut self.preview,
            &self.global_settings.project_root,
            timeline,
            EditAction::ReplaceTimeline { timeline: snapshot },
        )
        .expect("restoring history cannot be rejected");
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
        let current = timeline.data.clone();
        edit_and_rebuild_timeline(
            &mut self.preview,
            &self.global_settings.project_root,
            timeline,
            EditAction::ReplaceTimeline { timeline: snapshot },
        )
        .expect("restoring history cannot be rejected");
        timeline.undo_stack.push(current);
        self.reset_after_history_change();
    }

    pub(super) fn reset_after_history_change(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };

        timeline.playhead = timeline
            .playhead
            .clamp(TimelineTime::ZERO, timeline.data.content_duration());
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
        let has_clips = !timeline.data.clips.is_empty();
        let playhead = timeline.playhead;
        self.properties.transform_input_clip_id = None;
        self.properties.text_input_clip_id = None;
        if has_clips {
            self.load_timeline_position_with_options(playhead, true);
        }
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        timeline.save(&self.global_settings.project_root);
    }

    pub(super) fn save_timeline_playhead(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.capture_playhead(&self.global_settings.project_root);
        timeline.save(&self.global_settings.project_root);
    }

    pub(super) fn save_timeline_scroll(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.capture_scroll(&self.global_settings.project_root);
        timeline.save(&self.global_settings.project_root);
    }

    pub(super) fn toggle_track_magnet(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.interaction.magnet_enabled = !timeline.interaction.magnet_enabled;
        edit_and_rebuild_timeline(
            &mut self.preview,
            &self.global_settings.project_root,
            timeline,
            EditAction::SetTrackMagnet {
                enabled: timeline.interaction.magnet_enabled,
            },
        )
        .expect("changing the track magnet preference cannot be rejected");
        timeline.save(&self.global_settings.project_root);
    }
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

fn validate_clips_placements(
    timeline: &TimelineSerialization,
    clips: &[Clip],
) -> Result<(), ClipPlacementRejection> {
    if clips.is_empty() {
        return Err(ClipPlacementRejection::NoPlacements);
    }
    for clip in clips {
        match clip {
            Clip::Video(clip) | Clip::Audio(clip) => {
                let Some(asset) = timeline.asset(clip.asset_id) else {
                    return Err(ClipPlacementRejection::MissingAsset);
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
            return Err(ClipPlacementRejection::ProposedClipsOverlap);
        }
    }
    Ok(())
}

fn plural_suffix(count: usize) -> &'static str {
    if count == 1 { "" } else { "s" }
}

pub(super) enum EditAction {
    AddClips {
        clips: Vec<Clip>,
        assets: Vec<MediaAsset>,
    },
    RemoveClips {
        clip_ids: HashSet<Ulid>,
        close_track_gaps: bool,
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
    SetTextLength {
        clip_id: Ulid,
        length: Duration,
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

impl EditAction {
    fn kind(&self) -> &'static str {
        match self {
            Self::AddClips { .. } => "AddClips",
            Self::RemoveClips { .. } => "RemoveClips",
            Self::MoveClips { .. } => "MoveClips",
            Self::SetVideoProperties { .. } => "SetVideoProperties",
            Self::SetTextProperties { .. } => "SetTextProperties",
            Self::SetTextLength { .. } => "SetTextLength",
            Self::AddTrack { .. } => "AddTrack",
            Self::DeleteTrack { .. } => "DeleteTrack",
            Self::MoveTrack { .. } => "MoveTrack",
            Self::ToggleTrackVisibility { .. } => "ToggleTrackVisibility",
            Self::ToggleTrackMute { .. } => "ToggleTrackMute",
            Self::ToggleTrackLock { .. } => "ToggleTrackLock",
            Self::SetFrameRate { .. } => "SetFrameRate",
            Self::SetSavedPlayhead { .. } => "SetSavedPlayhead",
            Self::SetScroll { .. } => "SetScroll",
            Self::SetTimelineZoom { .. } => "SetTimelineZoom",
            Self::SetSnapping { .. } => "SetSnapping",
            Self::SetTrackMagnet { .. } => "SetTrackMagnet",
            Self::UpdateAssetPaths { .. } => "UpdateAssetPaths",
            Self::ReplaceTimeline { .. } => "ReplaceTimeline",
        }
    }
}

pub(super) fn edit_and_rebuild_timeline(
    preview: &mut PreviewState,
    project_root: &Path,
    timeline: &mut TimelineRuntimeState,
    action: EditAction,
) -> Result<(), EditError> {
    let action_type = action.kind();
    let t = Instant::now();
    let rebuild_timeline = edit_timeline(timeline, project_root, action)?;
    eprintln!("edit_timeline {action_type} - {:?}", t.elapsed());
    data_parity_check(&timeline, &timeline.ges_timeline)?;

    if !rebuild_timeline {
        return Ok(());
    }
    eprintln!("rebuild the timeline is slow");
    let volume = match &preview.target {
        PreviewTarget::Timeline(video) => video.volume(),
        _ => 1.0,
    };
    if let Some(video) = preview.target.video() {
        video.set_paused(true);
    }
    preview.target = PreviewTarget::None;
    timeline.ges_timeline = build_ges_timeline(
        &timeline.data,
        project_root,
        export::ExportOptions::from_timeline(&timeline.data),
    )?;
    timeline.playhead = timeline
        .playhead
        .clamp(TimelineTime::ZERO, timeline.data.content_duration());
    if timeline.data.clips.is_empty() {
        return Ok(());
    }

    let mut video = create_timeline_video_v2(&timeline.ges_timeline).unwrap();
    video.set_volume(volume);
    video.set_muted(volume <= f64::EPSILON);
    let _ = video.seek(timeline.data.duration(timeline.playhead), true);
    preview.target = PreviewTarget::Timeline(video);
    Ok(())
}

pub(super) fn edit_timeline(
    timeline: &mut TimelineRuntimeState,
    project_root: &Path,
    action: EditAction,
) -> Result<bool, EditError> {
    match action {
        EditAction::AddClips { clips, assets } => {
            let mut updated_timeline = timeline.data.clone();
            updated_timeline.assets.extend(assets);
            validate_clips_placements(&updated_timeline, &clips)?;
            updated_timeline.clips.extend(clips.iter().cloned());
            ges_add_clips(
                &timeline.ges_timeline,
                &updated_timeline,
                project_root,
                &clips,
            )?;
            timeline.data = updated_timeline;
            return Ok(false);
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
            ges_remove_clips(&timeline.ges_timeline, &clip_ids)?;
            return Ok(false);
        }
        EditAction::MoveClips { placements } => {
            let t = Instant::now();
            let moved_clip_ids = placements
                .iter()
                .map(|(clip_id, _, _)| *clip_id)
                .collect::<HashSet<_>>();
            timeline
                .data
                .validate_clip_move_placements(&placements, &moved_clip_ids)?;

            ges_move_clips(&timeline.ges_timeline, &timeline.data, &placements)?;

            for (clip_id, track_id, start) in placements {
                if let Some(clip) = timeline.data.clip_mut(clip_id) {
                    clip.set_timeline_start(start);
                    clip.set_track_id(track_id);
                }
            }
            eprintln!("EditAction::MoveClips {:?}", t.elapsed());

            return Ok(false);
        }
        EditAction::SetVideoProperties {
            clip_ids,
            properties,
        } => {
            for clip_id in clip_ids {
                if let Some(clip) = timeline.data.clip_mut(clip_id).and_then(Clip::media_mut) {
                    clip.video_properties = properties;
                }
            }
        }
        EditAction::SetTextProperties {
            clip_id,
            properties,
        } => {
            if let Some(Clip::Text(clip)) = timeline.data.clip_mut(clip_id) {
                clip.properties = properties;
                // change the text of a text clip
                ges_change_text_clip(
                    &timeline.ges_timeline,
                    clip_id,
                    &clip.properties,
                    clip.length,
                )?;
            }
            return Ok(false); // should not rebuild timeline
        }
        EditAction::SetTextLength { clip_id, length } => {
            let Some(Clip::Text(clip)) = timeline.data.clip(clip_id) else {
                return Err(ClipPlacementRejection::MissingClip.into());
            };
            let frame_rate = timeline.data.settings.frame_rate;
            validate_text_clip_placement(
                &timeline.data,
                clip.track_id,
                frame_rate.frames_from_duration_nearest(length),
                clip.timeline_start,
                &HashSet::from([clip_id]),
            )?;
            let properties = clip.properties.clone();
            ges_change_text_clip(&timeline.ges_timeline, clip_id, &properties, length)?;
            let Some(Clip::Text(clip)) = timeline.data.clip_mut(clip_id) else {
                return Err(ClipPlacementRejection::MissingClip.into());
            };
            clip.length = length;
            return Ok(false);
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
        EditAction::SetSnapping { enabled } => timeline.data.view.snapping_enabled = enabled,
        EditAction::SetTrackMagnet { enabled } => {
            timeline.data.view.track_magnet_enabled = enabled;
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
        EditAction::ReplaceTimeline { timeline: data } => timeline.data = data,
    }

    Ok(true)
}

#[derive(Debug)]
pub(super) enum EditError {
    ClipPlacement(ClipPlacementRejection),
    Preview(String),
}

impl std::fmt::Display for EditError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClipPlacement(rejection) => formatter.write_str(rejection.message()),
            Self::Preview(error) => formatter.write_str(error),
        }
    }
}

impl std::error::Error for EditError {}

impl From<ClipPlacementRejection> for EditError {
    fn from(error: ClipPlacementRejection) -> Self {
        Self::ClipPlacement(error)
    }
}

impl From<String> for EditError {
    fn from(error: String) -> Self {
        Self::Preview(error)
    }
}

fn ges_move_clips(
    ges: &gstreamer_editing_services::Timeline,
    timeline: &TimelineSerialization,
    placements: &[(Ulid, Ulid, TimelineTime)],
) -> Result<(), String> {
    use gstreamer_editing_services::prelude::*;

    if placements.is_empty() {
        return Ok(());
    }

    let layers = ges.layers();
    let ordered_tracks = timeline
        .tracks
        .iter()
        .filter(|track| track.kind == TrackKind::Text)
        .chain(
            timeline
                .tracks
                .iter()
                .filter(|track| track.kind != TrackKind::Text),
        )
        .collect::<Vec<_>>();
    let clips_by_name = layers
        .iter()
        .enumerate()
        .flat_map(|(layer_index, layer)| {
            layer
                .clips()
                .into_iter()
                .map(move |clip| (layer_index, clip))
        })
        .filter_map(|(layer_index, clip)| Some((clip.name()?.to_string(), (layer_index, clip))))
        .collect::<HashMap<_, _>>();
    let clock_time = |time| {
        let duration = timeline.duration(time);
        gstreamer::ClockTime::from_nseconds(duration.as_nanos().min(u64::MAX as u128) as u64)
    };

    let mut moves = Vec::new();
    for (clip_id, track_id, start) in placements {
        let layer_index = ordered_tracks
            .iter()
            .position(|track| track.id == *track_id)
            .ok_or_else(|| format!("Track {track_id} has no GES layer."))?;
        if layers.get(layer_index).is_none() {
            return Err(format!("Track {track_id} has no GES layer."));
        }
        let clip_name = format!("opencut-clip-{clip_id}");
        let Some((original_layer_index, clip)) = clips_by_name.get(&clip_name) else {
            continue;
        };
        moves.push((
            *clip_id,
            clip.clone(),
            *original_layer_index,
            layer_index,
            clock_time(*start),
        ));
    }

    if moves.len() > 1 {
        let parking_gap = clock_time(TimelineTime::ONE_FRAME).nseconds().max(1);
        let mut parking_start = layers
            .iter()
            .flat_map(|layer| layer.clips())
            .map(|clip| {
                clip.start()
                    .nseconds()
                    .saturating_add(clip.duration().nseconds())
            })
            .max()
            .unwrap_or(0)
            .saturating_add(parking_gap);
        for (clip_id, clip, original_layer_index, _, _) in &moves {
            clip.edit_full(
                *original_layer_index as i64,
                gstreamer_editing_services::EditMode::Normal,
                gstreamer_editing_services::Edge::None,
                parking_start,
            )
            .map_err(|error| format!("could not stage GES clip {clip_id}: {error}"))?;
            parking_start = parking_start
                .saturating_add(clip.duration().nseconds())
                .saturating_add(parking_gap);
        }
    }

    for (clip_id, clip, _, layer_index, start) in &moves {
        clip.edit_full(
            *layer_index as i64,
            gstreamer_editing_services::EditMode::Normal,
            gstreamer_editing_services::Edge::None,
            start.nseconds(),
        )
        .map_err(|error| format!("could not move GES clip {clip_id}: {error}"))?;
    }

    let placements_by_clip = placements
        .iter()
        .map(|(clip_id, _, start)| (*clip_id, *start))
        .collect::<HashMap<_, _>>();
    let content_duration = timeline
        .clips
        .iter()
        .map(|clip| {
            placements_by_clip
                .get(&clip.id())
                .copied()
                .unwrap_or_else(|| clip.timeline_start())
                + clip.frame_length(timeline.settings.frame_rate)
        })
        .max()
        .map(clock_time)
        .unwrap_or(gstreamer::ClockTime::ZERO);
    if let Some(background) = layers
        .iter()
        .flat_map(|layer| layer.clips())
        .find(|clip| clip.name().as_deref() == Some("opencut-black-background"))
        && !background.set_duration(content_duration)
    {
        return Err("could not update the GES timeline background duration".to_string());
    }
    if !ges.commit() {
        return Err("GStreamer could not commit the moved clips.".to_string());
    }
    Ok(())
}

// change the text of a text clip in the ges timeline
fn ges_change_text_clip(
    ges: &gstreamer_editing_services::Timeline,
    clip_id: Ulid,
    properties: &TextClipProperties,
    length: Duration,
) -> Result<(), String> {
    use gstreamer_editing_services::prelude::*;

    let clip_name = format!("opencut-clip-{clip_id}");
    let clip = ges
        .layers()
        .into_iter()
        .flat_map(|layer| layer.clips())
        .find(|clip| clip.name().as_deref() == Some(clip_name.as_str()))
        .ok_or_else(|| format!("timeline preview has no text clip for {clip_id}"))?;
    let overlay = clip
        .downcast::<gstreamer_editing_services::TextOverlayClip>()
        .map_err(|_| format!("timeline preview clip {clip_id} is not a text clip"))?;
    overlay.set_text(Some(&properties.text));
    overlay.set_font_desc(Some(&format!(
        "{} {}px",
        properties.font, properties.font_size
    )));
    overlay.set_color(properties.color);
    overlay.set_halign(gstreamer_editing_services::TextHAlign::Position);
    overlay.set_valign(gstreamer_editing_services::TextVAlign::Position);
    overlay.set_xpos(properties.position_x);
    overlay.set_ypos(properties.position_y);
    if !overlay.set_duration(gstreamer::ClockTime::from_nseconds(
        length.as_nanos().min(u64::MAX as u128) as u64,
    )) {
        return Err(format!(
            "could not change the duration of GES text clip {clip_id}"
        ));
    }
    let content_duration_ns = ges
        .layers()
        .into_iter()
        .flat_map(|layer| layer.clips())
        .filter(|clip| clip.name().as_deref() != Some("opencut-black-background"))
        .map(|clip| {
            clip.start()
                .nseconds()
                .saturating_add(clip.duration().nseconds())
        })
        .max()
        .unwrap_or(0);
    if let Some(background) = ges
        .layers()
        .into_iter()
        .flat_map(|layer| layer.clips())
        .find(|clip| clip.name().as_deref() == Some("opencut-black-background"))
        && !background.set_duration(gstreamer::ClockTime::from_nseconds(content_duration_ns))
    {
        return Err("could not update the GES timeline background duration".to_string());
    }
    if !ges.commit() {
        return Err("GStreamer could not commit the preview text change.".to_string());
    }
    Ok(())
}

fn ges_remove_clips(
    ges: &gstreamer_editing_services::Timeline,
    clips: &HashSet<Ulid>,
) -> Result<(), String> {
    use gstreamer_editing_services::prelude::*;

    if clips.is_empty() {
        return Ok(());
    }
    let clip_names = clips
        .iter()
        .map(|clip_id| format!("opencut-clip-{clip_id}"))
        .collect::<HashSet<_>>();
    let clips_to_remove = ges
        .layers()
        .into_iter()
        .flat_map(|layer| {
            layer
                .clips()
                .into_iter()
                .filter(|clip| {
                    clip.name()
                        .is_some_and(|name| clip_names.contains(name.as_str()))
                })
                .map(move |clip| (layer.clone(), clip))
        })
        .collect::<Vec<_>>();

    for (layer, clip) in clips_to_remove {
        let name = clip
            .name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| "<unnamed>".to_string());
        layer
            .remove_clip(&clip)
            .map_err(|error| format!("could not remove GES clip {name}: {error}"))?;
    }

    let mut background = None;
    let mut content_duration_ns = 0;
    for layer in ges.layers() {
        for clip in layer.clips() {
            if clip.name().as_deref() == Some("opencut-black-background") {
                background = Some((layer.clone(), clip));
                continue;
            }
            content_duration_ns = content_duration_ns.max(
                clip.start()
                    .nseconds()
                    .saturating_add(clip.duration().nseconds()),
            );
        }
    }
    if let Some((background_layer, background)) = background {
        if content_duration_ns == 0 {
            background_layer
                .remove_clip(&background)
                .map_err(|error| format!("could not remove the GES background: {error}"))?;
        } else if !background.set_duration(gstreamer::ClockTime::from_nseconds(content_duration_ns))
        {
            return Err("could not update the GES timeline background duration".to_string());
        }
    }
    if !ges.commit() {
        return Err("GStreamer could not commit the removed clips.".to_string());
    }
    Ok(())
}

fn ges_add_clips(
    ges: &gstreamer_editing_services::Timeline,
    timeline: &TimelineSerialization,
    project_root: &Path,
    clips: &[Clip],
) -> Result<(), String> {
    use gstreamer_editing_services::prelude::*;

    let layers = ges.layers();
    let options = super::export::ExportOptions::from_timeline(timeline);
    let output_scale = (options.width.max(2) as f64 / timeline.settings.width.max(2) as f64)
        .min(options.height.max(2) as f64 / timeline.settings.height.max(2) as f64);
    let clock_time = |duration: Duration| {
        gstreamer::ClockTime::from_nseconds(duration.as_nanos().min(u64::MAX as u128) as u64)
    };
    let ordered_tracks = timeline
        .tracks
        .iter()
        .filter(|track| track.kind == TrackKind::Text)
        .chain(
            timeline
                .tracks
                .iter()
                .filter(|track| track.kind != TrackKind::Text),
        )
        .collect::<Vec<_>>();
    let mut uri_assets = HashMap::<Ulid, gstreamer_editing_services::UriClipAsset>::new();

    for clip in clips {
        let timeline_track = timeline
            .track(clip.track_id())
            .ok_or_else(|| format!("Clip {} has no timeline track.", clip.id()))?;
        let layer_index = ordered_tracks
            .iter()
            .position(|track| track.id == timeline_track.id)
            .ok_or_else(|| format!("Clip {} has no GES layer.", clip.id()))?;
        let layer = layers
            .get(layer_index)
            .ok_or_else(|| format!("Clip {} has no GES layer.", clip.id()))?;

        if timeline_track.kind == TrackKind::Text {
            if !timeline_track.visible {
                continue;
            }
            let text = clip
                .text()
                .ok_or_else(|| format!("Media clip {} is on a text track.", clip.id()))?;
            let overlay = gstreamer_editing_services::TextOverlayClip::new()
                .ok_or_else(|| format!("could not create text clip {}", clip.id()))?;
            let font_size = (text.properties.font_size * output_scale).clamp(1.0, 1000.0);
            overlay.set_text(Some(&text.properties.text));
            overlay.set_font_desc(Some(&format!("{} {font_size}px", text.properties.font)));
            overlay.set_color(text.properties.color);
            overlay.set_halign(gstreamer_editing_services::TextHAlign::Position);
            overlay.set_valign(gstreamer_editing_services::TextVAlign::Position);
            overlay.set_xpos(text.properties.position_x);
            overlay.set_ypos(text.properties.position_y);
            overlay
                .set_name(Some(&format!("opencut-clip-{}", clip.id())))
                .map_err(|error| format!("could not identify clip {}: {error}", clip.id()))?;
            if !overlay.set_start(clock_time(timeline.duration(clip.timeline_start()))) {
                return Err(format!("could not set text clip {} start", clip.id()));
            }
            if !overlay.set_duration(clock_time(
                timeline.duration(clip.frame_length(timeline.settings.frame_rate)),
            )) {
                return Err(format!("could not set text clip {} duration", clip.id()));
            }
            layer.add_clip(&overlay).map_err(|error| {
                format!(
                    "could not add text clip {} to the GES timeline: {error}",
                    clip.id()
                )
            })?;
            continue;
        }

        let media = clip
            .media()
            .ok_or_else(|| format!("Text clip {} is on a media track.", clip.id()))?;
        let asset = timeline
            .asset(media.asset_id)
            .ok_or_else(|| format!("Clip {} has no source media.", clip.id()))?;
        let mut track_types = gstreamer_editing_services::TrackType::empty();
        if timeline_track.kind == TrackKind::Video
            && timeline_track.visible
            && matches!(asset.kind, MediaKind::Video | MediaKind::Image)
        {
            track_types |= gstreamer_editing_services::TrackType::VIDEO;
        }
        if asset.has_audio
            && !super::clip_render_plan::resolve_audio_clip_render_plan(
                timeline_track.muted,
                media.audio_properties,
            )
            .muted
        {
            track_types |= gstreamer_editing_services::TrackType::AUDIO;
        }
        if track_types.is_empty() {
            continue;
        }

        let uri_asset = if let Some(uri_asset) = uri_assets.get(&asset.id) {
            uri_asset.clone()
        } else {
            let source = project_root.join(&asset.path);
            let uri = url::Url::from_file_path(&source)
                .map_err(|_| format!("could not convert {} to a file URL", source.display()))?;
            let uri_asset = gstreamer_editing_services::UriClipAsset::request_sync(uri.as_str())
                .map_err(|error| format!("could not inspect {}: {error}", source.display()))?;
            uri_assets.insert(asset.id, uri_asset.clone());
            uri_asset
        };
        let start = clock_time(timeline.duration(clip.timeline_start()));
        let inpoint = if asset.kind == MediaKind::Image {
            gstreamer::ClockTime::ZERO
        } else if track_types.contains(gstreamer_editing_services::TrackType::VIDEO) {
            clock_time(Duration::from_secs_f64(timeline.source_start_seconds(clip)))
        } else {
            clock_time(timeline.audio_duration(media.source_in))
        };
        let duration =
            clock_time(timeline.duration(clip.frame_length(timeline.settings.frame_rate)));
        let ges_clip = layer
            .add_asset(&uri_asset, start, inpoint, duration, track_types)
            .map_err(|error| {
                format!("could not add {} to the GES timeline: {error}", asset.name)
            })?;
        ges_clip
            .set_name(Some(&format!("opencut-clip-{}", clip.id())))
            .map_err(|error| format!("could not identify clip {}: {error}", clip.id()))?;
        if track_types.contains(gstreamer_editing_services::TrackType::VIDEO) {
            super::export_gstreamer::apply_video_transform(
                &ges_clip,
                timeline,
                asset,
                options,
                media.video_properties,
            )?;
        }
        if track_types.contains(gstreamer_editing_services::TrackType::AUDIO) {
            let audio_plan = super::clip_render_plan::resolve_audio_clip_render_plan(
                timeline_track.muted,
                media.audio_properties,
            );
            let _ = ges_clip.set_child_property("volume", audio_plan.gain_linear);
        }
    }

    let content_duration = clock_time(timeline.duration(timeline.content_duration()));
    if let Some(background) = ges
        .layers()
        .into_iter()
        .flat_map(|layer| layer.clips())
        .find(|clip| clip.name().as_deref() == Some("opencut-black-background"))
    {
        if !background.set_duration(content_duration) {
            return Err("could not update the GES timeline background duration".to_string());
        }
    } else if !content_duration.is_zero() {
        let background_layer = ges.append_layer();
        let background = gstreamer_editing_services::TestClip::new()
            .ok_or_else(|| "could not create the GES timeline background".to_string())?;
        background.set_supported_formats(gstreamer_editing_services::TrackType::VIDEO);
        background.set_vpattern(gstreamer_editing_services::VideoTestPattern::Black);
        background.set_mute(true);
        background
            .set_name(Some("opencut-black-background"))
            .map_err(|error| format!("could not identify the GES background: {error}"))?;
        if !background.set_duration(content_duration) {
            return Err("could not set the GES timeline background duration".to_string());
        }
        background_layer
            .add_clip(&background)
            .map_err(|error| format!("could not add the GES timeline background: {error}"))?;
    }
    if !ges.commit() {
        return Err("GStreamer could not commit the added clips.".to_string());
    }
    Ok(())
}

#[allow(dead_code)] // Diagnostic utility for checking asynchronous GES commits.
pub(super) fn data_parity_check(
    timeline_runtime: &TimelineRuntimeState,
    ges_timeline: &gstreamer_editing_services::Timeline,
) -> Result<(), String> {
    use gstreamer_editing_services::prelude::*;

    let timeline = &timeline_runtime.data;
    let layers = ges_timeline.layers();
    let ordered_tracks = timeline
        .tracks
        .iter()
        .filter(|track| track.kind == TrackKind::Text)
        .chain(
            timeline
                .tracks
                .iter()
                .filter(|track| track.kind != TrackKind::Text),
        )
        .collect::<Vec<_>>();
    let mut ges_clips = HashMap::new();
    let mut backgrounds = Vec::new();
    for (layer_index, layer) in layers.iter().enumerate() {
        for clip in layer.clips() {
            let Some(name) = clip.name() else {
                return Err(format!("GES layer {layer_index} contains an unnamed clip"));
            };
            if name.as_str() == "opencut-black-background" {
                backgrounds.push(clip);
                continue;
            }
            let Some(id) = name.strip_prefix("opencut-clip-") else {
                return Err(format!(
                    "GES layer {layer_index} contains unexpected clip `{name}`"
                ));
            };
            let id = id
                .parse::<Ulid>()
                .map_err(|error| format!("GES clip `{name}` has an invalid ID: {error}"))?;
            if ges_clips.insert(id, (layer_index, clip)).is_some() {
                return Err(format!("GES contains duplicate clip {id}"));
            }
        }
    }

    let clock_time = |duration: Duration| {
        gstreamer::ClockTime::from_nseconds(duration.as_nanos().min(u64::MAX as u128) as u64)
    };
    let frame_rate = timeline.settings.frame_rate;
    let clock_time_frame = |time: gstreamer::ClockTime| {
        frame_rate.frames_from_duration_nearest(Duration::from_nanos(time.nseconds()))
    };
    for clip in &timeline.clips {
        let clip_id = clip.id();
        let track = timeline
            .track(clip.track_id())
            .ok_or_else(|| format!("Timeline clip {clip_id} has no track"))?;
        let expected_layer = ordered_tracks
            .iter()
            .position(|candidate| candidate.id == track.id)
            .ok_or_else(|| format!("Timeline track {} has no GES layer mapping", track.id))?;
        if layers.get(expected_layer).is_none() {
            return Err(format!(
                "Timeline track {} expects missing GES layer {expected_layer}",
                track.id
            ));
        }

        let (expected_rendered, expected_formats, expected_inpoint) = match clip {
            Clip::Text(_) => (
                track.kind == TrackKind::Text && track.visible,
                None,
                gstreamer::ClockTime::ZERO,
            ),
            Clip::Video(media) | Clip::Audio(media) => {
                let asset = timeline
                    .asset(media.asset_id)
                    .ok_or_else(|| format!("Timeline clip {clip_id} has no media asset"))?;
                let mut formats = gstreamer_editing_services::TrackType::empty();
                if track.kind == TrackKind::Video
                    && track.visible
                    && matches!(asset.kind, MediaKind::Video | MediaKind::Image)
                {
                    formats |= gstreamer_editing_services::TrackType::VIDEO;
                }
                if asset.has_audio
                    && !super::clip_render_plan::resolve_audio_clip_render_plan(
                        track.muted,
                        media.audio_properties,
                    )
                    .muted
                {
                    formats |= gstreamer_editing_services::TrackType::AUDIO;
                }
                let inpoint = if asset.kind == MediaKind::Image {
                    gstreamer::ClockTime::ZERO
                } else if formats.contains(gstreamer_editing_services::TrackType::VIDEO) {
                    clock_time(Duration::from_secs_f64(timeline.source_start_seconds(clip)))
                } else {
                    clock_time(timeline.audio_duration(media.source_in))
                };
                (!formats.is_empty(), Some(formats), inpoint)
            }
        };

        let rendered = ges_clips.remove(&clip_id);
        if !expected_rendered {
            if rendered.is_some() {
                return Err(format!(
                    "Timeline clip {clip_id} should not be rendered, but GES contains it"
                ));
            }
            continue;
        }
        let Some((actual_layer, rendered)) = rendered else {
            return Err(format!("GES is missing timeline clip {clip_id}"));
        };
        if actual_layer != expected_layer {
            return Err(format!(
                "Clip {clip_id} is on GES layer {actual_layer}, expected {expected_layer}"
            ));
        }
        let expected_start = clock_time(timeline.duration(clip.timeline_start()));
        if clock_time_frame(rendered.start()) != clip.timeline_start() {
            return Err(format!(
                "Clip {clip_id} starts at {} ns (frame {}) in GES, expected {} ns (frame {})",
                rendered.start().nseconds(),
                clock_time_frame(rendered.start()).frames(),
                expected_start.nseconds(),
                clip.timeline_start().frames()
            ));
        }
        let expected_duration_frames = clip.frame_length(frame_rate);
        let expected_duration = clock_time(timeline.duration(expected_duration_frames));
        if clock_time_frame(rendered.duration()) != expected_duration_frames {
            return Err(format!(
                "Clip {clip_id} lasts {} ns ({} frames) in GES, expected {} ns ({} frames)",
                rendered.duration().nseconds(),
                clock_time_frame(rendered.duration()).frames(),
                expected_duration.nseconds(),
                expected_duration_frames.frames()
            ));
        }
        if clock_time_frame(rendered.inpoint()) != clock_time_frame(expected_inpoint) {
            return Err(format!(
                "Clip {clip_id} has in-point {} ns (frame {}) in GES, expected {} ns (frame {})",
                rendered.inpoint().nseconds(),
                clock_time_frame(rendered.inpoint()).frames(),
                expected_inpoint.nseconds(),
                clock_time_frame(expected_inpoint).frames()
            ));
        }
        if matches!(clip, Clip::Text(_))
            && !rendered.is::<gstreamer_editing_services::TextOverlayClip>()
        {
            return Err(format!("Clip {clip_id} is not a GES text overlay"));
        }
        if let Some(expected_formats) = expected_formats
            && rendered.supported_formats() != expected_formats
        {
            return Err(format!(
                "Clip {clip_id} has GES formats {:?}, expected {:?}",
                rendered.supported_formats(),
                expected_formats
            ));
        }
    }

    if let Some((clip_id, _)) = ges_clips.into_iter().next() {
        return Err(format!("GES contains unexpected clip {clip_id}"));
    }
    let expected_background_duration = clock_time(timeline.duration(timeline.content_duration()));
    match backgrounds.as_slice() {
        [] if expected_background_duration.is_zero() => {}
        [] => return Err("GES is missing the black background clip".to_string()),
        [_] if expected_background_duration.is_zero() => {
            return Err("GES contains a black background for an empty timeline".to_string());
        }
        [background] if background.start() != gstreamer::ClockTime::ZERO => {
            return Err(format!(
                "GES background starts at {} ns, expected 0 ns",
                background.start().nseconds()
            ));
        }
        [background]
            if clock_time_frame(background.duration())
                != clock_time_frame(expected_background_duration) =>
        {
            return Err(format!(
                "GES background lasts {} ns ({} frames), expected {} ns ({} frames)",
                background.duration().nseconds(),
                clock_time_frame(background.duration()).frames(),
                expected_background_duration.nseconds(),
                clock_time_frame(expected_background_duration).frames()
            ));
        }
        [_] => {}
        _ => return Err("GES contains multiple black background clips".to_string()),
    }

    Ok(())
}

#[cfg(test)]
#[path = "editing.test.rs"]
mod tests;
