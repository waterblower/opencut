use super::model::{MediaAsset, MediaKind, deserialize_ulid};
use super::*;
use gpui::point;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    ops::{Add, AddAssign, Sub, SubAssign},
    path::Path,
    time::Duration,
};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub(super) struct TimelineTime(i64);

impl TimelineTime {
    pub const ZERO: Self = Self(0);
    pub const ONE_FRAME: Self = Self(1);

    pub const fn from_frames(frames: i64) -> Self {
        Self(frames)
    }

    pub const fn frames(self) -> i64 {
        self.0
    }

    pub fn abs_diff(self, other: Self) -> u64 {
        self.0.abs_diff(other.0)
    }
}

impl Add for TimelineTime {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_add(rhs.0))
    }
}

impl AddAssign for TimelineTime {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for TimelineTime {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl SubAssign for TimelineTime {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct FrameRate {
    pub numerator: u32,
    pub denominator: u32,
}

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

impl Default for FrameRate {
    fn default() -> Self {
        Self {
            numerator: 30,
            denominator: 1,
        }
    }
}

impl FrameRate {
    pub const fn new(numerator: u32, denominator: u32) -> Self {
        Self {
            numerator,
            denominator,
        }
    }

    pub fn label(self) -> String {
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

    pub fn frames_per_second(self) -> f64 {
        self.numerator as f64 / self.denominator.max(1) as f64
    }

    pub fn seconds(self, time: TimelineTime) -> f64 {
        time.frames() as f64 * self.denominator.max(1) as f64 / self.numerator.max(1) as f64
    }

    pub fn duration(self, time: TimelineTime) -> Duration {
        let frames = time.frames().max(0) as u128;
        let numerator = frames
            .saturating_mul(self.denominator.max(1) as u128)
            .saturating_mul(1_000_000_000);
        let nanos = divide_round(numerator, self.numerator.max(1) as u128);
        Duration::from_nanos(nanos.min(u64::MAX as u128) as u64)
    }

    pub fn floor_duration(self, duration: Duration) -> TimelineTime {
        let numerator = duration
            .as_nanos()
            .saturating_mul(self.numerator.max(1) as u128);
        let denominator = (self.denominator.max(1) as u128).saturating_mul(1_000_000_000);
        TimelineTime::from_frames((numerator / denominator).min(i64::MAX as u128) as i64)
    }

    pub fn audio_samples(self, time: TimelineTime, sample_rate: u32) -> u64 {
        let frames = time.frames().max(0) as u128;
        let numerator = frames
            .saturating_mul(self.denominator.max(1) as u128)
            .saturating_mul(sample_rate as u128);
        divide_round(numerator, self.numerator.max(1) as u128).min(u64::MAX as u128) as u64
    }

    pub fn nearest(self, seconds: f64) -> TimelineTime {
        // Pointer-driven seeks and edits select the closest timeline frame.
        self.quantize_seconds(seconds, f64::round)
    }

    pub fn ceil(self, seconds: f64) -> TimelineTime {
        // Imported media durations round outward so the last partial frame is retained.
        self.quantize_seconds(seconds, f64::ceil)
    }

    pub fn delta(self, seconds: f64) -> TimelineTime {
        if !seconds.is_finite() {
            return TimelineTime::ZERO;
        }
        let frames = (seconds * self.frames_per_second()).round();
        TimelineTime::from_frames(frames.clamp(i64::MIN as f64, i64::MAX as f64) as i64)
    }

    pub fn rescale_nearest(self, time: TimelineTime, target: Self) -> TimelineTime {
        self.rescale(time, target, divide_round)
    }

    pub fn rescale_floor(self, time: TimelineTime, target: Self) -> TimelineTime {
        self.rescale(time, target, |numerator, denominator| {
            numerator / denominator.max(1)
        })
    }

    fn rescale(
        self,
        time: TimelineTime,
        target: Self,
        round: impl FnOnce(u128, u128) -> u128,
    ) -> TimelineTime {
        if time <= TimelineTime::ZERO {
            return TimelineTime::ZERO;
        }
        let numerator = (time.frames() as u128)
            .saturating_mul(self.denominator.max(1) as u128)
            .saturating_mul(target.numerator.max(1) as u128);
        let denominator =
            (self.numerator.max(1) as u128).saturating_mul(target.denominator.max(1) as u128);
        TimelineTime::from_frames(round(numerator, denominator).min(i64::MAX as u128) as i64)
    }

    fn quantize_seconds(self, seconds: f64, round: impl FnOnce(f64) -> f64) -> TimelineTime {
        if !seconds.is_finite() || seconds <= 0.0 {
            return TimelineTime::ZERO;
        }
        let frames = round(seconds * self.frames_per_second());
        TimelineTime::from_frames(frames.clamp(0.0, i64::MAX as f64) as i64)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct TimelineSettings {
    pub frame_rate: FrameRate,
    pub width: u32,
    pub height: u32,
    pub audio_sample_rate: u32,
}

impl Default for TimelineSettings {
    fn default() -> Self {
        Self {
            frame_rate: FrameRate::default(),
            width: 1920,
            height: 1080,
            audio_sample_rate: 48_000,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TrackKind {
    #[default]
    Video,
    Audio,
}

/// Static visual adjustments for one timeline clip.
///
/// Position is an offset in timeline pixels from the clip's centered placement. Scale is a
/// normalized multiplier, so `1.0` means 100%.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub(super) struct VideoClipProperties {
    pub position_x: f64,
    pub position_y: f64,
    pub scale: f64,
}

impl Default for VideoClipProperties {
    fn default() -> Self {
        Self {
            position_x: 0.0,
            position_y: 0.0,
            scale: 1.0,
        }
    }
}

/// Static audio adjustments for one timeline clip.
///
/// `0 dB` is unity gain and pan ranges from `-1.0` (left) to `1.0` (right).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub(super) struct AudioClipProperties {
    pub gain_db: f64,
    pub muted: bool,
}

impl Default for AudioClipProperties {
    fn default() -> Self {
        Self {
            gain_db: 0.0,
            muted: false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct TimelineClip {
    #[serde(deserialize_with = "deserialize_ulid")]
    pub id: Ulid,
    #[serde(deserialize_with = "deserialize_ulid")]
    pub track_id: Ulid,
    #[serde(default = "Ulid::nil", deserialize_with = "deserialize_ulid")]
    pub asset_id: Ulid,
    pub timeline_start: TimelineTime,
    pub source_in: TimelineTime,
    pub source_out: TimelineTime,
    #[serde(default)]
    pub video_properties: VideoClipProperties,
    #[serde(default)]
    pub audio_properties: AudioClipProperties,
}

impl TimelineClip {
    pub fn duration(&self) -> TimelineTime {
        (self.source_out - self.source_in).max(TimelineTime::ZERO)
    }

    pub fn timeline_end(&self) -> TimelineTime {
        self.timeline_start + self.duration()
    }

    pub fn source_time_at(&self, timeline_position: TimelineTime) -> TimelineTime {
        let local =
            (timeline_position - self.timeline_start).clamp(TimelineTime::ZERO, self.duration());
        (self.source_in + local).min(self.source_out)
    }

    pub fn split_at(&self, timeline_position: TimelineTime) -> Option<(Self, Self)> {
        let local = timeline_position - self.timeline_start;
        if local < TimelineTime::ONE_FRAME || local > self.duration() - TimelineTime::ONE_FRAME {
            return None;
        }

        let source_split = self.source_time_at(timeline_position);
        let mut left = self.clone();
        left.source_out = source_split;
        let mut right = self.clone();
        right.id = Ulid::generate();
        right.timeline_start = timeline_position;
        right.source_in = source_split;
        Some((left, right))
    }
}

pub fn timeline_ranges_overlap(
    left_start: TimelineTime,
    left_end: TimelineTime,
    right_start: TimelineTime,
    right_end: TimelineTime,
) -> bool {
    left_start < right_end && right_start < left_end
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct TimelineTrack {
    #[serde(deserialize_with = "deserialize_ulid")]
    pub id: Ulid,
    pub name: String,
    pub kind: TrackKind,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub muted: bool,
    #[serde(default = "default_visible")]
    pub visible: bool,
}

fn default_visible() -> bool {
    true
}

fn divide_round(numerator: u128, denominator: u128) -> u128 {
    numerator.saturating_add(denominator / 2) / denominator.max(1)
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub(super) struct Timeline {
    pub settings: TimelineSettings,
    pub assets: Vec<MediaAsset>,
    pub tracks: Vec<TimelineTrack>,
    pub clips: Vec<TimelineClip>,
    pub view: TimelineViewState,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub(super) struct TimelineViewState {
    pub(super) saved_playhead_frame: TimelineTime,
    pub(super) horizontal_scroll: f32,
    pub(super) vertical_scroll: f32,
    pub(super) pixels_per_second: f32,
    pub(super) snapping_enabled: bool,
    pub(super) track_magnet_enabled: bool,
}

pub(super) struct TimelineState {
    pub(super) path: PathBuf,
    pub(super) data: Timeline,
    pub(super) playhead: TimelineTime,
    pub(super) scroll: ScrollHandle,
    pub(super) vertical_scroll: ScrollHandle,
    pub(super) interaction: TimelineInteractionState,
    pub(super) undo_stack: Vec<Timeline>,
    pub(super) redo_stack: Vec<Timeline>,
}

impl Default for TimelineViewState {
    fn default() -> Self {
        Self {
            saved_playhead_frame: TimelineTime::ZERO,
            horizontal_scroll: 0.0,
            vertical_scroll: 0.0,
            pixels_per_second: default_pixels_per_second(),
            snapping_enabled: true,
            track_magnet_enabled: true,
        }
    }
}

impl TimelineViewState {
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
            default_pixels_per_second()
        };
    }
}

impl Timeline {
    pub fn load(path: &Path) -> Result<Self, String> {
        let contents = fs::read_to_string(path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        let mut timeline = serde_json::from_str::<Self>(&contents)
            .map_err(|error| format!("could not parse {}: {error}", path.display()))?;
        timeline.normalize();
        Ok(timeline)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let directory = path
            .parent()
            .ok_or_else(|| "timeline path has no parent directory".to_string())?;
        fs::create_dir_all(directory)
            .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
        let json = serde_json::to_string_pretty(self)
            .map_err(|error| format!("could not serialize timeline: {error}"))?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, format!("{json}\n"))
            .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
        fs::rename(&temporary, path)
            .map_err(|error| format!("could not replace {}: {error}", path.display()))
    }

    pub fn asset(&self, id: Ulid) -> Option<&MediaAsset> {
        self.assets.iter().find(|asset| asset.id == id)
    }

    pub fn asset_for_path(&self, path: &Path) -> Option<&MediaAsset> {
        self.assets
            .iter()
            .find(|asset| asset.path.as_path() == path)
    }

    pub fn clip(&self, id: Ulid) -> Option<&TimelineClip> {
        self.clips.iter().find(|clip| clip.id == id)
    }

    pub fn clip_mut(&mut self, id: Ulid) -> Option<&mut TimelineClip> {
        self.clips.iter_mut().find(|clip| clip.id == id)
    }

    pub fn clip_index(&self, id: Ulid) -> Option<usize> {
        self.clips.iter().position(|clip| clip.id == id)
    }

    pub fn clip_locked(&self, clip_id: Ulid) -> bool {
        self.clip(clip_id)
            .and_then(|clip| self.track(clip.track_id))
            .is_some_and(|track| track.locked)
    }

    pub fn validate_clip_move_placements(
        &self,
        placements: &[(Ulid, Ulid, TimelineTime)],
        ignored_clip_ids: &HashSet<Ulid>,
    ) -> Result<(), ClipPlacementRejection> {
        if placements.is_empty() {
            return Err(ClipPlacementRejection::NoPlacements);
        }
        for (clip_id, track_id, start) in placements {
            let Some(clip) = self.clip(*clip_id) else {
                return Err(ClipPlacementRejection::MissingClip);
            };
            let Some(asset) = self.asset(clip.asset_id) else {
                return Err(ClipPlacementRejection::MissingAsset);
            };
            validate_clip_placement(
                self,
                *track_id,
                asset.kind,
                clip.duration(),
                *start,
                ignored_clip_ids,
            )?;
        }
        for (index, (clip_id, track_id, start)) in placements.iter().enumerate() {
            let duration = self
                .clip(*clip_id)
                .map(TimelineClip::duration)
                .ok_or(ClipPlacementRejection::MissingClip)?;
            if placements[index + 1..]
                .iter()
                .any(|(other_id, other_track_id, other_start)| {
                    let other_duration = self
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

    pub fn track(&self, id: Ulid) -> Option<&TimelineTrack> {
        self.tracks.iter().find(|track| track.id == id)
    }

    pub fn track_mut(&mut self, id: Ulid) -> Option<&mut TimelineTrack> {
        self.tracks.iter_mut().find(|track| track.id == id)
    }

    pub fn clips_on_track(&self, track_id: Ulid) -> impl Iterator<Item = &TimelineClip> {
        self.clips
            .iter()
            .filter(move |clip| clip.track_id == track_id)
    }

    pub fn content_duration(&self) -> TimelineTime {
        self.clips
            .iter()
            .map(TimelineClip::timeline_end)
            .max()
            .unwrap_or(TimelineTime::ZERO)
    }

    pub fn seconds(&self, time: TimelineTime) -> f64 {
        self.settings.frame_rate.seconds(time)
    }

    pub fn duration(&self, time: TimelineTime) -> Duration {
        self.settings.frame_rate.duration(time)
    }

    pub fn nearest_time(&self, seconds: f64) -> TimelineTime {
        self.settings.frame_rate.nearest(seconds)
    }

    pub fn audio_duration(&self, time: TimelineTime) -> Duration {
        let samples = self
            .settings
            .frame_rate
            .audio_samples(time, self.settings.audio_sample_rate);
        Duration::from_secs_f64(samples as f64 / self.settings.audio_sample_rate as f64)
    }

    /// Maps a timeline position within a clip to the source frame covering that instant.
    pub fn source_frame_at(
        &self,
        clip: &TimelineClip,
        timeline_position: TimelineTime,
    ) -> Option<i64> {
        let asset = self.asset(clip.asset_id)?;
        let source_rate = asset.frame_rate()?;
        let source_time = clip.source_time_at(timeline_position);
        Some(
            self.settings
                .frame_rate
                .rescale_floor(source_time, source_rate)
                .frames(),
        )
    }

    /// Returns an exact source-frame timestamp for video and a timeline-clock timestamp otherwise.
    pub fn source_position_at(
        &self,
        clip: &TimelineClip,
        timeline_position: TimelineTime,
    ) -> Duration {
        let Some(asset) = self.asset(clip.asset_id) else {
            return Duration::ZERO;
        };
        if let (Some(source_rate), Some(source_frame)) = (
            asset.frame_rate(),
            self.source_frame_at(clip, timeline_position),
        ) {
            return source_rate.duration(TimelineTime::from_frames(source_frame));
        }
        self.audio_duration(clip.source_time_at(timeline_position))
    }

    pub fn source_start_seconds(&self, clip: &TimelineClip) -> f64 {
        self.source_position_at(clip, clip.timeline_start)
            .as_secs_f64()
    }

    pub fn set_frame_rate(&mut self, frame_rate: FrameRate) {
        let frame_rate = FrameRate::new(frame_rate.numerator.max(1), frame_rate.denominator.max(1));
        let previous = self.settings.frame_rate;
        if previous == frame_rate {
            return;
        }

        for clip in &mut self.clips {
            let old_start = clip.timeline_start;
            let old_end = clip.timeline_end();
            let old_source_in = clip.source_in;
            clip.timeline_start = previous.rescale_nearest(old_start, frame_rate);
            let new_end = previous.rescale_nearest(old_end, frame_rate);
            let new_duration = (new_end - clip.timeline_start).max(TimelineTime::ONE_FRAME);
            clip.source_in = previous.rescale_nearest(old_source_in, frame_rate);
            clip.source_out = clip.source_in + new_duration;
        }
        self.settings.frame_rate = frame_rate;
        self.normalize();
    }

    pub fn ceil_time(&self, seconds: f64) -> TimelineTime {
        self.settings.frame_rate.ceil(seconds)
    }

    fn normalize(&mut self) {
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
        self.clips.retain(|clip| {
            self.tracks.iter().any(|track| track.id == clip.track_id)
                && self.assets.iter().any(|asset| asset.id == clip.asset_id)
                && clip.timeline_start >= TimelineTime::ZERO
                && clip.source_in >= TimelineTime::ZERO
                && clip.source_out - clip.source_in >= TimelineTime::ONE_FRAME
        });
        let frame_rate = self.settings.frame_rate;
        for clip in &mut self.clips {
            if let Some(asset) = self.assets.iter().find(|asset| asset.id == clip.asset_id) {
                if asset.kind == MediaKind::Image {
                    // An image has no time-based source to exhaust. Its five-second
                    // asset duration is only the initial clip length, not a maximum.
                    clip.source_in = clip.source_in.max(TimelineTime::ZERO);
                    clip.source_out = clip
                        .source_out
                        .max(clip.source_in + TimelineTime::ONE_FRAME);
                } else {
                    let asset_duration = frame_rate.ceil(asset.duration);
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
                .filter(|(_, clip)| clip.track_id == track.id)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            indices.sort_by(|left, right| {
                self.clips[*left]
                    .timeline_start
                    .cmp(&self.clips[*right].timeline_start)
                    .then_with(|| self.clips[*left].id.cmp(&self.clips[*right].id))
            });
            let mut next_available = TimelineTime::ZERO;
            for index in indices {
                self.clips[index].timeline_start =
                    self.clips[index].timeline_start.max(next_available);
                next_available = self.clips[index].timeline_end();
            }
        }
    }
}

impl TimelineState {
    pub(super) fn new(path: PathBuf, data: Timeline) -> Self {
        let playhead = data
            .view
            .saved_playhead_frame
            .clamp(TimelineTime::ZERO, data.content_duration());
        let scroll = ScrollHandle::new();
        scroll.set_offset(point(px(-data.view.horizontal_scroll), px(0.0)));
        let vertical_scroll = ScrollHandle::new();
        vertical_scroll.set_offset(point(px(0.0), px(-data.view.vertical_scroll)));
        let snapping_enabled = data.view.snapping_enabled;
        let magnet_enabled = data.view.track_magnet_enabled;
        let selected_clip_id = data.clips.first().map(|clip| clip.id);
        let selected_clip_ids = selected_clip_id.into_iter().collect();
        Self {
            path,
            data,
            playhead,
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
                context_menu: None,
            },
            scroll,
            vertical_scroll,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub(super) fn capture_playhead(&mut self) {
        self.data.view.saved_playhead_frame = self.playhead.max(TimelineTime::ZERO);
    }

    pub(super) fn save(&self, project_root: &Path) {
        if let Err(error) = self.data.save(&project_root.join(&self.path)) {
            eprintln!("Could not autosave timeline: {error}");
        }
    }

    pub(super) fn capture_scroll(&mut self) {
        self.data.view.horizontal_scroll = finite_nonnegative(-f32::from(self.scroll.offset().x));
        self.data.view.vertical_scroll =
            finite_nonnegative(-f32::from(self.vertical_scroll.offset().y));
    }

    pub(super) fn record_editing_history(&mut self) {
        self.undo_stack.push(self.data.clone());
        if self.undo_stack.len() > 100 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }
}

fn default_pixels_per_second() -> f32 {
    DEFAULT_TIMELINE_PIXELS_PER_SECOND
}

fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

#[cfg(test)]
#[path = "timeline.test.rs"]
mod tests;
