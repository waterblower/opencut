//! Compile explicit podcast decisions into the shared editor document.
use crate::{
    cli::{
        document::{self, Document},
        engine::probe,
        validate,
    },
    timeline::*,
};
use anyhow::{Context as _, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
use ulid::Ulid;

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub settings: TimelineSettings,
    /// Seconds per tick. All recipe times, including offsets, use this base.
    pub time_base: TimeBase,
    pub sources: Vec<Source>,
    pub master_audio: String,
    #[serde(default)]
    pub gain_db: f64,
    pub retained: Vec<Interval>,
    pub cameras: Vec<Camera>,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimeBase {
    pub numerator: u32,
    pub denominator: u32,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub name: String,
    pub path: PathBuf,
    /// source_time = episode_time + source_offset.
    pub source_offset: i64,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Interval {
    pub start: i64,
    pub end: i64,
}

#[derive(Debug, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Camera {
    pub source: String,
    pub start: i64,
    pub end: i64,
}

pub fn run(
    input: &Path,
    base: &Path,
    output: Option<&Path>,
    dry_run: bool,
    overwrite: bool,
) -> Result<Value> {
    let bytes = fs::read(input).context(format!("io_error at {}:{}", file!(), line!()))?;
    let recipe: Recipe = serde_json::from_slice(&bytes).context(format!(
        "invalid_recipe at {}:{}",
        file!(),
        line!()
    ))?;
    let mut media = Vec::new();
    for (i, source) in recipe.sources.iter().enumerate() {
        let info = match probe::probe(&base.join(&source.path)) {
            Ok(info) => info,
            Err(error) => {
                return Err(error.context(format!("/sources/{i}/path at {}:{}", file!(), line!())));
            }
        };
        media.push(info);
    }
    let (mut doc, report) = compile(&recipe, &media)?;
    if let Some(output) = output {
        if output.exists() {
            let target =
                fs::canonicalize(output).context(format!("io_error at {}:{}", file!(), line!()))?;
            let recipe_path =
                fs::canonicalize(input).context(format!("io_error at {}:{}", file!(), line!()))?;
            if target == recipe_path {
                return Err(anyhow!(
                    "output_is_source: output would overwrite the recipe at {}:{}",
                    file!(),
                    line!()
                ));
            }
            for source in &recipe.sources {
                let path = fs::canonicalize(base.join(&source.path)).context(format!(
                    "io_error at {}:{}",
                    file!(),
                    line!()
                ))?;
                if target == path {
                    return Err(anyhow!(
                        "output_is_source: output would overwrite source media at {}:{}",
                        file!(),
                        line!()
                    ));
                }
            }
            if !overwrite {
                return Err(anyhow!(
                    "output_exists: {} already exists at {}:{}",
                    output.display(),
                    file!(),
                    line!()
                ));
            }
        }
    }
    if !dry_run {
        let Some(output) = output else {
            return Err(anyhow!(
                "usage_error: assemble requires --output unless --dry-run is supplied at {}:{}",
                file!(),
                line!()
            ));
        };
        let timeline_dir = document::asset_base(output)?;
        for asset in &mut doc.assets {
            let source = std::path::absolute(base.join(&asset.path)).context(format!(
                "could not resolve source path {} at {}:{}",
                asset.path.display(),
                file!(),
                line!()
            ))?;
            asset.path = match source.strip_prefix(&timeline_dir) {
                Ok(relative) => relative.to_path_buf(),
                Err(_) => source,
            };
        }
        let value = serde_json::to_value(&doc).context(format!(
            "serialization_error at {}:{}",
            file!(),
            line!()
        ))?;
        document::write_atomic(output, &value, overwrite)?;
    }
    Ok(json!({"path": output, "dry_run": dry_run, "report": report}))
}

pub fn compile(recipe: &Recipe, media: &[probe::Probe]) -> Result<(Document, Value)> {
    let mut doc = Document {
        settings: recipe.settings,
        ..Document::default()
    };
    validate::require_valid(&doc, None)?;
    if recipe.time_base.numerator == 0 || recipe.time_base.denominator == 0 {
        return Err(anyhow!(
            "invalid_time_base: time base components must be positive (/time_base) at {}:{}",
            file!(),
            line!()
        ));
    }
    if !recipe.gain_db.is_finite() || !(-96.0..=24.0).contains(&recipe.gain_db) {
        return Err(anyhow!(
            "invalid_gain: gain must be -96..24 dB (/gain_db) at {}:{}",
            file!(),
            line!()
        ));
    }
    if recipe.sources.len() != media.len() || recipe.sources.is_empty() {
        return Err(anyhow!(
            "invalid_sources: each source requires media metadata (/sources) at {}:{}",
            file!(),
            line!()
        ));
    }
    let mut names = HashMap::new();
    let mut offsets = Vec::new();
    let mut rounding = Vec::new();
    for (i, source) in recipe.sources.iter().enumerate() {
        if source.name.is_empty() || names.insert(source.name.as_str(), i).is_some() {
            return Err(anyhow!(
                "duplicate_source: source names must be nonempty and unique ({}) at {}:{}",
                &format!("/sources/{i}/name"),
                file!(),
                line!()
            ));
        }
        let info = &media[i];
        let mut video = None;
        let mut audio = None;
        for stream in &info.streams {
            let slot = match stream.kind.as_str() {
                "video" => &mut video,
                "audio" => &mut audio,
                _ => continue,
            };
            if slot.is_some() {
                return Err(anyhow!(
                    "unsupported_streams: only one video and one audio stream per source are supported; extract the intended stream first ({}) at {}:{}",
                    &format!("/sources/{i}/path"),
                    file!(),
                    line!()
                ));
            }
            *slot = Some(stream);
        }
        if probe::is_image(&source.path)
            || (video.is_none() && audio.is_none())
            || !info.duration.is_finite()
            || info.duration <= 0.0
        {
            return Err(anyhow!(
                "invalid_source: source must be a recording with known positive duration ({}) at {}:{}",
                &format!("/sources/{i}/path"),
                file!(),
                line!()
            ));
        }
        let offset = quantize(
            source.source_offset,
            &recipe.time_base,
            recipe.settings.frame_rate,
            &format!("/sources/{i}/source_offset"),
            &mut rounding,
        )?;
        offsets.push(offset);
        let mut asset = MediaAsset {
            id: Ulid::generate(),
            kind: MediaKind::Audio,
            path: source.path.clone(),
            name: source.name.clone(),
            duration: info.duration,
            width: 0,
            height: 0,
            framerate: 0.0,
            frame_rate_numerator: 0,
            frame_rate_denominator: 0,
            codec: String::new(),
            has_audio: audio.is_some(),
        };
        if let Some(stream) = video {
            asset.kind = MediaKind::Video;
            asset.width = stream.width;
            asset.height = stream.height;
            asset.codec = stream.codec.clone();
            if let Some([n, d]) = stream.fps
                && n > 0
                && d > 0
            {
                asset.frame_rate_numerator = n as u32;
                asset.frame_rate_denominator = d as u32;
                asset.framerate = n as f64 / d as f64;
            }
        } else if let Some(stream) = audio {
            asset.codec = stream.codec.clone();
        }
        doc.assets.push(asset);
    }
    let Some(&master) = names.get(recipe.master_audio.as_str()) else {
        return Err(anyhow!(
            "unknown_source: master audio source does not exist (/master_audio) at {}:{}",
            file!(),
            line!()
        ));
    };
    if !doc.assets[master].has_audio || recipe.sources[master].source_offset != 0 {
        return Err(anyhow!(
            "invalid_master_audio: master must have audio and zero offset; it defines the episode clock (/master_audio) at {}:{}",
            file!(),
            line!()
        ));
    }
    if recipe.retained.is_empty() {
        return Err(anyhow!(
            "invalid_range: at least one retained interval is required (/retained) at {}:{}",
            file!(),
            line!()
        ));
    }
    let mut retained = Vec::new();
    let mut previous = 0;
    for (i, range) in recipe.retained.iter().enumerate() {
        let pointer = format!("/retained/{i}");
        let bounds = interval(
            range.start,
            range.end,
            previous,
            &recipe.time_base,
            recipe.settings.frame_rate,
            &pointer,
            &mut rounding,
        )?;
        previous = range.end;
        retained.push(bounds);
    }
    let mut cameras = Vec::new();
    previous = 0;
    for (i, camera) in recipe.cameras.iter().enumerate() {
        let pointer = format!("/cameras/{i}");
        let bounds = interval(
            camera.start,
            camera.end,
            previous,
            &recipe.time_base,
            recipe.settings.frame_rate,
            &pointer,
            &mut rounding,
        )?;
        previous = camera.end;
        let Some(&source) = names.get(camera.source.as_str()) else {
            return Err(anyhow!(
                "unknown_source: camera source does not exist ({pointer}) at {}:{}",
                file!(),
                line!()
            ));
        };
        if doc.assets[source].kind != MediaKind::Video {
            return Err(anyhow!(
                "invalid_camera: camera source requires video ({pointer}) at {}:{}",
                file!(),
                line!()
            ));
        }
        cameras.push((bounds.0, bounds.1, source));
    }
    for (kind, name) in [
        (TrackKind::Video, "Cameras"),
        (TrackKind::Audio, "Master audio"),
    ] {
        doc.tracks.push(Track {
            id: Ulid::generate(),
            name: name.into(),
            kind,
            locked: false,
            muted: false,
            visible: true,
        });
    }
    let mut mappings = Vec::new();
    let mut output_start = 0_i64;
    for (i, &(start, end)) in retained.iter().enumerate() {
        let pointer = format!("/retained/{i}");
        let output_end = output_start.checked_add(end - start).context(format!(
            "time_overflow: timeline duration overflow ({pointer}) at {}:{}",
            file!(),
            line!()
        ))?;
        append_clip(
            &mut doc,
            master,
            1,
            start,
            end,
            output_start,
            0,
            recipe.gain_db,
            &media[master].streams,
            &pointer,
        )?;
        let mut cursor = start;
        for &(camera_start, camera_end, source) in &cameras {
            let from = start.max(camera_start);
            let to = end.min(camera_end);
            if from >= to {
                continue;
            }
            if from != cursor {
                return Err(anyhow!(
                    "missing_camera_coverage: no camera at episode frame {cursor} ({pointer}) at {}:{}",
                    file!(),
                    line!()
                ));
            }
            append_clip(
                &mut doc,
                source,
                0,
                from,
                to,
                output_start + (from - start),
                offsets[source],
                0.0,
                &media[source].streams,
                &pointer,
            )?;
            cursor = to;
        }
        if cursor != end {
            return Err(anyhow!(
                "missing_camera_coverage: no camera at episode frame {cursor} ({pointer}) at {}:{}",
                file!(),
                line!()
            ));
        }
        mappings.push(json!({"episode_start": start, "episode_end": end, "output_start": output_start, "output_end": output_end}));
        output_start = output_end;
    }
    validate::require_valid(&doc, None)?;
    let mut clips = Vec::new();
    for clip in &doc.clips {
        let Some(data) = clip.media() else {
            continue;
        };
        let Some(asset) = doc.asset(data.asset_id) else {
            continue;
        };
        clips.push(json!({"source": asset.name, "kind": if data.audio_properties.muted { "video" } else { "audio" }, "source_in": data.source_in, "source_out": data.source_out, "output_start": data.timeline_start, "output_end": clip.timeline_end(recipe.settings.frame_rate)}));
    }
    Ok((
        doc,
        json!({"frame_rate": recipe.settings.frame_rate, "frames": output_start, "intervals": mappings, "clips": clips, "rounding": rounding}),
    ))
}

fn quantize(
    ticks: i64,
    time_base: &TimeBase,
    frame_rate: FrameRate,
    pointer: &str,
    rounding: &mut Vec<Value>,
) -> Result<i64> {
    let n = ticks as i128 * time_base.numerator as i128 * frame_rate.numerator as i128;
    let d = time_base.denominator as i128 * frame_rate.denominator as i128;
    let magnitude = (n.abs() + d / 2) / d;
    let frame = i64::try_from(if n < 0 { -magnitude } else { magnitude }).context(format!(
        "time_overflow ({pointer}) at {}:{}",
        file!(),
        line!()
    ))?;
    if n % d != 0 {
        rounding.push(json!({"pointer": pointer, "ticks": ticks, "effective_frames": frame}));
    }
    Ok(frame)
}

fn interval(
    start: i64,
    end: i64,
    previous: i64,
    time_base: &TimeBase,
    frame_rate: FrameRate,
    pointer: &str,
    rounding: &mut Vec<Value>,
) -> Result<(i64, i64)> {
    if start < previous || end <= start {
        return Err(anyhow!(
            "invalid_range: ranges must be nonnegative, ordered, nonoverlapping, and nonempty ({pointer}) at {}:{}",
            file!(),
            line!()
        ));
    }
    let a = quantize(
        start,
        time_base,
        frame_rate,
        &format!("{pointer}/start"),
        rounding,
    )?;
    let b = quantize(
        end,
        time_base,
        frame_rate,
        &format!("{pointer}/end"),
        rounding,
    )?;
    if a == b {
        return Err(anyhow!(
            "collapsed_range: range disappears at the project frame rate ({pointer}) at {}:{}",
            file!(),
            line!()
        ));
    }
    Ok((a, b))
}

fn append_clip(
    doc: &mut Document,
    source: usize,
    track: usize,
    start: i64,
    end: i64,
    output: i64,
    offset: i64,
    gain_db: f64,
    streams: &[probe::Stream],
    pointer: &str,
) -> Result<()> {
    let from = start.checked_add(offset).context(format!(
        "time_overflow: source time overflow ({pointer}) at {}:{}",
        file!(),
        line!()
    ))?;
    let to = end.checked_add(offset).context(format!(
        "time_overflow: source time overflow ({pointer}) at {}:{}",
        file!(),
        line!()
    ))?;
    let asset = &doc.assets[source];
    let mut duration = asset.duration;
    for stream in streams {
        if stream.kind == if track == 0 { "video" } else { "audio" }
            && let Some(stream_duration) = stream.duration
        {
            duration = duration.min(stream_duration);
        }
    }
    if from < 0
        || doc
            .settings
            .frame_rate
            .seconds(TimelineTime::from_frames(to))
            > duration + 1e-9
    {
        return Err(anyhow!(
            "source_out_of_bounds: {} lacks source frames [{from}, {to}) ({pointer}) at {}:{}",
            asset.name,
            file!(),
            line!()
        ));
    }
    let data = MediaClipData {
        id: Ulid::generate(),
        track_id: doc.tracks[track].id,
        asset_id: asset.id,
        timeline_start: TimelineTime::from_frames(output),
        source_in: TimelineTime::from_frames(from),
        source_out: TimelineTime::from_frames(to),
        video_properties: VideoClipProperties::default(),
        audio_properties: AudioClipProperties {
            gain_db,
            muted: track == 0,
        },
    };
    doc.clips.push(if track == 0 {
        Clip::Video(data)
    } else {
        Clip::Audio(data)
    });
    Ok(())
}
