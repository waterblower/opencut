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

pub(super) struct TimelineInteractionState {
    pub(super) active_tool: TimelineTool,
    pub(super) snapping_enabled: bool,
    pub(super) magnet_enabled: bool,
    pub(super) selected_clip_id: Option<Ulid>,
    pub(super) selected_clip_ids: HashSet<Ulid>,
    pub(super) blade_guide: Option<TimelineTime>,
    pub(super) snap_guide: Option<TimelineTime>,
    pub(super) clip_move_drag: Option<ClipMoveDrag>,
    pub(super) marquee_selection: Option<MarqueeSelection>,
    pub(super) scrubbing_playhead: bool,
    pub(super) last_scrub_seek: Option<Instant>,
    pub(super) context_menu: Option<TimelineClipContextMenu>,
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

    pub(super) fn selected_clips_editable(&self) -> bool {
        !self.interaction.selected_clip_ids.is_empty()
            && self.interaction.selected_clip_ids.iter().all(|clip_id| {
                self.data.clip(*clip_id).is_some() && !self.data.clip_locked(*clip_id)
            })
    }

    pub(super) fn selected_clip_ids_in_timeline_order(&self) -> Vec<Ulid> {
        self.data
            .clips
            .iter()
            .filter(|clip| self.interaction.selected_clip_ids.contains(&clip.id))
            .map(|clip| clip.id)
            .collect()
    }

    pub(super) fn activate_timeline_tool(&mut self, tool: TimelineTool) {
        self.interaction.active_tool = tool;
        self.interaction.blade_guide = None;
        self.interaction.clip_move_drag = None;
        self.interaction.marquee_selection = None;
        self.interaction.snap_guide = None;
    }

    pub(super) fn select_clips_in_marquee(&mut self) {
        let Some(selection) = self.interaction.marquee_selection.as_ref() else {
            return;
        };
        let left = selection.start_x.min(selection.current_x);
        let right = selection.start_x.max(selection.current_x);
        let top = selection.start_y.min(selection.current_y);
        let bottom = selection.start_y.max(selection.current_y);
        let scroll_x = f32::from(self.scroll.offset().x);
        let scroll_y = f32::from(self.vertical_scroll.offset().y);

        let mut selected = selection.initial_selection.clone();
        for (track_index, track) in self.data.tracks.iter().enumerate() {
            let clip_top = TIMELINE_HEADER_HEIGHT
                + RULER_HEIGHT
                + track_index as f32 * TRACK_HEIGHT
                + scroll_y
                + 5.0;
            let clip_bottom = clip_top + TRACK_HEIGHT - 10.0;
            for clip in self.data.clips_on_track(track.id) {
                let clip_left = TRACK_HEADER_WIDTH
                    + scroll_x
                    + TIMELINE_PADDING
                    + self.data.seconds(clip.timeline_start) as f32
                        * self.data.view.pixels_per_second;
                let clip_right = clip_left
                    + (self.data.seconds(clip.duration()) as f32
                        * self.data.view.pixels_per_second)
                        .max(4.0);
                if clip_left <= right
                    && clip_right >= left
                    && clip_top <= bottom
                    && clip_bottom >= top
                {
                    selected.insert(clip.id);
                }
            }
        }
        self.interaction.selected_clip_id = self
            .data
            .clips
            .iter()
            .find(|clip| selected.contains(&clip.id))
            .map(|clip| clip.id);
        self.interaction.selected_clip_ids = selected;
    }

    pub(super) fn snap_time_ignoring(
        &self,
        time: TimelineTime,
        ignored_clip_ids: &HashSet<Ulid>,
    ) -> (TimelineTime, Option<TimelineTime>) {
        if !self.interaction.snapping_enabled {
            return (time.max(TimelineTime::ZERO), None);
        }
        let threshold = self
            .data
            .settings
            .frame_rate
            .ceil(SNAP_DISTANCE_PX as f64 / self.data.view.pixels_per_second as f64)
            .frames()
            .max(1) as u64;
        let mut candidates = vec![TimelineTime::ZERO, self.playhead];
        for clip in &self.data.clips {
            if !ignored_clip_ids.contains(&clip.id) {
                candidates.push(clip.timeline_start);
                candidates.push(clip.timeline_end());
            }
        }
        let snapped = candidates
            .into_iter()
            .filter(|candidate| candidate.abs_diff(time) <= threshold)
            .min_by_key(|candidate| candidate.abs_diff(time))
            .map(|candidate| (candidate.max(TimelineTime::ZERO), Some(candidate)));
        snapped.unwrap_or((time.max(TimelineTime::ZERO), None))
    }

    pub(super) fn snap_clip_start_ignoring(
        &self,
        start: TimelineTime,
        duration: TimelineTime,
        ignored_clip_ids: &HashSet<Ulid>,
    ) -> (TimelineTime, Option<TimelineTime>) {
        let (start_candidate, start_guide) = self.snap_time_ignoring(start, ignored_clip_ids);
        let (snapped_end, end_guide) = self.snap_time_ignoring(start + duration, ignored_clip_ids);
        let end_candidate = snapped_end - duration;
        choose_clip_snap(
            start,
            start_candidate,
            start_guide,
            end_candidate,
            end_guide,
        )
    }

    pub(super) fn zoom(&mut self, factor: f32) {
        let previous_pixels_per_second = self.data.view.pixels_per_second;
        let pixels_per_second = (self.data.view.pixels_per_second * factor).clamp(
            MIN_TIMELINE_PIXELS_PER_SECOND,
            MAX_TIMELINE_PIXELS_PER_SECOND,
        );
        if pixels_per_second != previous_pixels_per_second {
            let mut scroll_offset = self.scroll.offset();
            let playhead_seconds = self.data.seconds(self.playhead);
            scroll_offset.x = px(zoom_scroll_offset(
                f32::from(scroll_offset.x),
                playhead_seconds,
                previous_pixels_per_second,
                pixels_per_second,
            ));
            self.scroll.set_offset(scroll_offset);
            self.data.view.pixels_per_second = pixels_per_second;
        }
    }

    pub(super) fn timeline_position_from_x(&self, x: f32) -> TimelineTime {
        let scroll_x: f32 = self.scroll.offset().x.into();
        let content_x = x - TRACK_HEADER_WIDTH - scroll_x - TIMELINE_PADDING;
        self.data
            .nearest_time(content_x as f64 / self.data.view.pixels_per_second as f64)
            .clamp(TimelineTime::ZERO, self.data.content_duration())
    }
}

pub(super) fn choose_clip_snap(
    original_start: TimelineTime,
    start_candidate: TimelineTime,
    start_guide: Option<TimelineTime>,
    end_candidate: TimelineTime,
    end_guide: Option<TimelineTime>,
) -> (TimelineTime, Option<TimelineTime>) {
    match (start_guide, end_guide) {
        (None, None) => (original_start.max(TimelineTime::ZERO), None),
        (Some(guide), None) => (start_candidate.max(TimelineTime::ZERO), Some(guide)),
        (None, Some(guide)) => (end_candidate.max(TimelineTime::ZERO), Some(guide)),
        (Some(start_guide), Some(end_guide)) => {
            if end_candidate.abs_diff(original_start) < start_candidate.abs_diff(original_start) {
                (end_candidate.max(TimelineTime::ZERO), Some(end_guide))
            } else {
                (start_candidate.max(TimelineTime::ZERO), Some(start_guide))
            }
        }
    }
}

pub(super) fn zoom_scroll_offset(
    previous_offset: f32,
    anchor_seconds: f64,
    previous_pixels_per_second: f32,
    pixels_per_second: f32,
) -> f32 {
    let anchor_seconds = anchor_seconds as f32;
    (previous_offset + anchor_seconds * (previous_pixels_per_second - pixels_per_second)).min(0.0)
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

const MAX_RULER_TICKS: usize = 240;
const MIN_RULER_LABEL_SPACING: f32 = 72.0;
const MIN_FRAME_TICK_SPACING: f32 = 4.0;
const FRAME_TICK_OVERSCAN: f32 = 120.0;
const TICK_STEPS: [f64; 12] = [
    1.0, 2.0, 5.0, 10.0, 15.0, 20.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0,
];

fn timeline_marquee(selection: &MarqueeSelection) -> gpui::AnyElement {
    let left = selection.start_x.min(selection.current_x);
    let top = selection.start_y.min(selection.current_y);
    let width = (selection.start_x - selection.current_x).abs();
    let height = (selection.start_y - selection.current_y).abs();

    div()
        .absolute()
        .left(px(left))
        .top(px(top))
        .w(px(width))
        .h(px(height))
        .border_1()
        .border_color(rgb(ACCENT))
        .bg(gpui::rgba(0xf0b75e24))
        .into_any_element()
}

impl Editor {
    pub(super) fn timeline(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(timeline) = self.timeline.as_ref() else {
            return div()
                .id("editor-timeline-empty")
                .h(px(TIMELINE_HEIGHT))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .border_t_1()
                .border_color(rgb(BORDER))
                .bg(rgb(0x0a0a0c))
                .text_color(rgb(MUTED))
                .child("Create or select a timeline to begin editing")
                .into_any_element();
        };
        let frames_per_second = timeline.data.settings.frame_rate.frames_per_second();

        div()
            .id("editor-timeline")
            .relative()
            .h(px(TIMELINE_HEIGHT))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(rgb(BORDER))
            .bg(rgb(0x0a0a0c))
            .on_mouse_move(cx.listener(Self::update_clip_move))
            .on_mouse_move(cx.listener(Self::update_playhead_scrub))
            .on_mouse_move(cx.listener(Self::update_marquee_selection))
            .on_scroll_wheel(cx.listener(Self::finish_timeline_scroll))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_clip_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_playhead_scrub))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(Self::finish_marquee_selection),
            )
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_clip_move))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_playhead_scrub))
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(Self::finish_marquee_selection),
            )
            .child(self.timeline_toolbar(frames_per_second, cx))
            .child(self.timeline_tracks_container(cx))
            .when_some(
                timeline.interaction.marquee_selection.as_ref(),
                |this, selection| this.child(timeline_marquee(selection)),
            )
            .into_any_element()
    }

    fn timeline_tracks_container(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let timeline = self
            .timeline
            .as_ref()
            .expect("timeline view requires timeline state");
        let duration = timeline
            .data
            .seconds(timeline.data.content_duration())
            .max(12.0);
        let timeline_width = (duration as f32 * timeline.data.view.pixels_per_second
            + TIMELINE_PADDING * 2.0)
            .max(900.0);
        let track_headers = timeline
            .data
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| self.track_header(index, track, cx))
            .collect::<Vec<_>>();
        let track_rows = timeline
            .data
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| self.track_row(index, track, timeline_width, cx))
            .collect::<Vec<_>>();
        div()
            .id("timeline-tracks-vertical-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&timeline.vertical_scroll)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, event: &MouseDownEvent, window, cx| {
                    editor.begin_marquee_selection(event, window, cx);
                }),
            )
            .child(
                div()
                    .h(px(
                        RULER_HEIGHT + timeline.data.tracks.len() as f32 * TRACK_HEIGHT
                    ))
                    .min_h_full()
                    .w_full()
                    .flex()
                    .child(
                        div()
                            .w(px(TRACK_HEADER_WIDTH))
                            .h_full()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .border_r_1()
                            .border_color(rgb(BORDER))
                            .child(
                                div()
                                    .h(px(RULER_HEIGHT))
                                    .flex_shrink_0()
                                    .border_b_1()
                                    .border_color(rgb(BORDER)),
                            )
                            .children(track_headers),
                    )
                    .child(
                        div()
                            .id("editor-timeline-scroll")
                            .min_w_0()
                            .flex_1()
                            .h_full()
                            .overflow_x_scroll()
                            .track_scroll(&timeline.scroll)
                            .cursor(match timeline.interaction.active_tool {
                                TimelineTool::Blade => CursorStyle::Crosshair,
                                TimelineTool::Selection => CursorStyle::Arrow,
                            })
                            .on_mouse_move(cx.listener(Self::update_blade_guide))
                            .on_hover(cx.listener(Self::update_blade_guide_hover))
                            .child(
                                div()
                                    .relative()
                                    .w(px(timeline_width))
                                    .min_h_full()
                                    .on_drag_move::<ExplorerMediaDrag>(
                                        cx.listener(Self::update_explorer_media_drag),
                                    )
                                    .on_drop(cx.listener(
                                        |editor, drag: &ExplorerMediaDrag, _, cx| {
                                            editor.drop_explorer_media(drag, cx);
                                        },
                                    ))
                                    .child(self.timeline_ruler(duration, cx))
                                    .children(track_rows)
                                    .child(self.timeline_playhead(cx))
                                    .when_some(timeline.interaction.snap_guide, |this, guide| {
                                        let guide_left = TIMELINE_PADDING
                                            + timeline.data.seconds(guide) as f32
                                                * timeline.data.view.pixels_per_second;
                                        this.child(
                                            div()
                                                .absolute()
                                                .top_0()
                                                .bottom_0()
                                                .left(px(guide_left))
                                                .w(px(2.0))
                                                .bg(rgb(0x63c8ff))
                                                .child(
                                                    div()
                                                        .absolute()
                                                        .top_0()
                                                        .left(px(-3.0))
                                                        .size_2()
                                                        .rounded_full()
                                                        .bg(rgb(0x63c8ff)),
                                                ),
                                        )
                                    })
                                    .when_some(
                                        timeline.interaction.blade_guide,
                                        |this, position| {
                                            let guide_left = TIMELINE_PADDING
                                                + timeline.data.seconds(position) as f32
                                                    * timeline.data.view.pixels_per_second;
                                            this.child(
                                                div()
                                                    .absolute()
                                                    .top_0()
                                                    .bottom_0()
                                                    .left(px(guide_left))
                                                    .w(px(2.0))
                                                    .bg(rgb(ERROR))
                                                    .cursor(CursorStyle::Crosshair)
                                                    .child(
                                                        div()
                                                            .absolute()
                                                            .top_0()
                                                            .left(px(-4.0))
                                                            .size_2()
                                                            .bg(rgb(ERROR)),
                                                    ),
                                            )
                                        },
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn timeline_playhead(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let timeline = self
            .timeline
            .as_ref()
            .expect("timeline view requires timeline state");
        let left = TIMELINE_PADDING
            + timeline.data.seconds(timeline.playhead) as f32
                * timeline.data.view.pixels_per_second;

        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(left))
            .w(px(1.0))
            .bg(rgb(ACCENT))
            .cursor(if timeline.interaction.active_tool == TimelineTool::Blade {
                CursorStyle::Crosshair
            } else {
                CursorStyle::ResizeLeftRight
            })
            .when(
                timeline.interaction.active_tool != TimelineTool::Blade,
                |this| {
                    this.on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|editor, event: &MouseDownEvent, _, cx| {
                            editor.begin_playhead_scrub(event);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                },
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left(px(-4.0))
                    .size_2()
                    .bg(rgb(ACCENT)),
            )
            .into_any_element()
    }

    fn timeline_ruler(&self, duration: f64, cx: &mut Context<Self>) -> gpui::AnyElement {
        let timeline = self
            .timeline
            .as_ref()
            .expect("timeline view requires timeline state");
        let frame_rate = timeline.data.settings.frame_rate;
        let frames_per_second = frame_rate.frames_per_second();
        let displayed_frames = frame_rate.ceil(duration).frames().max(1);
        let pixels_per_frame = timeline.data.view.pixels_per_second / frames_per_second as f32;
        let frame_step = frame_tick_step(pixels_per_frame);
        let scroll_left = (-f32::from(timeline.scroll.offset().x)).max(0.0);
        let viewport_width = {
            let width = f32::from(timeline.scroll.bounds().size.width);
            if width > 0.0 { width } else { 1_200.0 }
        };
        let visible_start = ((scroll_left - FRAME_TICK_OVERSCAN - TIMELINE_PADDING).max(0.0)
            / pixels_per_frame.max(f32::EPSILON))
        .floor() as i64;
        let visible_end = ((scroll_left + viewport_width + FRAME_TICK_OVERSCAN - TIMELINE_PADDING)
            .max(0.0)
            / pixels_per_frame.max(f32::EPSILON))
        .ceil() as i64;
        let first_frame = visible_start
            .div_euclid(frame_step)
            .saturating_mul(frame_step)
            .max(frame_step);
        let last_frame = visible_end.min(displayed_frames);
        let nominal_fps = frames_per_second.round().max(1.0) as i64;
        let frame_ticks = (first_frame..=last_frame)
            .step_by(frame_step as usize)
            .map(|frame| {
                let emphasized = frame % nominal_fps == 0;
                let medium = !emphasized && frame % 5 == 0;
                let height = if emphasized {
                    12.0
                } else if medium {
                    8.0
                } else {
                    5.0
                };
                div()
                    .absolute()
                    .left(px(TIMELINE_PADDING
                        + frame_rate.seconds(TimelineTime::from_frames(frame)) as f32
                            * timeline.data.view.pixels_per_second))
                    .bottom_0()
                    .h(px(height))
                    .border_l_1()
                    .border_color(rgb(if emphasized { 0x5a5a62 } else { 0x3a3a40 }))
            });
        let tick_step = ruler_tick_step(duration, timeline.data.view.pixels_per_second);
        let tick_count = (duration / tick_step).ceil() as usize + 1;
        let ruler_ticks = (0..tick_count).map(|index| {
            let time = index as f64 * tick_step;
            div()
                .absolute()
                .left(px(
                    TIMELINE_PADDING + time as f32 * timeline.data.view.pixels_per_second
                ))
                .top_0()
                .h_full()
                .border_l_1()
                .border_color(rgb(0x333338))
                .pl_1()
                .font_family("monospace")
                .text_xs()
                .text_color(rgb(MUTED))
                .child(format_time_precise(time))
        });
        div()
            .id("timeline-ruler")
            .relative()
            .w_full()
            .h(px(RULER_HEIGHT))
            .border_b_1()
            .border_color(rgb(BORDER))
            .cursor(CursorStyle::PointingHand)
            .children(frame_ticks)
            .children(ruler_ticks)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, event: &MouseDownEvent, _, cx| {
                    editor.begin_playhead_scrub(event);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .into_any_element()
    }

    fn timeline_toolbar(&self, frames_per_second: f64, cx: &mut Context<Self>) -> gpui::AnyElement {
        let timeline = self
            .timeline
            .as_ref()
            .expect("timeline view requires timeline state");
        div()
            .id("timeline-toolbar")
            .h(px(TIMELINE_HEADER_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        timeline_tool_button(
                            "timeline-selection-tool",
                            "V Select",
                            timeline.interaction.active_tool == TimelineTool::Selection,
                        )
                        .on_click(cx.listener(|editor, _, _, cx| {
                            let Some(timeline) = editor.timeline.as_mut() else {
                                return;
                            };
                            timeline.activate_timeline_tool(TimelineTool::Selection);
                            cx.notify();
                        })),
                    )
                    .child(
                        timeline_tool_button(
                            "timeline-blade-tool",
                            "B Blade",
                            timeline.interaction.active_tool == TimelineTool::Blade,
                        )
                        .on_click(cx.listener(|editor, _, _, cx| {
                            let Some(timeline) = editor.timeline.as_mut() else {
                                return;
                            };
                            timeline.activate_timeline_tool(TimelineTool::Blade);
                            cx.notify();
                        })),
                    )
                    .child(
                        timeline_icon_button(
                            "timeline-play",
                            if self.preview.target.video().map_or(false, |v| !v.paused()) {
                                "Ⅱ"
                            } else {
                                "▶"
                            },
                        )
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.toggle_playback();
                            cx.notify();
                        })),
                    )
                    .child(
                        div()
                            .w(px(108.0))
                            .font_family("monospace")
                            .text_sm()
                            .child(format!(
                                "{} / {}",
                                format_time(timeline.data.seconds(timeline.playhead), false),
                                format_time(
                                    timeline.data.seconds(timeline.data.content_duration()),
                                    false
                                )
                            )),
                    )
                    .child(
                        timeline_icon_button("add-video-track", "+V").on_click(cx.listener(
                            |editor, _, _, cx| {
                                editor.add_track(TrackKind::Video);
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        timeline_icon_button("add-audio-track", "+A").on_click(cx.listener(
                            |editor, _, _, cx| {
                                editor.add_track(TrackKind::Audio);
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        timeline_icon_button(
                            "toggle-timeline-snapping",
                            if timeline.interaction.snapping_enabled {
                                "Snap on"
                            } else {
                                "Snap off"
                            },
                        )
                        .border_1()
                        .border_color(rgb(if timeline.interaction.snapping_enabled {
                            ACCENT
                        } else {
                            BORDER
                        }))
                        .text_color(rgb(if timeline.interaction.snapping_enabled {
                            ACCENT
                        } else {
                            MUTED
                        }))
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.toggle_snapping();
                            cx.notify();
                        })),
                    )
                    .child(
                        timeline_icon_button(
                            "toggle-track-magnet",
                            if timeline.interaction.magnet_enabled {
                                "Magnet on"
                            } else {
                                "Magnet off"
                            },
                        )
                        .border_1()
                        .border_color(rgb(if timeline.interaction.magnet_enabled {
                            ACCENT
                        } else {
                            BORDER
                        }))
                        .text_color(rgb(if timeline.interaction.magnet_enabled {
                            ACCENT
                        } else {
                            MUTED
                        }))
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.toggle_track_magnet();
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(timeline_icon_button("zoom-out", "−").on_click(cx.listener(
                        |editor, _, _, cx| {
                            let Some(timeline) = editor.timeline.as_mut() else {
                                return;
                            };
                            timeline.zoom(0.8);
                            editor.save_timeline_scroll();
                            cx.notify();
                        },
                    )))
                    .child(
                        div()
                            .w(px(58.0))
                            .text_center()
                            .font_family("monospace")
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(format!("{:.0}px/s", timeline.data.view.pixels_per_second)),
                    )
                    .child(
                        div()
                            .w(px(66.0))
                            .text_center()
                            .font_family("monospace")
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(format!("{frames_per_second:.2} fps")),
                    )
                    .child(timeline_icon_button("zoom-in", "+").on_click(cx.listener(
                        |editor, _, _, cx| {
                            let Some(timeline) = editor.timeline.as_mut() else {
                                return;
                            };
                            timeline.zoom(1.25);
                            editor.save_timeline_scroll();
                            cx.notify();
                        },
                    ))),
            )
            .into_any_element()
    }
}

fn frame_tick_step(pixels_per_frame: f32) -> i64 {
    (MIN_FRAME_TICK_SPACING / pixels_per_frame.max(f32::EPSILON))
        .ceil()
        .max(1.0) as i64
}

fn ruler_tick_step(duration: f64, pixels_per_second: f32) -> f64 {
    let spacing_step = MIN_RULER_LABEL_SPACING as f64 / pixels_per_second.max(f32::EPSILON) as f64;
    let count_step = duration.max(0.0) / MAX_RULER_TICKS as f64;
    let minimum_step = spacing_step.max(count_step);

    TICK_STEPS
        .iter()
        .copied()
        .find(|step| *step >= minimum_step)
        .unwrap_or_else(|| {
            let largest_step = *TICK_STEPS
                .last()
                .expect("timeline tick steps are not empty");
            (minimum_step / largest_step).ceil() * largest_step
        })
}

fn timeline_icon_button(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h_7()
        .min_w(px(28.0))
        .px_2()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .bg(rgb(SURFACE))
        .cursor(CursorStyle::PointingHand)
        .text_xs()
        .hover(|style| style.bg(rgb(SURFACE_HOVER)))
        .child(label)
}

fn timeline_tool_button(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    active: bool,
) -> gpui::Stateful<gpui::Div> {
    timeline_icon_button(id, label)
        .border_1()
        .border_color(rgb(if active { ACCENT } else { BORDER }))
        .text_color(rgb(if active { ACCENT } else { MUTED }))
}

fn format_time_precise(seconds: f64) -> String {
    let minutes = (seconds / 60.0).floor() as u64;
    let seconds = seconds % 60.0;
    format!("{minutes:02}:{seconds:04.1}")
}

#[cfg(test)]
#[path = "timeline.test.rs"]
mod tests;
