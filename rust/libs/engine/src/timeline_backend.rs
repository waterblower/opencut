//! Timeline preview and playback state without a GPUI context. Seeks and edits prepare frames
//! synchronously; snapshot reads never decode. Owners that play drive [`TimelineBackend::advance`]
//! from a task, redraw when it reports a change, and release retired images.

use crate::timeline_decoder::{TimelineDecoder, TimelineFrameComposition};
use anyhow::{Context as _, Result, bail};
use gpui::{App, RenderImage};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use timeline::{FrameRate, TimelineEditingState, TimelineFrameIndex};

pub const MAX_CONTROL_WAIT: Duration = Duration::from_millis(100); // 暂停时轮询间隔；限制控制响应延迟。

#[rustfmt::skip]
pub struct TimelineBackend {
    timeline: TimelineEditingState,
    project_root: PathBuf,
    decoder: TimelineDecoder,
    displayed: Arc<TimelineFrameComposition>,    // 当前展示的合成帧；图像在准备时转换，渲染时不解码。
    clock: PlaybackClock,
    playing: bool,
    retired: Vec<Arc<RenderImage>>,   // 不再展示的图像；GPUI 纹理需由持有者释放。
}

/// Result of one playback step.
pub struct Advance {
    pub wait: Duration, // 下一次调用前的等待时间。
    pub changed: bool,  // 画面或播放状态已变化，持有者应重绘。
}

#[rustfmt::skip]
#[derive(Clone, Copy)]
struct PlaybackClock {
    start_position: Duration,          // 共同起点；不逐轮累加播放位置。
    start_time: Option<Instant>,       // None 表示冻结。
}

impl PlaybackClock {
    fn position(&self) -> Duration {
        match self.start_time {
            Some(start) => self.start_position + start.elapsed(),
            None => self.start_position,
        }
    }
}

impl TimelineBackend {
    /// Validates the document and prepares frame zero, paused. Media paths resolve against `project_root`.
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
        let frame = decoder.frame_at(&timeline, TimelineFrameIndex::ZERO)?;
        Ok(Self {
            timeline,
            project_root: project_root.to_owned(),
            decoder,
            displayed: Arc::new(frame),
            clock: PlaybackClock {
                start_position: Duration::ZERO,
                start_time: None,
            },
            playing: false,
            retired: Vec::new(),
        })
    }

    pub fn timeline(&self) -> &TimelineEditingState {
        &self.timeline
    }

    /// Prepares edited content at the current frame before publishing it.
    /// Errors preserve the document and frame.
    pub fn replace_timeline(&mut self, timeline: TimelineEditingState) -> Result<()> {
        timeline.validate()?;
        // Asset IDs may now point at different media, so no cached decoder is reused.
        let mut decoder = TimelineDecoder::new(&self.project_root);
        let frame = decoder.frame_at(&timeline, self.frame_index())?;
        self.timeline = timeline;
        self.decoder = decoder;
        self.show(frame);
        if !self.playing {
            self.clock.start_position = self.clock.start_position.min(self.duration());
        }
        Ok(())
    }

    /// Synchronously prepares the nearest frame, clamped to the last one.
    /// Errors preserve the previous frame and position.
    pub fn seek(&mut self, position: Duration) -> Result<()> {
        let rate = self.timeline.settings.frame_rate;
        if rate.numerator == 0 || rate.denominator == 0 {
            bail!("Timeline frame rate must have a positive numerator and denominator");
        }
        let frame = rate.frames_from_duration_nearest(position);
        self.seek_frame(frame)
    }

    /// Shows `frame`, clamped to the last one, and restarts the playback clock at its start.
    /// Errors preserve the previous frame and position.
    pub fn seek_frame(&mut self, frame: TimelineFrameIndex) -> Result<()> {
        let frame = frame.clamp(TimelineFrameIndex::ZERO, self.last_frame());
        if frame != self.frame_index() {
            let picture = self.decoder.frame_at(&self.timeline, frame)?;
            self.show(picture);
        }
        self.clock = PlaybackClock {
            start_position: self.timeline.position_at_frame(frame),
            start_time: self.clock.start_time.map(|_| Instant::now()),
        };
        Ok(())
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn is_ended(&self) -> bool {
        !self.playing && self.clock.start_position >= self.duration()
    }

    /// Restarts from zero after the end.
    pub fn play(&mut self) -> Result<()> {
        if self.playing {
            return Ok(());
        }
        if self.is_ended() {
            self.seek_frame(TimelineFrameIndex::ZERO)?;
        }
        self.clock.start_time = Some(Instant::now());
        self.playing = true;
        Ok(())
    }

    pub fn pause(&mut self) {
        if !self.playing {
            return;
        }
        self.clock = PlaybackClock {
            start_position: self.clock_position(),
            start_time: None,
        };
        self.playing = false;
    }

    pub fn toggle_playback(&mut self) -> Result<()> {
        if self.playing {
            self.pause();
            Ok(())
        } else {
            self.play()
        }
    }

    /// Shows the frame under the clock, skipping frames when preparation falls behind.
    pub fn advance(&mut self) -> Result<Advance> {
        if !self.playing {
            return Ok(Advance {
                wait: MAX_CONTROL_WAIT,
                changed: false,
            });
        }
        let rate = self.timeline.settings.frame_rate;
        let frame = floor_frame(rate, self.clock.position());
        if frame >= self.timeline.content_duration() {
            self.playing = false;
            self.clock = PlaybackClock {
                start_position: self.duration(),
                start_time: None,
            };
            return Ok(Advance {
                wait: MAX_CONTROL_WAIT,
                changed: true,
            });
        }
        let changed = frame != self.frame_index();
        if changed {
            let frame = self.decoder.frame_at(&self.timeline, frame)?;
            self.show(frame);
        }
        let next = rate.duration(frame + TimelineFrameIndex::ONE_FRAME);
        Ok(Advance {
            wait: next
                .saturating_sub(self.clock.position())
                .min(MAX_CONTROL_WAIT),
            changed,
        })
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

    /// The index of the displayed timeline frame.
    pub fn frame_index(&self) -> TimelineFrameIndex {
        self.displayed.frame_index
    }

    /// Start time of the displayed frame.
    pub fn position(&self) -> Duration {
        self.displayed.timestamp
    }

    /// Exact playback time, e.g. a seek target inside the displayed frame.
    pub fn clock_position(&self) -> Duration {
        self.clock.position().min(self.duration())
    }

    /// The displayed frame. Older snapshots remain valid after seeking or dropping this backend.
    pub fn preview_frame(&self) -> Arc<TimelineFrameComposition> {
        Arc::clone(&self.displayed)
    }

    pub fn get_current_frame(&self) -> Result<Arc<TimelineFrameComposition>> {
        Ok(self.preview_frame())
    }

    fn last_frame(&self) -> TimelineFrameIndex {
        (self.timeline.content_duration() - TimelineFrameIndex::ONE_FRAME)
            .max(TimelineFrameIndex::ZERO)
    }

    /// Replaces the displayed frame and retires images the new frame no longer uses.
    fn show(&mut self, frame: TimelineFrameComposition) {
        let previous = std::mem::replace(&mut self.displayed, Arc::new(frame));
        for image in previous.images() {
            if !self.displayed.images().any(|kept| Arc::ptr_eq(kept, image))
                && !self
                    .retired
                    .iter()
                    .any(|retired| Arc::ptr_eq(retired, image))
            {
                self.retired.push(Arc::clone(image));
            }
        }
    }
}

/// The timeline frame containing the position; frames change at their start, not their midpoint.
/// One extra nanosecond absorbs `FrameRate::duration` rounding a frame start down.
fn floor_frame(rate: FrameRate, position: Duration) -> TimelineFrameIndex {
    let frames = position
        .as_nanos()
        .saturating_add(1)
        .saturating_mul(rate.numerator.max(1) as u128)
        / (rate.denominator.max(1) as u128 * 1_000_000_000);
    (frames.min(i64::MAX as u128) as i64).into()
}
