use clap::{Parser, Subcommand, ValueEnum};
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
    /// Inspect a media file's streams, duration, and keyframe spacing.
    Probe { media_file: PathBuf },
    /// Create a version 1 timeline with video and audio tracks.
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
    /// Print the authoritative timeline JSON Schema.
    Schema {
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
    /// Apply an atomic, validated edit and return affected entities.
    Edit {
        timeline: PathBuf,
        #[command(subcommand)]
        command: Edit,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Kind {
    Video,
    Audio,
    Text,
}

#[derive(Subcommand)]
pub enum Edit {
    /// Add a video, audio, or text track.
    AddTrack {
        #[arg(long, value_enum)]
        kind: Kind,
        #[arg(long)]
        name: Option<String>,
    },
    /// Register media or an image and place a clip on a track.
    AddClip {
        #[arg(long)]
        track: String,
        #[arg(long)]
        asset: PathBuf,
        #[arg(long)]
        at: String,
        #[arg(long = "in", default_value = "0")]
        source_in: String,
        #[arg(long = "out")]
        source_out: Option<String>,
    },
    /// Place a static text clip.
    AddText {
        #[arg(long)]
        track: String,
        #[arg(long)]
        text: String,
        #[arg(long)]
        at: String,
        #[arg(long)]
        duration: String,
        #[arg(long, default_value = "Sans")]
        font: String,
        #[arg(long, default_value_t = 72.0)]
        size: f64,
        #[arg(long, default_value = "#ffffff")]
        color: String,
        #[arg(long, default_value = "0.5,0.5")]
        pos: String,
    },
    /// Change a clip's timeline start and optionally its track.
    MoveClip {
        #[arg(long)]
        clip: String,
        #[arg(long)]
        to: String,
        #[arg(long)]
        track: Option<String>,
    },
    /// Change media source in/out points without moving the clip.
    TrimClip {
        #[arg(long)]
        clip: String,
        #[arg(long = "in")]
        source_in: Option<String>,
        #[arg(long = "out")]
        source_out: Option<String>,
    },
    /// Split a clip at an absolute timeline time.
    SplitClip {
        #[arg(long)]
        clip: String,
        #[arg(long)]
        at: String,
    },
    /// Remove a clip and its associated transitions.
    RemoveClip {
        #[arg(long)]
        clip: String,
    },
    /// Set a static property using a dotted path and JSON value.
    Set {
        #[arg(long)]
        clip: String,
        #[arg(long)]
        property: String,
        #[arg(long)]
        value: String,
    },
}
