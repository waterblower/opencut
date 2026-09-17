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
    /// Base directory for assembly recipe sources; timeline assets use the timeline's directory.
    #[arg(long, global = true, default_value = ".")]
    pub project_root: PathBuf,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Transcribe audio/video with MiniMax (MINIMAX_API_KEY).
    Transcribe {
        media_file: PathBuf,
        #[arg(long, value_enum, default_value = "verbose_json")]
        format: opencut_player::cli::transcribe::Format,
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
    /// Summarize a video, audio, image, or timeline JSON file.
    Probe { file: PathBuf },
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
    /// Check document semantics and referenced media; stop if a media file cannot be probed.
    Validate { timeline: PathBuf },
    /// Print the authoritative timeline or assembly recipe JSON Schema.
    Schema {
        #[arg(long, default_value = "timeline", value_parser = ["timeline", "recipe"])]
        kind: String,
        #[arg(long, default_value = "json-schema", value_parser = ["json-schema"])]
        format: String,
    },
    /// Print an agent-friendly Markdown guide to using the CLI.
    #[command(alias = "docs")]
    Doc,
}
