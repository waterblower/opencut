use ffmpeg::{codec, format, media::Type};
use ffmpeg_next as ffmpeg;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    ops::{Add, AddAssign, Sub, SubAssign},
    path::{Path, PathBuf},
    time::Duration,
};

pub(super) const DEFAULT_IMAGE_CLIP_DURATION: f64 = 5.0;
pub(super) const PROJECT_VERSION: u32 = 6;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub(super) struct TimelineTime(i64);

impl TimelineTime {
    pub const ZERO: Self = Self(0);
    pub const ONE_FRAME: Self = Self(1);
    pub const MAX: Self = Self(i64::MAX);

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
        // Pointer-driven seeks and edits select the closest project frame.
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
pub(super) struct ProjectSettings {
    pub frame_rate: FrameRate,
    pub width: u32,
    pub height: u32,
    pub audio_sample_rate: u32,
}

impl Default for ProjectSettings {
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
pub(super) enum MediaKind {
    #[default]
    Video,
    Image,
    Audio,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum TrackKind {
    #[default]
    Video,
    Audio,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct MediaAsset {
    pub id: u64,
    #[serde(default)]
    pub kind: MediaKind,
    pub path: PathBuf,
    pub name: String,
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub framerate: f64,
    #[serde(default)]
    pub frame_rate_numerator: u32,
    #[serde(default)]
    pub frame_rate_denominator: u32,
    pub codec: String,
    pub has_audio: bool,
}

impl MediaAsset {
    pub fn frame_rate(&self) -> Option<FrameRate> {
        if self.kind != MediaKind::Video {
            return None;
        }
        if self.frame_rate_numerator > 0 && self.frame_rate_denominator > 0 {
            return Some(FrameRate::new(
                self.frame_rate_numerator,
                self.frame_rate_denominator,
            ));
        }
        approximate_frame_rate(self.framerate)
    }
}

/// Static visual adjustments for one timeline clip.
///
/// Position is an offset in project pixels from the clip's centered placement. Scale and
/// opacity are normalized multipliers, so `1.0` means 100%.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub(super) struct VideoClipProperties {
    pub position_x: f64,
    pub position_y: f64,
    pub scale: f64,
    pub opacity: f64,
    pub crop_left: f64,
    pub crop_right: f64,
    pub crop_top: f64,
    pub crop_bottom: f64,
}

impl Default for VideoClipProperties {
    fn default() -> Self {
        Self {
            position_x: 0.0,
            position_y: 0.0,
            scale: 1.0,
            opacity: 1.0,
            crop_left: 0.0,
            crop_right: 0.0,
            crop_top: 0.0,
            crop_bottom: 0.0,
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
    pub id: u64,
    pub track_id: u64,
    #[serde(default)]
    pub asset_id: u64,
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

    pub fn contains(&self, time: TimelineTime) -> bool {
        time >= self.timeline_start && time < self.timeline_end()
    }

    pub fn is_continuous_with(&self, next: &Self) -> bool {
        self.track_id == next.track_id
            && self.asset_id == next.asset_id
            && self.timeline_end() == next.timeline_start
            && self.source_out == next.source_in
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
    pub id: u64,
    pub name: String,
    pub kind: TrackKind,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub muted: bool,
    #[serde(default = "default_visible")]
    pub visible: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Project {
    pub version: u32,
    pub settings: ProjectSettings,
    pub assets: Vec<MediaAsset>,
    pub tracks: Vec<TimelineTrack>,
    pub clips: Vec<TimelineClip>,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            version: PROJECT_VERSION,
            settings: ProjectSettings::default(),
            assets: Vec::new(),
            tracks: default_tracks(),
            clips: Vec::new(),
        }
    }
}

impl Project {
    pub fn load(path: &Path) -> Result<Self, String> {
        let contents = fs::read_to_string(path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        let mut project = serde_json::from_str::<Self>(&contents)
            .map_err(|error| format!("could not parse {}: {error}", path.display()))?;
        project.normalize();
        Ok(project)
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

    pub fn asset(&self, id: u64) -> Option<&MediaAsset> {
        self.assets.iter().find(|asset| asset.id == id)
    }

    pub fn clip(&self, id: u64) -> Option<&TimelineClip> {
        self.clips.iter().find(|clip| clip.id == id)
    }

    pub fn clip_mut(&mut self, id: u64) -> Option<&mut TimelineClip> {
        self.clips.iter_mut().find(|clip| clip.id == id)
    }

    pub fn clip_index(&self, id: u64) -> Option<usize> {
        self.clips.iter().position(|clip| clip.id == id)
    }

    pub fn track(&self, id: u64) -> Option<&TimelineTrack> {
        self.tracks.iter().find(|track| track.id == id)
    }

    pub fn track_mut(&mut self, id: u64) -> Option<&mut TimelineTrack> {
        self.tracks.iter_mut().find(|track| track.id == id)
    }

    pub fn clips_on_track(&self, track_id: u64) -> impl Iterator<Item = &TimelineClip> {
        self.clips
            .iter()
            .filter(move |clip| clip.track_id == track_id)
    }

    pub fn trim_limits(&self, clip_id: u64) -> Option<(TimelineTime, TimelineTime)> {
        let clip = self.clip(clip_id)?;
        let previous_end = self
            .clips_on_track(clip.track_id)
            .filter(|other| other.id != clip_id && other.timeline_start < clip.timeline_start)
            .map(TimelineClip::timeline_end)
            .max()
            .unwrap_or(TimelineTime::ZERO);
        let next_start = self
            .clips_on_track(clip.track_id)
            .filter(|other| other.id != clip_id && other.timeline_start >= clip.timeline_end())
            .map(|other| other.timeline_start)
            .min()
            .unwrap_or(TimelineTime::MAX);
        Some((previous_end, next_start))
    }

    pub fn content_duration(&self) -> TimelineTime {
        self.clips
            .iter()
            .map(TimelineClip::timeline_end)
            .max()
            .unwrap_or(TimelineTime::ZERO)
    }

    pub fn timeline_duration(&self) -> TimelineTime {
        self.content_duration()
    }

    pub fn visual_clip_at_time(&self, time: TimelineTime) -> Option<&TimelineClip> {
        self.tracks
            .iter()
            .filter(|track| track.visible && track.kind == TrackKind::Video)
            .find_map(|track| {
                self.clips_on_track(track.id)
                    .filter(|clip| {
                        clip.contains(time)
                            && self
                                .asset(clip.asset_id)
                                .is_some_and(|asset| asset.kind == MediaKind::Video)
                    })
                    .max_by_key(|clip| clip.timeline_start)
            })
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

    pub fn floor_duration(&self, duration: Duration) -> TimelineTime {
        self.settings.frame_rate.floor_duration(duration)
    }

    pub fn audio_duration(&self, time: TimelineTime) -> Duration {
        let samples = self
            .settings
            .frame_rate
            .audio_samples(time, self.settings.audio_sample_rate);
        Duration::from_secs_f64(samples as f64 / self.settings.audio_sample_rate as f64)
    }

    pub fn audio_seconds(&self, time: TimelineTime) -> f64 {
        self.audio_duration(time).as_secs_f64()
    }

    /// Maps a timeline position within a clip to the source frame covering that instant.
    pub fn source_frame_at(
        &self,
        clip: &TimelineClip,
        timeline_position: TimelineTime,
    ) -> Option<i64> {
        let asset = self.asset(clip.asset_id)?;
        let source_rate = asset.frame_rate()?;
        let local =
            (timeline_position - clip.timeline_start).clamp(TimelineTime::ZERO, clip.duration());
        let source_time = (clip.source_in + local).min(clip.source_out);
        Some(
            self.settings
                .frame_rate
                .rescale_floor(source_time, source_rate)
                .frames(),
        )
    }

    /// Returns an exact source-frame timestamp for video and a project-clock timestamp otherwise.
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
        let local =
            (timeline_position - clip.timeline_start).clamp(TimelineTime::ZERO, clip.duration());
        self.audio_duration((clip.source_in + local).min(clip.source_out))
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

    pub fn next_id(&self) -> u64 {
        self.assets
            .iter()
            .map(|asset| asset.id)
            .chain(self.tracks.iter().map(|track| track.id))
            .chain(self.clips.iter().map(|clip| clip.id))
            .max()
            .unwrap_or(0)
            + 1
    }

    fn normalize(&mut self) {
        self.version = PROJECT_VERSION;
        if self.settings.frame_rate.numerator == 0 {
            self.settings.frame_rate.numerator = 30;
        }
        if self.settings.frame_rate.denominator == 0 {
            self.settings.frame_rate.denominator = 1;
        }
        self.settings.width = self.settings.width.max(2);
        self.settings.height = self.settings.height.max(2);
        self.settings.audio_sample_rate = self.settings.audio_sample_rate.max(8_000);
        if self.tracks.is_empty() {
            self.tracks = default_tracks();
        }
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

fn default_visible() -> bool {
    true
}

fn divide_round(numerator: u128, denominator: u128) -> u128 {
    numerator.saturating_add(denominator / 2) / denominator.max(1)
}

fn default_tracks() -> Vec<TimelineTrack> {
    vec![
        TimelineTrack {
            id: 1,
            name: "Video 1".into(),
            kind: TrackKind::Video,
            locked: false,
            muted: false,
            visible: true,
        },
        TimelineTrack {
            id: 2,
            name: "Audio 1".into(),
            kind: TrackKind::Audio,
            locked: false,
            muted: false,
            visible: true,
        },
    ]
}

pub(super) fn probe_media(path: &Path, id: u64) -> Result<MediaAsset, String> {
    ffmpeg::init().map_err(|error| format!("could not initialize FFmpeg: {error}"))?;
    let input = format::input(path)
        .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
    let stream = input
        .streams()
        .best(Type::Video)
        .ok_or_else(|| format!("{} has no video stream", path.display()))?;
    let context = codec::context::Context::from_parameters(stream.parameters())
        .map_err(|error| format!("could not inspect video parameters: {error}"))?;
    let decoder = context
        .decoder()
        .video()
        .map_err(|error| format!("could not inspect video decoder: {error}"))?;
    let source_frame_rate = frame_rate_from_ffmpeg(stream.avg_frame_rate()).unwrap_or_default();
    let framerate = source_frame_rate.frames_per_second();
    let duration = if input.duration() > 0 {
        input.duration() as f64 / ffmpeg::ffi::AV_TIME_BASE as f64
    } else if stream.duration() > 0 {
        stream.duration() as f64 * rational_to_f64(stream.time_base()).unwrap_or(0.0)
    } else {
        0.0
    };
    if !duration.is_finite() || duration <= 0.0 {
        return Err(format!(
            "could not determine duration of {}",
            path.display()
        ));
    }
    if duration < 1.0 / 30.0 {
        return Err(format!("{} is shorter than one frame", path.display()));
    }

    Ok(MediaAsset {
        id,
        kind: MediaKind::Video,
        path: fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()),
        name: path
            .file_stem()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string()),
        duration,
        width: decoder.width(),
        height: decoder.height(),
        framerate,
        frame_rate_numerator: source_frame_rate.numerator,
        frame_rate_denominator: source_frame_rate.denominator,
        codec: stream.parameters().id().name().to_string(),
        has_audio: input.streams().best(Type::Audio).is_some(),
    })
}

pub(super) fn probe_image(path: &Path, id: u64) -> Result<MediaAsset, String> {
    let (width, height) = image::image_dimensions(path)
        .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
    let codec = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_uppercase())
        .unwrap_or_else(|| "IMAGE".to_string());

    Ok(MediaAsset {
        id,
        kind: MediaKind::Image,
        path: fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()),
        name: path
            .file_stem()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string()),
        duration: DEFAULT_IMAGE_CLIP_DURATION,
        width,
        height,
        framerate: 0.0,
        frame_rate_numerator: 0,
        frame_rate_denominator: 0,
        codec,
        has_audio: false,
    })
}

pub(super) fn probe_audio(path: &Path, id: u64) -> Result<MediaAsset, String> {
    ffmpeg::init().map_err(|error| format!("could not initialize FFmpeg: {error}"))?;
    let input = format::input(path)
        .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
    let stream = input
        .streams()
        .best(Type::Audio)
        .ok_or_else(|| format!("{} has no audio stream", path.display()))?;
    let duration = if input.duration() > 0 {
        input.duration() as f64 / ffmpeg::ffi::AV_TIME_BASE as f64
    } else if stream.duration() > 0 {
        stream.duration() as f64 * rational_to_f64(stream.time_base()).unwrap_or(0.0)
    } else {
        0.0
    };
    if !duration.is_finite() || duration <= 0.0 {
        return Err(format!(
            "could not determine duration of {}",
            path.display()
        ));
    }
    Ok(MediaAsset {
        id,
        kind: MediaKind::Audio,
        path: fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf()),
        name: path
            .file_stem()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string()),
        duration,
        width: 0,
        height: 0,
        framerate: 0.0,
        frame_rate_numerator: 0,
        frame_rate_denominator: 0,
        codec: stream.parameters().id().name().to_string(),
        has_audio: true,
    })
}

fn rational_to_f64(value: ffmpeg::Rational) -> Option<f64> {
    let denominator = value.denominator();
    (denominator != 0).then(|| value.numerator() as f64 / denominator as f64)
}

fn frame_rate_from_ffmpeg(value: ffmpeg::Rational) -> Option<FrameRate> {
    let numerator = u32::try_from(value.numerator()).ok()?;
    let denominator = u32::try_from(value.denominator()).ok()?;
    (numerator > 0 && denominator > 0).then(|| FrameRate::new(numerator, denominator))
}

fn approximate_frame_rate(fps: f64) -> Option<FrameRate> {
    if !fps.is_finite() || fps <= 0.0 {
        return None;
    }
    for rate in [
        FrameRate::new(24_000, 1_001),
        FrameRate::new(30_000, 1_001),
        FrameRate::new(60_000, 1_001),
    ] {
        if (rate.frames_per_second() - fps).abs() < 0.01 {
            return Some(rate);
        }
    }
    Some(FrameRate::new(
        fps.round().clamp(1.0, u32::MAX as f64) as u32,
        1,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(value: i64) -> TimelineTime {
        TimelineTime::from_frames(value)
    }

    fn video_clip(id: u64, start: i64, duration: i64) -> TimelineClip {
        TimelineClip {
            id,
            track_id: 1,
            asset_id: 100,
            timeline_start: frames(start),
            source_in: TimelineTime::ZERO,
            source_out: frames(duration),
            video_properties: VideoClipProperties::default(),
            audio_properties: AudioClipProperties::default(),
        }
    }

    fn video_asset() -> MediaAsset {
        MediaAsset {
            id: 100,
            kind: MediaKind::Video,
            path: "clip.mp4".into(),
            name: "clip".into(),
            duration: 30.0,
            width: 1920,
            height: 1080,
            framerate: 30.0,
            frame_rate_numerator: 30,
            frame_rate_denominator: 1,
            codec: "h264".into(),
            has_audio: true,
        }
    }

    fn image_asset() -> MediaAsset {
        MediaAsset {
            id: 100,
            kind: MediaKind::Image,
            path: "still.png".into(),
            name: "still".into(),
            duration: DEFAULT_IMAGE_CLIP_DURATION,
            width: 1920,
            height: 1080,
            framerate: 0.0,
            frame_rate_numerator: 0,
            frame_rate_denominator: 0,
            codec: "PNG".into(),
            has_audio: false,
        }
    }

    #[test]
    fn repairs_overlapping_clips_when_loading_a_project() {
        let mut project = Project {
            assets: vec![video_asset()],
            clips: vec![video_clip(10, 0, 150), video_clip(11, 90, 120)],
            ..Project::default()
        };

        project.normalize();

        assert_eq!(project.clips[0].timeline_start, frames(0));
        assert_eq!(project.clips[1].timeline_start, frames(150));
    }

    #[test]
    fn still_image_clips_can_extend_beyond_their_default_duration() {
        let mut project = Project {
            assets: vec![image_asset()],
            clips: vec![video_clip(10, 0, 300)],
            ..Project::default()
        };

        project.normalize();

        assert_eq!(project.clips[0].duration(), frames(300));
        assert_eq!(project.seconds(project.clips[0].duration()), 10.0);
    }

    #[test]
    fn time_based_media_remains_bounded_by_its_source_duration() {
        let mut project = Project {
            assets: vec![video_asset()],
            clips: vec![video_clip(10, 0, 1_200)],
            ..Project::default()
        };

        project.normalize();

        assert_eq!(project.clips[0].duration(), frames(900));
    }

    #[test]
    fn fractional_frame_rates_round_trip_without_drift() {
        for frame_rate in [
            FrameRate {
                numerator: 24_000,
                denominator: 1_001,
            },
            FrameRate {
                numerator: 30_000,
                denominator: 1_001,
            },
            FrameRate {
                numerator: 60_000,
                denominator: 1_001,
            },
        ] {
            let original = frames(1_000_003);
            let seconds = frame_rate.seconds(original);
            assert_eq!(frame_rate.nearest(seconds), original);
        }
    }

    #[test]
    fn repeated_frame_splits_preserve_the_total_duration() {
        let original = frames(10_000);
        let mut remaining = original;
        let mut pieces = Vec::new();
        for split in [1, 17, 301, 999, 2_048] {
            let piece = frames(split);
            remaining -= piece;
            pieces.push(piece);
        }
        let reconstructed = pieces
            .into_iter()
            .fold(remaining, |duration, piece| duration + piece);
        assert_eq!(reconstructed, original);
    }

    #[test]
    fn long_timeline_duration_uses_exact_frame_counts() {
        let frame_rate = FrameRate {
            numerator: 30_000,
            denominator: 1_001,
        };
        let ten_hours = frame_rate.nearest(10.0 * 60.0 * 60.0);
        assert_eq!(frame_rate.nearest(frame_rate.seconds(ten_hours)), ten_hours);
        assert_eq!(
            frame_rate.floor_duration(frame_rate.duration(ten_hours)),
            ten_hours
        );
    }

    #[test]
    fn preview_and_export_boundaries_share_the_same_frame_time() {
        let project = Project {
            settings: ProjectSettings {
                frame_rate: FrameRate {
                    numerator: 24_000,
                    denominator: 1_001,
                },
                ..ProjectSettings::default()
            },
            ..Project::default()
        };
        let boundary = frames(98_765);
        let preview_duration = project.duration(boundary).as_secs_f64();
        let export_seconds = project.seconds(boundary);
        assert!((preview_duration - export_seconds).abs() <= 1.0e-9);
    }

    #[test]
    fn project_frames_map_to_exact_audio_samples() {
        let frame_rate = FrameRate {
            numerator: 30_000,
            denominator: 1_001,
        };
        assert_eq!(frame_rate.audio_samples(frames(30_000), 48_000), 48_048_000);
    }

    #[test]
    fn maps_30_fps_source_frames_onto_a_24_fps_timeline() {
        let project = Project {
            settings: ProjectSettings {
                frame_rate: FrameRate::new(24, 1),
                ..ProjectSettings::default()
            },
            assets: vec![video_asset()],
            clips: vec![video_clip(10, 0, 24)],
            ..Project::default()
        };
        let clip = &project.clips[0];
        let mapped = (0..=8)
            .map(|frame| project.source_frame_at(clip, frames(frame)).unwrap())
            .collect::<Vec<_>>();

        assert_eq!(mapped, vec![0, 1, 2, 3, 5, 6, 7, 8, 10]);
    }

    #[test]
    fn changing_timeline_rate_preserves_elapsed_edit_times() {
        let mut project = Project {
            assets: vec![video_asset()],
            clips: vec![video_clip(10, 30, 300)],
            ..Project::default()
        };

        project.set_frame_rate(FrameRate::new(24, 1));

        assert_eq!(project.clips[0].timeline_start, frames(24));
        assert_eq!(project.clips[0].duration(), frames(240));
    }

    #[test]
    fn recognizes_two_halves_of_a_split_as_continuous() {
        let first = video_clip(10, 0, 120);
        let mut second = video_clip(11, 120, 120);
        second.source_in = frames(120);
        second.source_out = frames(240);

        assert!(first.is_continuous_with(&second));
        second.source_in = frames(121);
        assert!(!first.is_continuous_with(&second));
    }

    #[test]
    fn project_serialization_stores_integer_frames_and_rational_rate() {
        let project = Project {
            assets: vec![video_asset()],
            clips: vec![video_clip(10, 17, 83)],
            ..Project::default()
        };
        let json = serde_json::to_value(project).unwrap();
        assert_eq!(json["settings"]["frame_rate"]["numerator"], 30);
        assert_eq!(json["settings"]["frame_rate"]["denominator"], 1);
        assert_eq!(json["clips"][0]["timeline_start"], 17);
        assert_eq!(json["clips"][0]["source_out"], 83);
    }

    #[test]
    fn clip_properties_have_neutral_defaults() {
        assert_eq!(
            VideoClipProperties::default(),
            VideoClipProperties {
                position_x: 0.0,
                position_y: 0.0,
                scale: 1.0,
                opacity: 1.0,
                crop_left: 0.0,
                crop_right: 0.0,
                crop_top: 0.0,
                crop_bottom: 0.0,
            }
        );
        assert_eq!(
            AudioClipProperties::default(),
            AudioClipProperties {
                gain_db: 0.0,
                muted: false,
            }
        );
    }

    #[test]
    fn clips_without_property_objects_deserialize_with_defaults() {
        let legacy = serde_json::json!({
            "id": 10,
            "track_id": 1,
            "asset_id": 100,
            "timeline_start": 0,
            "source_in": 0,
            "source_out": 30
        });
        let clip = serde_json::from_value::<TimelineClip>(legacy).unwrap();

        assert_eq!(clip.video_properties, VideoClipProperties::default());
        assert_eq!(clip.audio_properties, AudioClipProperties::default());
    }

    #[test]
    fn clip_properties_round_trip_through_project_json() {
        let mut clip = video_clip(10, 0, 30);
        clip.video_properties = VideoClipProperties {
            position_x: 120.0,
            position_y: -45.0,
            scale: 1.25,
            opacity: 0.8,
            crop_left: 0.1,
            crop_right: 0.2,
            crop_top: 0.05,
            crop_bottom: 0.15,
        };
        clip.audio_properties = AudioClipProperties {
            gain_db: -6.0,
            muted: true,
        };
        let value = serde_json::to_value(&clip).unwrap();
        let restored = serde_json::from_value::<TimelineClip>(value).unwrap();

        assert_eq!(restored.video_properties, clip.video_properties);
        assert_eq!(restored.audio_properties, clip.audio_properties);
    }
}
