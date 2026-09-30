//! Plays the visual layers of a timeline document. Audio is not played yet.

use crate::playback_decoder::{PlaybackDecoder, PreparedFrame, PreparedLayer};
use anyhow::{Context as _, Result};
use gpui::{
    AnyElement, AvailableSpace, Bounds, ClickEvent, Context, CursorStyle, Task, TextAlign, Window,
    canvas, div, img, prelude::*, px, relative, rgb, rgba,
};
use player_ui::video_player::PlaybackState;
use std::{
    cell::Cell,
    path::Path,
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
use timeline::{
    FrameRate, TimelineEditingState, TimelineFrame as TimelineFrameIndex, TimelineSerialization,
};

const MAX_CONTROL_WAIT: Duration = Duration::from_millis(100); // 暂停时轮询间隔；限制控制响应延迟。

#[rustfmt::skip]
pub struct TimelinePlayer {
    timeline: TimelineEditingState,
    decoder: PlaybackDecoder,
    displayed: Arc<PreparedFrame>,  // 当前展示的合成帧；图像在准备时转换，渲染时不解码。
    clock: PlaybackClock,
    pub playback_state: PlaybackState,
    pub title: String,
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
    /// Loads the document and prepares frame zero. Media paths resolve against the document's directory.
    pub fn new(path: &Path) -> Result<Self> {
        let document = TimelineSerialization::load(path)?;
        let timeline = document.to_editing_state();
        timeline.validate()?;
        let project_root = std::path::absolute(path)?
            .parent()
            .context("Timeline path has no parent directory")?
            .to_path_buf();
        let mut decoder = PlaybackDecoder::new(&project_root);
        let frame = decoder.frame_at(&timeline, TimelineFrameIndex::ZERO)?;
        Ok(Self {
            timeline,
            decoder,
            displayed: Arc::new(frame),
            clock: PlaybackClock {
                start_position: Duration::ZERO,
                start_time: None,
            },
            playback_state: PlaybackState::Playing,
            title: path.display().to_string(),
        })
    }

    /// Starts playback once. The owner must retain the task and drop it before the player.
    pub fn start(&mut self, cx: &mut Context<Self>) -> Task<()> {
        self.clock.start_time = Some(Instant::now());
        cx.spawn(async move |player, cx| {
            loop {
                let wait = match player.update(cx, |player, cx| player.advance(cx)) {
                    Ok(Ok(wait)) => wait,
                    Ok(Err(error)) => {
                        eprintln!("Timeline player failed: {error:?}");
                        std::process::exit(1);
                    }
                    Err(_) => return, // 播放器已释放。
                };
                cx.background_executor().timer(wait).await;
            }
        })
    }

    pub fn duration(&self) -> Duration {
        self.timeline
            .position_at_frame(self.timeline.content_duration())
    }

    pub fn is_ended(&self) -> bool {
        matches!(self.playback_state, PlaybackState::Paused)
            && self.clock.start_position >= self.duration()
    }

    #[rustfmt::skip]
    pub fn toggle_playback(&mut self, cx: &mut Context<Self>) -> Result<()> {
        match self.playback_state {
            PlaybackState::Playing => {
                self.clock = PlaybackClock {
                    start_position: self.clock.position().min(self.duration()),
                    start_time: None,
                };
                self.playback_state = PlaybackState::Paused;
            }
            PlaybackState::Paused => {
                if self.is_ended() {
                    self.seek(Duration::ZERO, cx)?;
                }
                self.clock.start_time = Some(Instant::now());
                self.playback_state = PlaybackState::Playing;
            }
        }
        cx.notify();
        Ok(())
    }

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
        if rate.duration(frame) != self.displayed.timestamp {
            let frame = self.decoder.frame_at(&self.timeline, frame)?;
            self.show(frame, cx);
            cx.notify();
        }
        let next = rate.duration(frame + TimelineFrameIndex::ONE_FRAME);
        Ok(next
            .saturating_sub(self.clock.position())
            .min(MAX_CONTROL_WAIT))
    }
}

impl TimelinePlayer {
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
    }
}

/// The timeline frame containing the position; frames change at their start, not their midpoint.
fn floor_frame(rate: FrameRate, position: Duration) -> TimelineFrameIndex {
    let frames = position
        .as_nanos()
        .saturating_mul(rate.numerator.max(1) as u128)
        / (rate.denominator.max(1) as u128 * 1_000_000_000);
    TimelineFrameIndex::from_frames(frames.min(i64::MAX as u128) as i64)
}

impl PreparedFrame {
    fn images(&self) -> impl Iterator<Item = &Arc<gpui::RenderImage>> {
        self.layers.iter().filter_map(|layer| match layer {
            PreparedLayer::Picture { image, .. } => Some(image),
            PreparedLayer::Text(_) => None,
        })
    }

    /// Same composition as the editor preview: fit the canvas, then apply each layer's transform.
    fn render(&self, width: f32, height: f32) -> AnyElement {
        let root = div()
            .w(px(width.max(0.0)))
            .h(px(height.max(0.0)))
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .bg(rgb(0));
        let scale = (width / self.width as f32).min(height / self.height as f32);
        if !scale.is_finite() || scale <= 0.0 {
            return root.into_any_element();
        }
        let canvas_width = self.width as f32 * scale;
        let canvas_height = self.height as f32 * scale;
        let mut canvas = div()
            .relative()
            .flex_shrink_0()
            .overflow_hidden()
            .w(px(canvas_width))
            .h(px(canvas_height));
        for layer in &self.layers {
            match layer {
                PreparedLayer::Picture { image, properties } => {
                    if properties.scale <= 0.0 {
                        continue;
                    }
                    let size = image.size(0);
                    let source_width = size.width.0 as f32;
                    let source_height = size.height.0 as f32;
                    let fit = (canvas_width / source_width).min(canvas_height / source_height);
                    let width = source_width * fit * properties.scale as f32;
                    let height = source_height * fit * properties.scale as f32;
                    let x = (canvas_width - width) / 2.0 + properties.position_x as f32 * scale;
                    let y = (canvas_height - height) / 2.0 + properties.position_y as f32 * scale;
                    canvas = canvas.child(
                        img(Arc::clone(image))
                            .absolute()
                            .left(px(x))
                            .top(px(y))
                            .w(px(width))
                            .h(px(height)),
                    );
                }
                PreparedLayer::Text(properties) => {
                    canvas = canvas.child(
                        div()
                            .absolute()
                            .left(px(properties.position_x as f32 * canvas_width))
                            .top(px(properties.position_y as f32 * canvas_height))
                            .w(px(0.0))
                            .h(px(0.0))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .flex_shrink_0()
                                    .whitespace_nowrap()
                                    .font_family(properties.font.clone())
                                    .text_size(px(properties.font_size as f32 * scale))
                                    .line_height(px(properties.font_size as f32 * scale * 1.2))
                                    .text_align(TextAlign::Center)
                                    .text_color(rgba(properties.color.rotate_left(8)))
                                    .child(properties.text.clone()),
                            ),
                    );
                }
            }
        }
        root.child(canvas).into_any_element()
    }
}

impl Render for TimelinePlayer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let seek_bounds = Rc::new(Cell::new(Bounds::default()));
        let duration = self.duration();
        let ended = self.is_ended();
        let position = if ended {
            duration
        } else {
            self.displayed.timestamp.min(duration)
        };
        let playback_label = match (ended, &self.playback_state) {
            (true, _) => "Ended",
            (_, PlaybackState::Playing) => "Pause",
            (_, PlaybackState::Paused) => "Play",
        };
        let progress = if duration.is_zero() {
            0.0
        } else {
            (position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
        };
        let frame = Arc::clone(&self.displayed);

        div()
            .on_children_prepainted({
                let seek_bounds = seek_bounds.clone();
                move |bounds, _, _| {
                    seek_bounds.set(bounds[1]); // 子元素依次为画面、进度条、控制栏；使用进度条的窗口坐标。
                }
            })
            .id("timeline-player")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x080808))
            .text_color(rgb(0xeeeeee))
            .child(
                div().w_full().flex_1().min_h_0().overflow_hidden().child(
                    canvas(
                        move |bounds, window, cx| {
                            let mut element =
                                frame.render(bounds.size.width.into(), bounds.size.height.into());
                            element.prepaint_as_root(
                                bounds.origin,
                                bounds.size.map(AvailableSpace::Definite),
                                window,
                                cx,
                            );
                            element
                        },
                        |_, mut element, window, cx| element.paint(window, cx),
                    )
                    .size_full(),
                ),
            )
            .child(
                div()
                    .id("seek")
                    .w_full()
                    .h(px(20.0))
                    .flex_shrink_0()
                    .bg(rgb(0x303030))
                    .cursor(CursorStyle::PointingHand)
                    .on_click(cx.listener({
                        let seek_bounds = seek_bounds.clone();
                        move |player, event: &ClickEvent, _, cx| {
                            let bounds = seek_bounds.get();
                            let width = f32::from(bounds.size.width).max(1.0);
                            let fraction = f32::from(event.position().x - bounds.left()) / width;
                            let position = player
                                .duration()
                                .mul_f64(f64::from(fraction.clamp(0.0, 1.0)));
                            if let Err(error) = player.seek(position, cx) {
                                eprintln!("Timeline player seek failed: {error:?}");
                                std::process::exit(1);
                            }
                            cx.notify();
                        }
                    }))
                    .child(div().h_full().w(relative(progress)).bg(rgb(0xdba34b))),
            )
            .child(
                div()
                    .h(px(80.0))
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .id("play-pause")
                            .cursor(CursorStyle::PointingHand)
                            .p_2()
                            .on_click(cx.listener(|player, _, _, cx| {
                                if let Err(error) = player.toggle_playback(cx) {
                                    eprintln!("Toggling timeline playback failed: {error:?}");
                                }
                            }))
                            .child(playback_label),
                    )
                    .child(format!(
                        "{} / {}",
                        format_time(position),
                        format_time(duration)
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_ellipsis()
                            .child(self.title.clone()),
                    ),
            )
    }
}

fn format_time(time: Duration) -> String {
    let seconds = time.as_secs();
    format!("{}:{:02}", seconds / 60, seconds % 60)
}
