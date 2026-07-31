use ffmpeg::{codec, format, media::Type};
use ffmpeg_next as ffmpeg;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

pub(super) const MIN_CLIP_DURATION: f64 = 1.0 / 30.0;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub(super) struct MediaAsset {
    pub id: u64,
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
    pub asset_id: u64,
    pub source_in: f64,
    pub source_out: f64,
}

impl TimelineClip {
    pub fn duration(&self) -> f64 {
        (self.source_out - self.source_in).max(0.0)
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(super) struct Project {
    pub assets: Vec<MediaAsset>,
    pub timeline: Vec<TimelineClip>,
}

impl Project {
    pub fn load() -> Self {
        let path = project_path();
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

    pub fn save(&self) -> Result<(), String> {
        let path = project_path();
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
        self.timeline.iter().find(|clip| clip.id == id)
    }

    pub fn clip_index(&self, id: u64) -> Option<usize> {
        self.timeline.iter().position(|clip| clip.id == id)
    }

    pub fn timeline_start(&self, index: usize) -> f64 {
        self.timeline
            .iter()
            .take(index)
            .map(TimelineClip::duration)
            .sum()
    }

    pub fn timeline_duration(&self) -> f64 {
        self.timeline.iter().map(TimelineClip::duration).sum()
    }

    pub fn clip_at_time(&self, time: f64) -> Option<(usize, f64)> {
        let mut start = 0.0;
        for (index, clip) in self.timeline.iter().enumerate() {
            let end = start + clip.duration();
            if time < end || (index + 1 == self.timeline.len() && time <= end) {
                return Some((index, (time - start).clamp(0.0, clip.duration())));
            }
            start = end;
        }
        None
    }

    pub fn next_id(&self) -> u64 {
        self.assets
            .iter()
            .map(|asset| asset.id)
            .chain(self.timeline.iter().map(|clip| clip.id))
            .max()
            .unwrap_or(0)
            + 1
    }

    fn normalize(&mut self) {
        self.timeline.retain(|clip| {
            self.assets.iter().any(|asset| asset.id == clip.asset_id)
                && clip.source_in.is_finite()
                && clip.source_out.is_finite()
                && clip.source_out - clip.source_in >= MIN_CLIP_DURATION
        });
        for clip in &mut self.timeline {
            if let Some(asset) = self.assets.iter().find(|asset| asset.id == clip.asset_id) {
                let maximum_in = (asset.duration - MIN_CLIP_DURATION).max(0.0);
                clip.source_in = clip.source_in.clamp(0.0, maximum_in);
                clip.source_out = clip
                    .source_out
                    .clamp(clip.source_in + MIN_CLIP_DURATION, asset.duration);
            }
        }
    }
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

fn rational_to_f64(value: ffmpeg::Rational) -> Option<f64> {
    let denominator = value.denominator();
    (denominator != 0).then(|| value.numerator() as f64 / denominator as f64)
}

fn project_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/editor-project.json")
}
