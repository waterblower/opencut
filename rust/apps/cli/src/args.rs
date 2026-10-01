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
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Export a timeline to MP4 with H.264 video and stereo AAC audio (macOS).
    Export {
        /// Timeline JSON file.
        timeline: PathBuf,
        /// Output MP4 file; must not already exist unless --overwrite is given.
        #[arg(short, long)]
        output: PathBuf,
        /// Base directory for relative assets; defaults to the timeline's directory.
        #[arg(long)]
        project_root: Option<PathBuf>,
        /// H.264 target bitrate in kbps (1 kbps = 1,000 bits per second).
        #[arg(long, default_value_t = 8_000, value_parser = clap::value_parser!(u64).range(1..=i64::MAX as u64 / 1_000))]
        video_bitrate: u64,
        /// Replace an existing output file; source media is never replaced.
        #[arg(long)]
        overwrite: bool,
    },
    /// Transcribe audio/video with MiniMax (MINIMAX_API_KEY).
    Transcribe {
        media_file: PathBuf,
        #[arg(long, value_enum, default_value = "verbose_json")]
        format: crate::transcribe::Format,
        /// Merge SRT cues separated by less than 100 ms (requires --format srt).
        #[arg(long)]
        post_merge: bool,
        /// BCP-47 hint; omitted by default for mixed-language recognition.
        #[arg(long)]
        language: Option<String>,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long)]
        overwrite: bool,
    },
    /// Summarize a video, audio, image, or timeline JSON file.
    Probe { file: PathBuf },
    /// Check document semantics and referenced media; stop if a media file cannot be probed.
    Validate { timeline: PathBuf },
    /// Print the authoritative timeline JSON Schema.
    Schema {
        #[arg(long, default_value = "json-schema", value_parser = ["json-schema"])]
        format: String,
    },
    /// Print an agent-friendly Markdown guide to using the CLI.
    #[command(alias = "docs")]
    Doc,
}
