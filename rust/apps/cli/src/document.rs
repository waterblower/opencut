//! CLI-owned file I/O for the shared timeline format.
use anyhow::{Context as _, Result, anyhow, ensure};
use serde_json::{Value, json};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
pub use timeline::TimelineEditingState;
use timeline::TrackKind;

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
        clips.push(json!({"id": clip.id(), "track_id": clip.track_id(), "start_frame": i64::from(clip.timeline_start()), "end_frame": i64::from(clip.timeline_end(fps)), "start_s": fps.seconds(clip.timeline_start()), "duration_s": fps.seconds(clip.frame_length(fps)), "asset_id": clip.asset_id()}));
    }
    for track in &doc.tracks {
        let mut members: Vec<_> = doc
            .clips
            .iter()
            .filter(|c| c.track_id() == track.id)
            .collect();
        members.sort_by_key(|c| i64::from(c.timeline_start()));
        let mut gaps = Vec::new();
        let mut end = 0;
        for clip in members {
            if i64::from(clip.timeline_start()) > end {
                gaps.push(
                    json!({"start_frame": end, "end_frame": i64::from(clip.timeline_start())}),
                );
            }
            end = i64::from(clip.timeline_end(fps));
        }
        if end < i64::from(doc.content_duration()) {
            gaps.push(json!({"start_frame": end, "end_frame": i64::from(doc.content_duration())}));
        }
        tracks.push(json!({"id": track.id, "kind": match track.kind { TrackKind::Video => "Video", TrackKind::Audio => "Audio", TrackKind::Text => "Text" }, "name": track.name, "muted": track.muted, "gaps": gaps}));
    }
    for asset in &doc.assets {
        let used: Vec<_> = doc
            .clips
            .iter()
            .filter(|c| c.asset_id().is_some_and(|id| id == asset.id))
            .map(|c| c.id())
            .collect();
        assets.push(json!({"id": asset.id, "path": asset.path, "clips": used}));
    }
    json!({"frames": i64::from(doc.content_duration()), "duration_s": fps.seconds(doc.content_duration()), "settings": {"width": doc.settings.width, "height": doc.settings.height, "audio_sample_rate": doc.settings.audio_sample_rate, "frame_rate": {"numerator": fps.numerator, "denominator": fps.denominator}}, "tracks": tracks, "clips": clips, "assets": assets})
}

fn write_bytes(path: &Path, bytes: &[u8], overwrite: bool) -> Result<()> {
    let mut temporary_name = path.as_os_str().to_os_string();
    temporary_name.push(".tmp");
    let temporary_path = PathBuf::from(temporary_name);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary_path)
        .with_context(|| format!("Creating temporary file {}", temporary_path.display()))?;
    let result = (|| {
        file.write_all(bytes)
            .with_context(|| format!("Writing temporary file {}", temporary_path.display()))?;
        file.sync_all()
            .with_context(|| format!("Syncing temporary file {}", temporary_path.display()))?;
        drop(file);
        if !overwrite {
            ensure!(
                !path.try_exists()? && !path.is_symlink(),
                "Output already exists: {}",
                path.display()
            );
        }
        // 临时文件与目标同目录，普通重命名兼容不支持排他重命名的外置磁盘。
        fs::rename(&temporary_path, path).with_context(|| {
            format!(
                "Renaming {} to {}",
                temporary_path.display(),
                path.display()
            )
        })?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}
