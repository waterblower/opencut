use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "opencut",
    version,
    about = "Headless declarative video editing"
)]
pub(crate) struct Args {
    #[arg(long, global = true)]
    pub(crate) json: bool,
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Edit timeline documents without opening the editor.
    Timeline {
        #[command(subcommand)]
        command: TimelineCommand,
    },
    /// Export a timeline to MP4 with H.264 video and stereo AAC audio (macOS).
    Export(ExportArgs),
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
}

#[derive(clap::Args)]
pub(crate) struct ExportArgs {
    /// Timeline JSON file.
    pub(crate) timeline: PathBuf,
    /// Output MP4 file; must not already exist unless --overwrite is given.
    #[arg(short, long)]
    pub(crate) output: PathBuf,
    /// Base directory for relative assets; defaults to the timeline's directory.
    #[arg(long)]
    pub(crate) project_root: Option<PathBuf>,
    /// H.264 target bitrate in kbps (1 kbps = 1,000 bits per second).
    #[arg(long, short, default_value_t = 8_000, value_parser = clap::value_parser!(u64).range(1..=i64::MAX as u64 / 1_000))]
    pub(crate) bitrate: u64,
    /// Replace an existing output file; source media is never replaced.
    #[arg(long)]
    pub(crate) overwrite: bool,
}

#[derive(Subcommand)]
pub(crate) enum TimelineCommand {
    /// Keep time covered by text clips and close uncovered gaps across every track.
    KeepTextSections(TimelineEditArgs),
    /// Remove leading and internal gaps unoccupied by clips across every track.
    RemoveGaps(TimelineEditArgs),
}

#[derive(clap::Args)]
#[command(group(clap::ArgGroup::new("destination").required(true).multiple(false).args(["output", "write_inplace"])))]
pub(crate) struct TimelineEditArgs {
    /// Input timeline (.timeline or .timeline.json).
    pub(crate) timeline: PathBuf,
    /// Write a new timeline; the destination must not already exist.
    #[arg(short, long)]
    pub(crate) output: Option<PathBuf>,
    /// Atomically replace the input timeline after validation succeeds.
    #[arg(long)]
    pub(crate) write_inplace: bool,
}
