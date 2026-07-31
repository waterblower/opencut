use super::model::{MediaKind, Project};
use std::{path::Path, process::Command};

pub(super) fn export_project(
    project: &Project,
    project_root: &Path,
    output: &Path,
) -> Result<(), String> {
    if project.timeline.is_empty() {
        return Err("Add at least one clip before exporting.".to_string());
    }
    let first_clip = &project.timeline[0];
    let first_asset = project
        .asset(first_clip.asset_id)
        .ok_or_else(|| "The first timeline clip has no source media.".to_string())?;
    let width = even(first_asset.width.max(2));
    let height = even(first_asset.height.max(2));
    let framerate = project
        .timeline
        .iter()
        .filter_map(|clip| project.asset(clip.asset_id))
        .find(|asset| asset.kind == MediaKind::Video)
        .map(|asset| asset.framerate.clamp(1.0, 60.0))
        .unwrap_or(30.0);

    let mut command = Command::new("ffmpeg");
    command.args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"]);
    for clip in &project.timeline {
        let asset = project
            .asset(clip.asset_id)
            .ok_or_else(|| format!("Clip {} has no source media.", clip.id))?;
        if asset.kind == MediaKind::Image {
            command
                .args(["-loop", "1", "-t"])
                .arg(decimal(clip.duration()))
                .arg("-i")
                .arg(project_root.join(&asset.path));
        } else {
            command
                .arg("-ss")
                .arg(decimal(clip.source_in))
                .arg("-t")
                .arg(decimal(clip.duration()))
                .arg("-i")
                .arg(project_root.join(&asset.path));
        }
    }

    let mut filters = Vec::new();
    let mut concat_inputs = String::new();
    for (index, clip) in project.timeline.iter().enumerate() {
        let asset = project.asset(clip.asset_id).unwrap();
        filters.push(format!(
            "[{index}:v:0]scale={width}:{height}:force_original_aspect_ratio=decrease,pad={width}:{height}:(ow-iw)/2:(oh-ih)/2:black,setsar=1,fps={},setpts=PTS-STARTPTS[v{index}]",
            decimal(framerate)
        ));
        if asset.has_audio {
            filters.push(format!(
                "[{index}:a:0]aformat=sample_rates=48000:channel_layouts=stereo,asetpts=PTS-STARTPTS[a{index}]"
            ));
        } else {
            filters.push(format!(
                "anullsrc=r=48000:cl=stereo,atrim=duration={},asetpts=PTS-STARTPTS[a{index}]",
                decimal(clip.duration())
            ));
        }
        concat_inputs.push_str(&format!("[v{index}][a{index}]"));
    }
    filters.push(format!(
        "{concat_inputs}concat=n={}:v=1:a=1[vout][aout]",
        project.timeline.len()
    ));

    let result = command
        .arg("-filter_complex")
        .arg(filters.join(";"))
        .args(["-map", "[vout]", "-map", "[aout]"])
        .args(["-c:v", "libx264", "-preset", "medium", "-crf", "18"])
        .args(["-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k"])
        .args(["-movflags", "+faststart"])
        .arg(output)
        .output()
        .map_err(|error| format!("could not start FFmpeg: {error}"))?;

    if result.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&result.stderr);
        let detail = tail_chars(stderr.trim(), 2_000);
        Err(if detail.is_empty() {
            format!("FFmpeg exited with {}.", result.status)
        } else {
            format!("FFmpeg export failed: {detail}")
        })
    }
}

fn even(value: u32) -> u32 {
    value - value % 2
}

fn decimal(value: f64) -> String {
    format!("{value:.6}")
}

fn tail_chars(value: &str, limit: usize) -> String {
    let mut tail = value.chars().rev().take(limit).collect::<Vec<_>>();
    tail.reverse();
    tail.into_iter().collect()
}
