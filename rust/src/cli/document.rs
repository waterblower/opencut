//! CLI-owned file I/O for the shared timeline format.
pub use crate::timeline::TimelineEditingState;
use crate::timeline::{ParseError, TrackKind};

pub fn parse(value: &Value) -> std::result::Result<TimelineEditingState, ParseError> {
    Ok(crate::timeline::parse(value)?.to_editing_state())
}
use anyhow::{Context as _, Result, anyhow};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use ulid::Ulid;

pub fn load(path: &Path) -> Result<(Value, TimelineEditingState)> {
    let contents = fs::read(path).context(format!("io_error at {}:{}", file!(), line!()))?;
    let value: Value = serde_json::from_slice(&contents).context(format!(
        "invalid_json at {}:{}",
        file!(),
        line!()
    ))?;
    let document = parse(&value)?;
    Ok((value, document))
}

pub fn asset_base(timeline: &Path) -> Result<PathBuf> {
    let path = std::path::absolute(timeline).context(format!(
        "could not resolve timeline path {} at {}:{}",
        timeline.display(),
        file!(),
        line!()
    ))?;
    let Some(parent) = path.parent() else {
        return Err(anyhow!(
            "timeline path has no parent: {} at {}:{}",
            path.display(),
            file!(),
            line!()
        ));
    };
    Ok(parent.to_path_buf())
}

pub fn write_atomic(path: &Path, value: &Value, overwrite: bool) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value).context(format!(
        "invalid_json at {}:{}",
        file!(),
        line!()
    ))?;
    write_bytes(path, &bytes, overwrite)
}

/// Publish transcript bytes without blocking the async runtime. Reuses the same
/// atomic publication and cleanup as synchronous document writes.
pub async fn write_atomic_bytes(path: &Path, bytes: Vec<u8>, overwrite: bool) -> Result<()> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || write_bytes(&path, &bytes, overwrite))
        .await
        .context(format!("io_error at {}:{}", file!(), line!()))?
}

pub fn summary(doc: &TimelineEditingState) -> Value {
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
        tracks.push(json!({"id": track.id, "kind": match track.kind { TrackKind::Video => "Video", TrackKind::Audio => "Audio", TrackKind::Text => "Text" }, "name": track.name, "muted": track.muted, "gaps": gaps}));
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
    json!({"frames": doc.content_duration().frames(), "duration_s": fps.seconds(doc.content_duration()), "settings": {"width": doc.settings.width, "height": doc.settings.height, "audio_sample_rate": doc.settings.audio_sample_rate, "frame_rate": {"numerator": fps.numerator, "denominator": fps.denominator}}, "tracks": tracks, "clips": clips, "assets": assets})
}

fn write_bytes(path: &Path, bytes: &[u8], overwrite: bool) -> Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    let temp = parent.join(format!(".opencut-{}.tmp", Ulid::generate()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .context(format!("io_error at {}:{}", file!(), line!()))?;
        file.write_all(bytes)
            .context(format!("io_error at {}:{}", file!(), line!()))?;
        file.sync_all()
            .context(format!("io_error at {}:{}", file!(), line!()))?;
        if overwrite {
            fs::rename(&temp, path).context(format!("io_error at {}:{}", file!(), line!()))?;
        } else {
            fs::hard_link(&temp, path).context(format!("io_error at {}:{}", file!(), line!()))?;
        }
        Ok(())
    })();
    if temp.exists() {
        fs::remove_file(&temp).context(format!("io_error at {}:{}", file!(), line!()))?;
    }
    result
}
