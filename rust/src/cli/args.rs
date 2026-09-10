use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "opencut",
    version,
    about = "Headless declarative video editing"
)]
pub struct Args {
    #[arg(long, global = true)]
    pub json: bool,
    /// Base directory for project-relative asset paths.
    #[arg(long, global = true, default_value = ".")]
    pub project_root: PathBuf,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Assemble explicit podcast cuts and camera choices into an editor timeline.
    Assemble {
        recipe: PathBuf,
        #[arg(short, long, required_unless_present = "dry_run")]
        output: Option<PathBuf>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        overwrite: bool,
    },
    /// Inspect a media file's streams, duration, and keyframe spacing.
    Probe { media_file: PathBuf },
    /// Create a shared editor timeline with video and audio tracks.
    New {
        timeline: PathBuf,
        #[arg(long, default_value_t = 1920)]
        width: u32,
        #[arg(long, default_value_t = 1080)]
        height: u32,
        #[arg(long, default_value = "30")]
        fps: String,
    },
    /// Check document semantics and referenced media; report all findings.
    Validate { timeline: PathBuf },
    /// Summarize duration, clips, tracks, gaps, and asset usage.
    Inspect { timeline: PathBuf },
    /// Print the authoritative timeline or assembly recipe JSON Schema.
    Schema {
        #[arg(long, default_value = "timeline", value_parser = ["timeline", "recipe"])]
        kind: String,
        #[arg(long, default_value = "json-schema", value_parser = ["json-schema"])]
        format: String,
    },
    /// Print the compact agent usage guide.
    Docs,
    /// Render one composited frame as PNG or JPEG.
    Still {
        timeline: PathBuf,
        #[arg(long)]
        at: String,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value_t = 1.0)]
        scale: f64,
        #[arg(long)]
        overwrite: bool,
    },
    /// Encode a timeline or range as video with stereo AAC audio.
    Render {
        timeline: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        range: Option<String>,
        #[arg(long, default_value_t = 1.0)]
        scale: f64,
        #[arg(long, default_value = "standard", value_parser = ["draft", "standard", "high"])]
        preset: String,
        #[arg(long, default_value = "h264", value_parser = ["h264", "hevc", "prores"])]
        video_codec: String,
        /// Video bitrate in bits/s (e.g. 1500000, 1500k, 1.5M); defaults to source.
        #[arg(long, alias = "video-bitrate")]
        bitrate: Option<String>,
        #[arg(long, default_value = "aac", value_parser = ["aac"])]
        audio_codec: String,
        #[arg(long, default_value = "bar", value_parser = ["json", "bar", "none"])]
        progress: String,
        #[arg(long)]
        overwrite: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        no_metadata: bool,
    },
}
