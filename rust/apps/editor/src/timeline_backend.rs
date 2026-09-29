//! Synchronous timeline frame preparation. Snapshot reads never decode or advance time.

use crate::editor::preview_timeline::TimelinePreviewFrame;
use ::engine::timeline_decoder::{TimelineDecoder, TimelineFrame as DecodedTimelineFrame};
use ::timeline::{TimelineEditingState, TimelineFrame};
use anyhow::{Context as _, Result, bail};
use std::{path::Path, sync::Arc, time::Duration};

pub struct TimelineBackend {
    timeline: TimelineEditingState,
    decoder: TimelineDecoder,
    preview: Arc<TimelinePreviewFrame>,
}

impl TimelineBackend {
    /// Validates the document and prepares frame zero before returning.
    pub fn new(timeline: TimelineEditingState, project_root: &Path) -> Result<Self> {
        let metadata = project_root.metadata().context(format!(
            "Inspecting timeline media root {}",
            project_root.display()
        ))?;
        if !metadata.is_dir() {
            bail!(
                "Timeline media root is not a directory: {}",
                project_root.display()
            );
        }
        timeline.validate()?;
        let mut decoder = TimelineDecoder::new(project_root);
        let frame = Arc::new(decoder.frame_at(&timeline, Duration::ZERO)?);
        let preview = Arc::new(TimelinePreviewFrame::new(frame));
        Ok(Self {
            timeline,
            decoder,
            preview,
        })
    }

    pub fn timeline(&self) -> &TimelineEditingState {
        &self.timeline
    }

    /// Prepares edited content before publishing it. Errors preserve the document and frame.
    pub fn replace_timeline(&mut self, timeline: TimelineEditingState) -> Result<()> {
        timeline.validate()?;
        let position = clamp_position(&timeline, self.position())?;
        self.decoder.clear_cache();
        #[rustfmt::skip]
        let frame = match self.decoder.frame_at(&timeline, position) {
            Ok(frame) => {
                frame
            }
            Err(error) => {
                self.decoder.clear_cache();
                return Err(error);
            }
        };
        self.preview = Arc::new(TimelinePreviewFrame::new(Arc::new(frame)));
        self.timeline = timeline;
        Ok(())
    }

    /// Synchronously prepares a snapped, clamped frame. Errors preserve the previous position.
    pub fn seek(&mut self, position: Duration) -> Result<()> {
        let position = clamp_position(&self.timeline, position)?;
        if position == self.position() {
            return Ok(());
        }
        let frame = self.decoder.frame_at(&self.timeline, position)?;
        self.preview = Arc::new(TimelinePreviewFrame::new(Arc::new(frame)));
        Ok(())
    }

    pub fn seek_frame(&mut self, frame: TimelineFrame) -> Result<()> {
        self.seek(self.timeline.position_at_frame(frame))
    }

    pub fn preview_frame(&self) -> Arc<TimelinePreviewFrame> {
        Arc::clone(&self.preview)
    }

    pub fn frame_size(&self) -> (u32, u32) {
        (self.timeline.settings.width, self.timeline.settings.height)
    }

    pub fn framerate(&self) -> Option<f64> {
        Some(self.timeline.settings.frame_rate.frames_per_second())
    }

    pub fn duration(&self) -> Duration {
        self.timeline
            .position_at_frame(self.timeline.content_duration())
    }

    pub fn position(&self) -> Duration {
        self.preview.frame.timestamp
    }

    /// Older snapshots remain valid after seeking or dropping this backend.
    pub fn get_current_frame(&self) -> Result<Arc<DecodedTimelineFrame>> {
        Ok(Arc::clone(&self.preview.frame))
    }
}

fn clamp_position(timeline: &TimelineEditingState, position: Duration) -> Result<Duration> {
    let rate = timeline.settings.frame_rate;
    if rate.numerator == 0 || rate.denominator == 0 {
        bail!("Timeline frame rate must have a positive numerator and denominator");
    }
    let last = (timeline.content_duration() - TimelineFrame::ONE_FRAME).max(TimelineFrame::ZERO);
    let position = rate
        .frames_from_duration_nearest(position)
        .clamp(TimelineFrame::ZERO, last);
    Ok(rate.duration(position))
}

#[cfg(test)]
#[path = "tests/timeline_backend.test.rs"]
mod tests;
