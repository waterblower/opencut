use anyhow::{Result, anyhow};
use super::*;
use gpui::point;
pub use opencut_player::timeline::{
    FrameRate, TimelineSerialization, TimelineTime, TimelineViewState,
};
use std::{fs, path::Path};

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
    pub path: PathBuf,
    pub(super) video_backend: TimelineVideoBackend,
    pub(super) h_scroll: ScrollHandle,
    pub(super) v_scroll: ScrollHandle,
    pub(super) interaction: TimelineInteractionState,
    pub(super) undo_stack: Vec<TimelineSerialization>,
    pub(super) redo_stack: Vec<TimelineSerialization>,
    pub(super) preview_drop_asset: Option<PreviewDropAsset>,
    // serialized data
    pub data: TimelineSerialization,
}

#[derive(Debug)]
pub struct PreviewDropAsset {
    pub track_id: Ulid,
    pub start_time: TimelineTime,
    pub asset: AssetBeingDragged,
}

pub fn timeline_ranges_overlap(
    left_start: TimelineTime,
    left_end: TimelineTime,
    right_start: TimelineTime,
    right_end: TimelineTime,
) -> bool {
    left_start < right_end && right_start < left_end
}
pub trait TimelineEditorExt: Sized {
    fn load(path: &Path) -> Result<Self>;
    fn save(&self, path: &Path) -> Result<()>;
    fn validate_clip_move_placements(
        &self,
        placements: &[(Ulid, Ulid, TimelineTime)],
        ignored_clip_ids: &HashSet<Ulid>,
    ) -> Result<()>;
    fn set_frame_rate(&mut self, frame_rate: FrameRate);
    fn repair_and_prune_invalid_data(&mut self);
}
impl TimelineEditorExt for TimelineSerialization {
    fn load(path: &Path) -> Result<Self> {
        let contents = match fs::read(path) {
            Ok(contents) => contents,
            Err(error) => {
                return Err(anyhow!(
                    "could not read {}: {error} at {}:{}",
                    path.display(),
                    file!(),
                    line!()
                ));
            }
        };
        let value = match serde_json::from_slice(&contents) {
            Ok(value) => value,
            Err(error) => {
                return Err(anyhow!(
                    "could not parse {}: {error} at {}:{}",
                    path.display(),
                    file!(),
                    line!()
                ));
            }
        };
        let mut timeline = opencut_player::timeline::parse(&value)?;
        timeline.repair_and_prune_invalid_data();
        Ok(timeline)
    }
    fn save(&self, path: &Path) -> Result<()> {
        let Some(directory) = path.parent() else {
            return Err(anyhow!(
                "timeline path has no parent directory at {}:{}",
                file!(),
                line!()
            ));
        };
        if let Err(error) = fs::create_dir_all(directory) {
            return Err(anyhow!(
                "could not create {}: {error} at {}:{}",
                directory.display(),
                file!(),
                line!()
            ));
        }
        let mut bytes = match serde_json::to_vec_pretty(self) {
            Ok(bytes) => bytes,
            Err(error) => {
                return Err(anyhow!(
                    "could not serialize timeline: {error} at {}:{}",
                    file!(),
                    line!()
                ));
            }
        };
        bytes.push(b'\n');
        let temporary = path.with_extension("json.tmp");
        if let Err(error) = fs::write(&temporary, bytes) {
            return Err(anyhow!(
                "could not write {}: {error} at {}:{}",
                temporary.display(),
                file!(),
                line!()
            ));
        }
        if let Err(error) = fs::rename(&temporary, path) {
            return Err(anyhow!(
                "could not replace {}: {error} at {}:{}",
                path.display(),
                file!(),
                line!()
            ));
        }
        Ok(())
    }
    fn validate_clip_move_placements(
        &self,
        placements: &[(Ulid, Ulid, TimelineTime)],
        ignored_clip_ids: &HashSet<Ulid>,
    ) -> Result<()> {
        if placements.is_empty() {
            return Err(ClipPlacementRejection::NoPlacements.into());
        }
        for (clip_id, track_id, start) in placements {
            let Some(clip) = self.clip(*clip_id) else {
                return Err(ClipPlacementRejection::MissingClip.into());
            };
            match clip {
                Clip::Video(media) | Clip::Audio(media) => {
                    let Some(asset) = self.asset(media.asset_id) else {
                        return Err(ClipPlacementRejection::MissingAsset.into());
                    };
                    // An audio clip may use the audio stream of a video asset.
                    let media_kind = if matches!(clip, Clip::Audio(_)) {
                        MediaKind::Audio
                    } else {
                        asset.kind
                    };
                    validate_clip_placement(
                        self,
                        *track_id,
                        media_kind,
                        media.source_out - media.source_in,
                        *start,
                        ignored_clip_ids,
                    )?;
                }
                Clip::Text(clip) => validate_text_clip_placement(
                    self,
                    *track_id,
                    clip.frame_length(self.settings.frame_rate),
                    *start,
                    ignored_clip_ids,
                )?,
            }
        }
        for (index, (clip_id, track_id, start)) in placements.iter().enumerate() {
            let frame_rate = self.settings.frame_rate;
            let duration = self
                .clip(*clip_id)
                .map(|clip| clip.frame_length(frame_rate))
                .ok_or(ClipPlacementRejection::MissingClip)?;
            if placements[index + 1..]
                .iter()
                .any(|(other_id, other_track_id, other_start)| {
                    let other_duration = self
                        .clip(*other_id)
                        .map(|clip| clip.frame_length(frame_rate))
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
                return Err(ClipPlacementRejection::ProposedClipsOverlap.into());
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
                .max(TimelineTime::ONE_FRAME);
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
        self.view.normalize();
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
                || clip.timeline_start() < TimelineTime::ZERO
                || match clip {
                    Clip::Video(clip) | Clip::Audio(clip) => {
                        clip.source_in < TimelineTime::ZERO
                            || clip.source_out - clip.source_in < TimelineTime::ONE_FRAME
                    }
                    Clip::Text(clip) => clip.frame_length(frame_rate) < TimelineTime::ONE_FRAME,
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
                    clip.source_in = clip.source_in.max(TimelineTime::ZERO);
                    clip.source_out = clip
                        .source_out
                        .max(clip.source_in + TimelineTime::ONE_FRAME);
                } else {
                    let asset_duration = frame_rate
                        .nearest(asset.duration)
                        .max(TimelineTime::ONE_FRAME);
                    let maximum_in =
                        (asset_duration - TimelineTime::ONE_FRAME).max(TimelineTime::ZERO);
                    clip.source_in = clip.source_in.clamp(TimelineTime::ZERO, maximum_in);
                    clip.source_out = clip
                        .source_out
                        .clamp(clip.source_in + TimelineTime::ONE_FRAME, asset_duration);
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
            let mut next_available = TimelineTime::ZERO;
            for index in indices {
                let timeline_start = self.clips[index].timeline_start().max(next_available);
                self.clips[index].set_timeline_start(timeline_start);
                next_available = self.clips[index].timeline_end(frame_rate);
            }
        }
    }
}
impl TimelineRuntimeState {
    pub(super) fn new(
        path: PathBuf,
        data: TimelineSerialization,
        ges_timeline: gstreamer_editing_services::Timeline,
    ) -> Result<Self> {
        let scroll = ScrollHandle::new();
        scroll.set_offset(point(px(-data.view.horizontal_scroll), px(0.0)));
        let vertical_scroll = ScrollHandle::new();
        vertical_scroll.set_offset(point(px(0.0), px(-data.view.vertical_scroll)));
        let snapping_enabled = data.view.snapping_enabled;
        let magnet_enabled = data.view.track_magnet_enabled;
        let selected_clip_id = data.clips.first().map(Clip::id);
        let selected_clip_ids = selected_clip_id.into_iter().collect();

        Ok(Self {
            path,
            data,
            video_backend: TimelineVideoBackend::new(ges_timeline)?,
            interaction: TimelineInteractionState {
                active_tool: TimelineTool::Selection,
                snapping_enabled,
                magnet_enabled,
                selected_clip_id,
                selected_clip_ids,
                blade_guide: None,
                snap_guide: None,
                clip_move_drag: None,
                marquee_selection: None,
                scrubbing_playhead: false,
                last_scrub_seek: None,
            },
            h_scroll: scroll,
            v_scroll: vertical_scroll,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            preview_drop_asset: None,
        })
    }

    pub fn playhead(&self) -> TimelineTime {
        self.data
            .settings
            .frame_rate
            .frames_from_duration_nearest(self.video_backend.playback().position())
    }

    pub(super) fn save_timeline_playhead(self: &mut TimelineRuntimeState, project_root: &Path) {
        self.capture_playhead(project_root);
        self.save(project_root);
    }

    pub(super) fn capture_playhead(&mut self, project_root: &Path) {
        edit_timeline(
            self,
            project_root,
            EditAction::SetSavedPlayhead {
                playhead: self.playhead(),
            },
        )
        .expect("saving the playhead cannot be rejected");
    }

    pub(super) fn save(&self, project_root: &Path) {
        if let Err(error) = self.data.save(&project_root.join(&self.path)) {
            eprintln!("Could not autosave timeline: {error}");
        }
    }

    pub(super) fn capture_scroll(&mut self, project_root: &Path) {
        edit_timeline(
            self,
            project_root,
            EditAction::SetScroll {
                horizontal: -f32::from(self.h_scroll.offset().x),
                vertical: -f32::from(self.v_scroll.offset().y),
            },
        )
        .expect("saving timeline scroll cannot be rejected");
    }

    pub(super) fn record_editing_history(&mut self) {
        self.undo_stack.push(self.data.clone());
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
trait TimelineViewExt {
    fn normalize(&mut self);
}
impl TimelineViewExt for TimelineViewState {
    fn normalize(&mut self) {
        self.saved_playhead_frame = self.saved_playhead_frame.max(TimelineTime::ZERO);
        self.horizontal_scroll = finite_nonnegative(self.horizontal_scroll);
        self.vertical_scroll = finite_nonnegative(self.vertical_scroll);
        self.pixels_per_second = if self.pixels_per_second.is_finite() {
            self.pixels_per_second.clamp(
                MIN_TIMELINE_PIXELS_PER_SECOND,
                MAX_TIMELINE_PIXELS_PER_SECOND,
            )
        } else {
            DEFAULT_TIMELINE_PIXELS_PER_SECOND
        };
    }
}
fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}
#[cfg(test)]
#[path = "tests/timeline.test.rs"]
mod tests;
