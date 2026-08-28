use super::timeline::{FrameRate, TimelineSerialization};
use std::path::Path;

pub(super) const DEFAULT_VIDEO_BIT_RATE: usize = 8_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ExportEncoder {
    Hardware,
    Software,
}

impl ExportEncoder {
    pub(super) const fn default_for_platform() -> Self {
        if cfg!(target_os = "macos") {
            Self::Hardware
        } else {
            Self::Software
        }
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Hardware => "Hardware",
            Self::Software => "Software",
        }
    }

    pub(super) const fn implementation(self) -> &'static str {
        match self {
            Self::Hardware => "VideoToolbox",
            Self::Software => "x264",
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct ExportOptions {
    pub width: u32,
    pub height: u32,
    pub frame_rate: FrameRate,
    pub video_bit_rate: usize,
    pub encoder: ExportEncoder,
}

impl ExportOptions {
    pub fn from_timeline(timeline: &TimelineSerialization) -> Self {
        Self {
            width: timeline.settings.width,
            height: timeline.settings.height,
            frame_rate: timeline.settings.frame_rate,
            video_bit_rate: DEFAULT_VIDEO_BIT_RATE,
            encoder: ExportEncoder::Software,
        }
    }
}

pub(super) fn export_timeline(
    timeline: &TimelineSerialization,
    project_root: &Path,
    output: &Path,
    options: ExportOptions,
    report_progress: impl FnMut(f32),
) -> anyhow::Result<()> {
    super::export_gstreamer::export_timeline(
        timeline,
        project_root,
        output,
        options,
        report_progress,
    )
}
