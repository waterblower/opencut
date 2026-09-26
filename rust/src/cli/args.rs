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
    /// Render a five-second, 30 fps GPUI demo (macOS): white text on black.
    Render {
        #[arg(short, long, default_value = "output.mp4")]
        output: PathBuf,
    },
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
