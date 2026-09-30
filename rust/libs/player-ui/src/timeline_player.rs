//! Plays the visual layers of a timeline. Audio is not played yet.

use crate::{
    timeline_decoder::{TimelineDecoder, PreparedFrame},
    video_player::PlaybackState,
};
use anyhow::Result;
use gpui::{Context, Task};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
use timeline::{FrameRate, TimelineEditingState, TimelineFrame as TimelineFrameIndex};

const MAX_CONTROL_WAIT: Duration = Duration::from_millis(100); // 暂停时轮询间隔；限制控制响应延迟。

#[rustfmt::skip]
pub struct TimelinePlayer {
    timeline: TimelineEditingState,
    project_root: PathBuf,
    decoder: TimelineDecoder,
    pub(crate) displayed: Arc<PreparedFrame>, // 当前展示的合成帧；图像在准备时转换，渲染时不解码。
    clock: PlaybackClock,
    pub playback_state: PlaybackState,
    pub title: String,
    pub error: Option<String>,                // 最近一次播放失败；播放已暂停，保留上一帧。
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

impl TimelinePlayer {
    /// Validates the timeline and prepares frame zero, paused.
    /// Media paths resolve against `project_root`. Call [`Self::start`] once the player is in an entity.
    pub fn new(timeline: TimelineEditingState, project_root: &Path) -> Result<Self> {
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
            playback_state: PlaybackState::Paused,
            title: String::new(),
            error: None,
        })
    }

    /// Starts the playback loop once. The owner must retain the task and drop it before the player.
    pub fn start(&mut self, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |player, cx| {
            loop {
                let Ok(wait) = player.update(cx, |player, cx| {
                    player.advance(cx).unwrap_or_else(|error| {
                        player.fail(error, cx);
                        MAX_CONTROL_WAIT
                    })
                }) else {
                    return; // 播放器已释放。
                };
                cx.background_executor().timer(wait).await;
            }
        })
    }

    pub fn timeline(&self) -> &TimelineEditingState {
        &self.timeline
    }

    pub fn duration(&self) -> Duration {
        self.timeline
            .position_at_frame(self.timeline.content_duration())
    }

    /// The displayed timeline frame.
    pub fn frame(&self) -> TimelineFrameIndex {
        self.displayed.frame
    }

    /// Start time of the displayed frame.
    pub fn position(&self) -> Duration {
        self.displayed.timestamp
    }

    pub fn is_ended(&self) -> bool {
        matches!(self.playback_state, PlaybackState::Paused)
            && self.clock.start_position >= self.duration()
    }

    pub fn play(&mut self, cx: &mut Context<Self>) -> Result<()> {
        if matches!(self.playback_state, PlaybackState::Playing) {
            return Ok(());
        }
        if self.is_ended() {
            self.seek(Duration::ZERO, cx)?;
        }
        self.error = None;
        self.clock.start_time = Some(Instant::now());
        self.playback_state = PlaybackState::Playing;
        cx.notify();
        Ok(())
    }

    pub fn pause(&mut self, cx: &mut Context<Self>) {
        if matches!(self.playback_state, PlaybackState::Paused) {
            return;
        }
        self.clock = PlaybackClock {
            start_position: self.clock.position().min(self.duration()),
            start_time: None,
        };
        self.playback_state = PlaybackState::Paused;
        cx.notify();
    }

    pub fn toggle_playback(&mut self, cx: &mut Context<Self>) -> Result<()> {
        match self.playback_state {
            PlaybackState::Playing => {
                self.pause(cx);
                Ok(())
            }
            PlaybackState::Paused => self.play(cx),
        }
    }

    /// Shows the frame containing `position`. Errors keep the previous frame and position.
    pub fn seek(&mut self, position: Duration, cx: &mut Context<Self>) -> Result<()> {
        let position = position.min(self.duration());
        let rate = self.timeline.settings.frame_rate;
        let frame = self
            .decoder
            .frame_at(&self.timeline, floor_frame(rate, position))?;
        self.show(frame, cx);
        self.clock = PlaybackClock {
            start_position: position,
            start_time: self.clock.start_time.map(|_| Instant::now()),
        };
        Ok(())
    }

    pub fn seek_frame(&mut self, frame: TimelineFrameIndex, cx: &mut Context<Self>) -> Result<()> {
        self.seek(self.timeline.position_at_frame(frame), cx)
    }

    /// Swaps in edited content at the current frame. Errors keep the previous timeline and frame.
    pub fn replace_timeline(
        &mut self,
        timeline: TimelineEditingState,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        timeline.validate()?;
        // Asset IDs may now point at different media, so no cached decoder is reused.
        let mut decoder = TimelineDecoder::new(&self.project_root);
        let frame = decoder.frame_at(&timeline, self.frame())?;
        self.timeline = timeline;
        self.decoder = decoder;
        self.show(frame, cx);
        Ok(())
    }

    /// Shows the frame under the clock, skipping frames when preparation falls behind.
    fn advance(&mut self, cx: &mut Context<Self>) -> Result<Duration> {
        if matches!(self.playback_state, PlaybackState::Paused) {
            return Ok(MAX_CONTROL_WAIT);
        }
        let rate = self.timeline.settings.frame_rate;
        let frame = floor_frame(rate, self.clock.position());
        if frame >= self.timeline.content_duration() {
            self.playback_state = PlaybackState::Paused;
            self.clock = PlaybackClock {
                start_position: self.duration(),
                start_time: None,
            };
            cx.notify();
            return Ok(MAX_CONTROL_WAIT);
        }
        if frame != self.displayed.frame {
            let frame = self.decoder.frame_at(&self.timeline, frame)?;
            self.show(frame, cx);
        }
        let next = rate.duration(frame + TimelineFrameIndex::ONE_FRAME);
        Ok(next
            .saturating_sub(self.clock.position())
            .min(MAX_CONTROL_WAIT))
    }

    /// Pauses on the last good frame and records the error for the owner to display.
    pub(crate) fn fail(&mut self, error: anyhow::Error, cx: &mut Context<Self>) {
        eprintln!("Timeline player failed: {error:?}");
        self.pause(cx);
        self.error = Some(format!("{error:#}"));
        cx.notify();
    }

    /// Replaces the displayed frame and frees textures the new frame no longer uses.
    fn show(&mut self, frame: PreparedFrame, cx: &mut Context<Self>) {
        let previous = std::mem::replace(&mut self.displayed, Arc::new(frame));
        let current = Arc::clone(&self.displayed);
        // GPUI keeps every RenderImage in its atlas until dropped. Defer so the window
        // currently being updated (click handlers) is back in the app's window list.
        cx.defer(move |cx| {
            for image in previous.images() {
                if !current.images().any(|kept| Arc::ptr_eq(kept, image)) {
                    cx.drop_image(Arc::clone(image), None);
                }
            }
        });
        cx.notify();
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
    TimelineFrameIndex::from_frames(frames.min(i64::MAX as u128) as i64)
}
