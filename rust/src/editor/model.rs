use ffmpeg::{codec, format, media::Type};
use ffmpeg_next as ffmpeg;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

pub(super) const MIN_CLIP_DURATION: f64 = 1.0 / 30.0;
pub(super) const DEFAULT_IMAGE_DURATION: f64 = 5.0;
pub(super) const PROJECT_VERSION: u32 = 3;

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
    pub codec: String,
    pub has_audio: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct TimelineClip {
    pub id: u64,
    pub track_id: u64,
    #[serde(default)]
    pub asset_id: Option<u64>,
    pub timeline_start: f64,
    pub source_in: f64,
    pub source_out: f64,
}

impl TimelineClip {
    pub fn duration(&self) -> f64 {
        (self.source_out - self.source_in).max(0.0)
    }

    pub fn timeline_end(&self) -> f64 {
        self.timeline_start + self.duration()
    }

    pub fn contains(&self, time: f64) -> bool {
        time >= self.timeline_start && time < self.timeline_end()
    }
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
pub(super) struct TimelineMarker {
    pub id: u64,
    pub time: f64,
    pub label: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct Project {
    pub version: u32,
    pub assets: Vec<MediaAsset>,
    pub tracks: Vec<TimelineTrack>,
    pub clips: Vec<TimelineClip>,
    pub markers: Vec<TimelineMarker>,
}

impl Default for Project {
    fn default() -> Self {
        Self {
            version: PROJECT_VERSION,
            assets: Vec::new(),
            tracks: default_tracks(),
            clips: Vec::new(),
            markers: Vec::new(),
        }
    }
}

impl Project {
    pub fn load(project_root: &Path) -> Self {
        let path = project_path(project_root);
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) if error.kind() == ErrorKind::NotFound => return Self::default(),
            Err(error) => {
                eprintln!("Could not read {}: {error}", path.display());
                return Self::default();
            }
        };
        match serde_json::from_str::<Self>(&contents) {
            Ok(mut project) => {
                project.normalize();
                project
            }
            Err(error) => {
                eprintln!("Could not parse {}: {error}", path.display());
                Self::default()
            }
        }
    }

    pub fn save(&self, project_root: &Path) -> Result<(), String> {
        let path = project_path(project_root);
        let directory = path
            .parent()
            .ok_or_else(|| "project path has no parent directory".to_string())?;
        fs::create_dir_all(directory)
            .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
        let json = serde_json::to_string_pretty(self)
            .map_err(|error| format!("could not serialize project: {error}"))?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, format!("{json}\n"))
            .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
        fs::rename(&temporary, &path)
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

    pub fn nearest_available_start(
        &self,
        track_id: u64,
        ignored_clip_id: Option<u64>,
        desired_start: f64,
        duration: f64,
    ) -> f64 {
        let desired_start = desired_start.max(0.0);
        let duration = duration.max(MIN_CLIP_DURATION);
        let mut occupied = self
            .clips_on_track(track_id)
            .filter(|clip| Some(clip.id) != ignored_clip_id)
            .map(|clip| (clip.timeline_start, clip.timeline_end()))
            .collect::<Vec<_>>();
        occupied.sort_by(|left, right| left.0.total_cmp(&right.0));

        let mut candidates = Vec::new();
        let mut gap_start = 0.0_f64;
        for (occupied_start, occupied_end) in occupied {
            let latest_start = occupied_start - duration;
            if latest_start >= gap_start {
                candidates.push(desired_start.clamp(gap_start, latest_start));
            }
            gap_start = gap_start.max(occupied_end);
        }
        candidates.push(desired_start.max(gap_start));
        candidates
            .into_iter()
            .min_by(|left, right| {
                (left - desired_start)
                    .abs()
                    .total_cmp(&(right - desired_start).abs())
            })
            .unwrap_or(desired_start)
    }

    pub fn trim_limits(&self, clip_id: u64) -> Option<(f64, f64)> {
        let clip = self.clip(clip_id)?;
        let previous_end = self
            .clips_on_track(clip.track_id)
            .filter(|other| other.id != clip_id && other.timeline_start < clip.timeline_start)
            .map(TimelineClip::timeline_end)
            .fold(0.0, f64::max);
        let next_start = self
            .clips_on_track(clip.track_id)
            .filter(|other| other.id != clip_id && other.timeline_start >= clip.timeline_end())
            .map(|other| other.timeline_start)
            .min_by(f64::total_cmp)
            .unwrap_or(f64::INFINITY);
        Some((previous_end, next_start))
    }

    /// The end of the rendered content, ignoring markers past the last clip.
    pub fn content_duration(&self) -> f64 {
        self.clips
            .iter()
            .map(TimelineClip::timeline_end)
            .fold(0.0, f64::max)
    }

    /// How far the timeline is scrubbable, which includes markers left beyond the clips.
    pub fn timeline_duration(&self) -> f64 {
        self.markers
            .iter()
            .map(|marker| marker.time)
            .fold(self.content_duration(), f64::max)
    }

    pub fn visual_clip_at_time(&self, time: f64) -> Option<&TimelineClip> {
        self.tracks
            .iter()
            .filter(|track| track.visible && track.kind == TrackKind::Video)
            .find_map(|track| {
                self.clips_on_track(track.id)
                    .filter(|clip| {
                        clip.contains(time)
                            && clip
                                .asset_id
                                .and_then(|id| self.asset(id))
                                .is_some_and(|asset| asset.kind == MediaKind::Video)
                    })
                    .max_by(|left, right| left.timeline_start.total_cmp(&right.timeline_start))
            })
    }

    pub fn next_id(&self) -> u64 {
        self.assets
            .iter()
            .map(|asset| asset.id)
            .chain(self.tracks.iter().map(|track| track.id))
            .chain(self.clips.iter().map(|clip| clip.id))
            .chain(self.markers.iter().map(|marker| marker.id))
            .max()
            .unwrap_or(0)
            + 1
    }

    fn normalize(&mut self) {
        self.version = PROJECT_VERSION;
        if self.tracks.is_empty() {
            self.tracks = default_tracks();
        }
        self.clips.retain(|clip| {
            self.tracks.iter().any(|track| track.id == clip.track_id)
                && clip
                    .asset_id
                    .is_some_and(|id| self.assets.iter().any(|asset| asset.id == id))
                && clip.timeline_start.is_finite()
                && clip.timeline_start >= 0.0
                && clip.source_in.is_finite()
                && clip.source_out.is_finite()
                && clip.source_out - clip.source_in >= MIN_CLIP_DURATION
        });
        for clip in &mut self.clips {
            if let Some(asset) = clip
                .asset_id
                .and_then(|id| self.assets.iter().find(|asset| asset.id == id))
            {
                let maximum_in = (asset.duration - MIN_CLIP_DURATION).max(0.0);
                clip.source_in = clip.source_in.clamp(0.0, maximum_in);
                clip.source_out = clip
                    .source_out
                    .clamp(clip.source_in + MIN_CLIP_DURATION, asset.duration);
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
                    .total_cmp(&self.clips[*right].timeline_start)
                    .then_with(|| self.clips[*left].id.cmp(&self.clips[*right].id))
            });
            let mut next_available = 0.0_f64;
            for index in indices {
                self.clips[index].timeline_start =
                    self.clips[index].timeline_start.max(next_available);
                next_available = self.clips[index].timeline_end();
            }
        }
        self.markers
            .retain(|marker| marker.time.is_finite() && marker.time >= 0.0);
    }
}

fn default_visible() -> bool {
    true
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
    let framerate = rational_to_f64(stream.avg_frame_rate())
        .filter(|fps| fps.is_finite() && *fps > 0.0)
        .unwrap_or(30.0);
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
    if duration < MIN_CLIP_DURATION {
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
        duration: DEFAULT_IMAGE_DURATION,
        width,
        height,
        framerate: 0.0,
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
        codec: stream.parameters().id().name().to_string(),
        has_audio: true,
    })
}

fn rational_to_f64(value: ffmpeg::Rational) -> Option<f64> {
    let denominator = value.denominator();
    (denominator != 0).then(|| value.numerator() as f64 / denominator as f64)
}

fn project_path(project_root: &Path) -> PathBuf {
    project_root.join(".opencut/project.json")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video_clip(id: u64, start: f64, duration: f64) -> TimelineClip {
        TimelineClip {
            id,
            track_id: 1,
            asset_id: Some(100),
            timeline_start: start,
            source_in: 0.0,
            source_out: duration,
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
            codec: "h264".into(),
            has_audio: true,
        }
    }

    #[test]
    fn finds_the_nearest_gap_without_overlapping_a_track() {
        let project = Project {
            assets: vec![video_asset()],
            clips: vec![video_clip(10, 0.0, 5.0), video_clip(11, 10.0, 5.0)],
            ..Project::default()
        };

        assert_eq!(project.nearest_available_start(1, None, 4.0, 3.0), 5.0);
        assert_eq!(project.nearest_available_start(1, None, 8.0, 3.0), 7.0);
        assert_eq!(project.nearest_available_start(1, None, 14.0, 3.0), 15.0);
    }

    #[test]
    fn repairs_overlapping_clips_when_loading_a_project() {
        let mut project = Project {
            assets: vec![video_asset()],
            clips: vec![video_clip(10, 0.0, 5.0), video_clip(11, 3.0, 4.0)],
            ..Project::default()
        };

        project.normalize();

        assert_eq!(project.clips[0].timeline_start, 0.0);
        assert_eq!(project.clips[1].timeline_start, 5.0);
    }

    #[test]
    fn keeps_markers_out_of_the_rendered_duration() {
        let project = Project {
            assets: vec![video_asset()],
            clips: vec![video_clip(10, 0.0, 5.0)],
            markers: vec![TimelineMarker {
                id: 20,
                time: 42.0,
                label: "Marker 1".into(),
            }],
            ..Project::default()
        };

        assert_eq!(project.content_duration(), 5.0);
        assert_eq!(project.timeline_duration(), 42.0);
    }
}
