use super::{
    audio::Mixer,
    compose::Composer,
    encode::{self, EncoderWorker},
    probe,
};
use crate::timeline::{Clip, TimelineTime, TrackKind};
use crate::{
    cli::{
        document::Document,
        error::Result,
        validate::{MediaInfo, require_valid},
    },
    cli_error, cli_try,
};
use image::{DynamicImage, ImageFormat, imageops};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    sync::mpsc::SyncSender,
    time::{Duration, Instant},
};
use ulid::Ulid;

pub struct Options {
    pub start: i64,
    pub end: i64,
    pub scale: f64,
    pub preset: String,
    pub video_codec: String,
    pub bitrate: Option<u64>,
    pub overwrite: bool,
    pub metadata: Option<String>,
}

pub fn summary(doc: &Document) -> Value {
    let fps = doc.settings.frame_rate;
    let mut clips = Vec::new();
    let mut tracks = Vec::new();
    let mut assets = Vec::new();
    for clip in &doc.clips {
        clips.push(json!({"id": clip.id(), "track_id": clip.track_id(), "start_frame": clip.timeline_start().frames(), "end_frame": clip.timeline_end(fps).frames(), "start_s": fps.seconds(clip.timeline_start()), "duration_s": fps.seconds(clip.frame_length(fps)), "asset_id": clip.media().map(|data| data.asset_id)}));
    }
    for track in &doc.tracks {
        let mut members: Vec<_> = doc
            .clips
            .iter()
            .filter(|c| c.track_id() == track.id)
            .collect();
        members.sort_by_key(|c| c.timeline_start().frames());
        let mut gaps = Vec::new();
        let mut end = 0;
        for clip in members {
            if clip.timeline_start().frames() > end {
                gaps.push(json!({"start_frame": end, "end_frame": clip.timeline_start().frames()}));
            }
            end = clip.timeline_end(fps).frames();
        }
        if end < doc.content_duration().frames() {
            gaps.push(json!({"start_frame": end, "end_frame": doc.content_duration().frames()}));
        }
        tracks.push(json!({"id": track.id, "kind": track.kind, "name": track.name, "muted": track.muted, "gaps": gaps}));
    }
    for asset in &doc.assets {
        let used: Vec<_> = doc
            .clips
            .iter()
            .filter(|c| c.media().is_some_and(|data| data.asset_id == asset.id))
            .map(|c| c.id())
            .collect();
        assets.push(json!({"id": asset.id, "path": asset.path, "clips": used}));
    }
    json!({"frames": doc.content_duration().frames(), "duration_s": fps.seconds(doc.content_duration()), "settings": doc.settings, "tracks": tracks, "clips": clips, "assets": assets})
}

pub fn plan(
    doc: &Document,
    base: &Path,
    output: &Path,
    options: &Options,
    media: &HashMap<Ulid, MediaInfo>,
) -> Result<Value> {
    require_valid(doc, None)?;
    require_valid(doc, Some(media))?;
    if options.start < 0
        || options.end <= options.start
        || options.end > doc.content_duration().frames()
    {
        return Err(cli_error!(
            "invalid_range",
            "",
            3,
            "render range must be nonempty and within timeline"
        ));
    }
    let (width, height) = dimensions(doc.settings.width, doc.settings.height, options.scale)?;
    if width % 2 != 0 || height % 2 != 0 {
        return Err(cli_error!(
            "invalid_dimensions",
            "",
            3,
            "video encoding requires even output width and height"
        ));
    }
    let ext = output
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_ascii_lowercase();
    if (options.video_codec == "prores" && ext != "mov")
        || (options.video_codec != "prores" && ext != "mov" && ext != "mp4")
    {
        return Err(cli_error!(
            "invalid_container",
            "",
            2,
            "ProRes requires .mov; H.264/HEVC require .mov or .mp4"
        ));
    }
    protect_assets(doc, base, output)?;
    check_output(output, options.overwrite)?;
    encode::video_codec(&options.video_codec)?;
    let frames = options.end - options.start;
    let duration = doc
        .settings
        .frame_rate
        .seconds(TimelineTime::from_frames(frames));
    let (video_bitrate, bitrate_source) = resolve_bitrate(doc, media, options)?;
    let estimated_bitrate = if options.video_codec == "prores" {
        let base = match options.preset.as_str() {
            "draft" => 45_000_000.0,
            "high" => 220_000_000.0,
            _ => 147_000_000.0,
        };
        base * width as f64 * height as f64 / (1920.0 * 1080.0)
            * doc.settings.frame_rate.numerator as f64
            / doc.settings.frame_rate.denominator as f64
            / (30000.0 / 1001.0)
    } else {
        video_bitrate as f64
    };
    let mut active = 0;
    for clip in &doc.clips {
        if clip.timeline_start().frames() < options.end
            && clip.timeline_end(doc.settings.frame_rate).frames() > options.start
        {
            active += 1;
        }
    }
    Ok(
        json!({"path": output, "frames": frames, "duration_s": duration, "width": width, "height": height, "active_clip_count": active, "estimated_output_bytes": ((estimated_bitrate + 192000.0) * duration / 8.0) as u64, "video_codec": options.video_codec, "video_bitrate": video_bitrate, "bitrate_source": bitrate_source, "audio_codec": "aac"}),
    )
}

pub fn still(
    doc: &Document,
    base: &Path,
    frame: i64,
    output: &Path,
    scale: f64,
    overwrite: bool,
) -> Result<()> {
    require_valid(doc, None)?;
    let media = probe::assets(doc, base)?;
    require_valid(doc, Some(&media))?;
    if frame < 0 || frame >= doc.content_duration().frames() {
        return Err(cli_error!(
            "invalid_time",
            "",
            3,
            "still frame must be within the timeline"
        ));
    }
    let (width, height) = dimensions(doc.settings.width, doc.settings.height, scale)?;
    let format = cli_try!(ImageFormat::from_path(output), "invalid_container", "", 2);
    if !matches!(format, ImageFormat::Png | ImageFormat::Jpeg) {
        return Err(cli_error!(
            "invalid_container",
            "",
            2,
            "stills require PNG or JPEG"
        ));
    }
    protect_assets(doc, base, output)?;
    let temp = temporary(output, overwrite)?;
    let result = (|| {
        let mut composer = Composer::default();
        let image = composer.frame(doc, base, frame)?;
        let image = imageops::resize(&image, width, height, imageops::FilterType::Triangle);
        let image = if format == ImageFormat::Jpeg {
            DynamicImage::ImageRgb8(DynamicImage::ImageRgba8(image).to_rgb8())
        } else {
            DynamicImage::ImageRgba8(image)
        };
        cli_try!(image.save_with_format(&temp, format), "io_error", "", 6);
        commit(&temp, output, overwrite)
    })();
    if temp.exists() {
        cli_try!(fs::remove_file(&temp), "io_error", "", 6);
    }
    result
}

pub fn render(
    doc: &Document,
    base: &Path,
    output: &Path,
    options: &Options,
    progress: SyncSender<Value>,
) -> Result<Value> {
    require_valid(doc, None)?;
    let media = probe::assets(doc, base)?;
    let plan = plan(doc, base, output, options, &media)?;
    let (width, height) = dimensions(doc.settings.width, doc.settings.height, options.scale)?;
    let (bitrate, _) = resolve_bitrate(doc, &media, options)?;
    let temp = temporary(output, options.overwrite)?;
    let result = (|| {
        let fps = doc.settings.frame_rate;
        let rate = doc.settings.audio_sample_rate;
        let mut encoder = EncoderWorker::open(
            temp.clone(),
            (width, height),
            fps,
            rate,
            encode::VideoEncoding {
                codec: options.video_codec.clone(),
                preset: options.preset.clone(),
                bitrate,
            },
            options.metadata.clone(),
        )?;
        let mut composer = Composer::default();
        let mut mixer = Mixer::default();
        let total = options.end - options.start;
        let audio_start = fps.samples(options.start, rate);
        let audio_total = fps.samples(options.end, rate) - audio_start;
        let mut audio_at = 0;
        let clock = Instant::now();
        let mut next_progress = clock + Duration::from_secs(5);
        for frame in 0..total {
            let image = composer.frame(doc, base, options.start + frame)?;
            let image = if image.dimensions() == (width, height) {
                image
            } else {
                imageops::resize(&image, width, height, imageops::FilterType::Triangle)
            };
            encoder.video(image, frame)?;
            let audio_end =
                (fps.samples(options.start + frame + 1, rate) - audio_start).min(audio_total);
            while audio_at < audio_end {
                let count = (encoder.audio_frame_size as i64).min(audio_total - audio_at) as usize;
                let samples = mixer.block(doc, base, &media, audio_start + audio_at, count)?;
                encoder.audio(samples, audio_at)?;
                audio_at += count as i64;
            }
            let now = Instant::now();
            if now >= next_progress && frame + 1 < total {
                let speed =
                    (frame + 1) as f64 / now.duration_since(clock).as_secs_f64().max(0.000001);
                cli_try!(progress.send(json!({"frame": frame + 1, "total": total, "fps": speed, "eta_s": (total - frame - 1) as f64 / speed})), "render_cancelled", "", 5);
                next_progress = now + Duration::from_secs(5);
            }
        }
        encoder.finish()?;
        commit(&temp, output, options.overwrite)?;
        let speed = total as f64 / clock.elapsed().as_secs_f64().max(0.000001);
        cli_try!(
            progress.send(json!({"frame": total, "total": total, "fps": speed, "eta_s": 0.0})),
            "render_cancelled",
            "",
            5
        );
        Ok(plan)
    })();
    if temp.exists() {
        cli_try!(fs::remove_file(&temp), "io_error", "", 6);
    }
    result
}

pub fn parse_bitrate(value: &str) -> Result<u64> {
    let (number, multiplier) = match value.as_bytes().last() {
        Some(b'k' | b'K') => (&value[..value.len() - 1], 1000.0),
        Some(b'm' | b'M') => (&value[..value.len() - 1], 1_000_000.0),
        _ => (value, 1.0),
    };
    let number: f64 = cli_try!(number.parse(), "invalid_bitrate", "", 2);
    let bitrate = number * multiplier;
    if !bitrate.is_finite() || bitrate < 1.0 || bitrate >= i64::MAX as f64 {
        return Err(cli_error!(
            "invalid_bitrate",
            "",
            2,
            "bitrate must be a positive, representable number of bits per second"
        ));
    }
    Ok(bitrate.round() as u64)
}

pub fn resolve_bitrate(
    doc: &Document,
    media: &HashMap<Ulid, MediaInfo>,
    options: &Options,
) -> Result<(u64, &'static str)> {
    if let Some(bitrate) = options.bitrate {
        if bitrate == 0 || bitrate > i64::MAX as u64 {
            return Err(cli_error!(
                "invalid_bitrate",
                "",
                2,
                "bitrate must be positive and fit a signed 64-bit integer"
            ));
        }
        return Ok((bitrate, "explicit"));
    }
    let mut weighted = 0_u128;
    let mut frames = 0_u128;
    for clip in &doc.clips {
        let Clip::Video(data) = clip else {
            continue;
        };
        if !doc.tracks.iter().any(|track| {
            track.id == data.track_id && track.kind == TrackKind::Video && track.visible
        }) {
            continue;
        }
        let length = clip
            .timeline_end(doc.settings.frame_rate)
            .frames()
            .min(options.end)
            - data.timeline_start.frames().max(options.start);
        if length <= 0 {
            continue;
        }
        let Some(info) = media.get(&data.asset_id) else {
            continue;
        };
        let Some(bitrate) = info.video_bitrate else {
            continue;
        };
        if bitrate == 0 {
            continue;
        }
        weighted += bitrate as u128 * length as u128;
        frames += length as u128;
    }
    if let Some(bitrate) = (weighted + frames / 2).checked_div(frames) {
        return Ok((bitrate as u64, "source"));
    }
    let (width, height) = dimensions(doc.settings.width, doc.settings.height, options.scale)?;
    Ok((
        encode::bitrate(width, height, doc.settings.frame_rate, &options.preset) as u64,
        "preset",
    ))
}

fn dimensions(width: u32, height: u32, scale: f64) -> Result<(u32, u32)> {
    if !scale.is_finite() || scale <= 0.0 {
        return Err(cli_error!(
            "invalid_scale",
            "",
            2,
            "scale must be finite and positive"
        ));
    }
    let width = (width as f64 * scale).round();
    let height = (height as f64 * scale).round();
    if !(1.0..=16384.0).contains(&width) || !(1.0..=16384.0).contains(&height) {
        return Err(cli_error!(
            "invalid_dimensions",
            "",
            3,
            "scaled dimensions must be 1..16384"
        ));
    }
    Ok((width as u32, height as u32))
}

fn protect_assets(doc: &Document, base: &Path, output: &Path) -> Result<()> {
    if let Ok(target) = fs::canonicalize(output) {
        for asset in &doc.assets {
            if let Ok(source) = fs::canonicalize(base.join(&asset.path))
                && source == target
            {
                return Err(cli_error!(
                    "output_is_source",
                    "",
                    6,
                    "output would overwrite source media"
                ));
            }
        }
    }
    Ok(())
}

fn temporary(output: &Path, overwrite: bool) -> Result<PathBuf> {
    check_output(output, overwrite)?;
    let parent = output.parent().unwrap_or(Path::new("."));
    let extension = output.extension().unwrap_or_default().to_string_lossy();
    let temp = parent.join(format!(".opencut-{}.{}", Ulid::generate(), extension));
    cli_try!(
        OpenOptions::new().write(true).create_new(true).open(&temp),
        "io_error",
        "",
        6
    );
    Ok(temp)
}

fn commit(temp: &Path, output: &Path, overwrite: bool) -> Result<()> {
    let file = cli_try!(OpenOptions::new().write(true).open(temp), "io_error", "", 6);
    cli_try!(file.sync_all(), "io_error", "", 6);
    if overwrite {
        cli_try!(fs::rename(temp, output), "io_error", "", 6);
    } else {
        cli_try!(fs::hard_link(temp, output), "io_error", "", 6);
    }
    Ok(())
}

fn check_output(output: &Path, overwrite: bool) -> Result<()> {
    if output.exists() && !overwrite {
        return Err(cli_error!(
            "output_exists",
            "",
            6,
            "use --overwrite to replace {}",
            output.display()
        ));
    }
    Ok(())
}
