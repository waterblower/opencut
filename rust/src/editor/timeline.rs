use super::model::TimelineSettings;
use super::*;
use gpui::point;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub(super) struct Timeline {
    pub settings: TimelineSettings,
    pub assets: Vec<MediaAsset>,
    pub tracks: Vec<TimelineTrack>,
    pub clips: Vec<TimelineClip>,
    #[serde(default)]
    pub view: TimelineViewState,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub(super) struct TimelineViewState {
    pub(super) saved_playhead_frame: i64,
    pub(super) horizontal_scroll: f32,
    pub(super) vertical_scroll: f32,
    #[serde(default = "default_enabled")]
    pub(super) snapping_enabled: bool,
    #[serde(default = "default_enabled")]
    pub(super) track_magnet_enabled: bool,
}

pub(super) struct TimelineInteractionState {
    pub(super) active_tool: TimelineTool,
    pub(super) snapping_enabled: bool,
    pub(super) magnet_enabled: bool,
    pub(super) selected_clip_id: Option<u64>,
    pub(super) selected_clip_ids: HashSet<u64>,
    pub(super) blade_guide: Option<TimelineTime>,
    pub(super) snap_guide: Option<TimelineTime>,
    pub(super) trim_drag: Option<TrimDrag>,
    pub(super) clip_move_drag: Option<ClipMoveDrag>,
    pub(super) marquee_selection: Option<MarqueeSelection>,
    pub(super) scrubbing_playhead: bool,
    pub(super) last_scrub_seek: Option<Instant>,
    pub(super) context_menu: Option<TimelineClipContextMenu>,
}

pub(super) struct TimelineState {
    pub(super) path: PathBuf,
    pub(super) data: Timeline,
    pub(super) playhead: TimelineTime,
    pub(super) pixels_per_second: f32,
    pub(super) zoom_save_due: Option<Instant>,
    pub(super) interaction: TimelineInteractionState,
    pub(super) scroll: ScrollHandle,
    pub(super) vertical_scroll: ScrollHandle,
    pub(super) clipboard: Option<ClipClipboard>,
    pub(super) next_id: u64,
    pub(super) undo_stack: Vec<Timeline>,
    pub(super) redo_stack: Vec<Timeline>,
}

impl Default for TimelineViewState {
    fn default() -> Self {
        Self {
            saved_playhead_frame: 0,
            horizontal_scroll: 0.0,
            vertical_scroll: 0.0,
            snapping_enabled: true,
            track_magnet_enabled: true,
        }
    }
}

impl TimelineViewState {
    fn normalize(&mut self) {
        self.saved_playhead_frame = self.saved_playhead_frame.max(0);
        self.horizontal_scroll = finite_nonnegative(self.horizontal_scroll);
        self.vertical_scroll = finite_nonnegative(self.vertical_scroll);
    }
}

impl Timeline {
    pub fn load(path: &Path) -> Result<Self, String> {
        let contents = fs::read_to_string(path)
            .map_err(|error| format!("could not read {}: {error}", path.display()))?;
        let mut timeline = serde_json::from_str::<Self>(&contents)
            .map_err(|error| format!("could not parse {}: {error}", path.display()))?;
        timeline.normalize();
        Ok(timeline)
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        let directory = path
            .parent()
            .ok_or_else(|| "timeline path has no parent directory".to_string())?;
        fs::create_dir_all(directory)
            .map_err(|error| format!("could not create {}: {error}", directory.display()))?;
        let json = serde_json::to_string_pretty(self)
            .map_err(|error| format!("could not serialize timeline: {error}"))?;
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, format!("{json}\n"))
            .map_err(|error| format!("could not write {}: {error}", temporary.display()))?;
        fs::rename(&temporary, path)
            .map_err(|error| format!("could not replace {}: {error}", path.display()))
    }

    pub fn asset(&self, id: u64) -> Option<&MediaAsset> {
        self.assets.iter().find(|asset| asset.id == id)
    }

    pub fn clip(&self, id: u64) -> Option<&TimelineClip> {
        self.clips.iter().find(|clip| clip.id == id)
    }

    pub fn clip_mut(&mut self, id: u64) -> Option<&mut TimelineClip> {
        self.clips.iter_mut().find(|clip| clip.id == id)
    }

    pub fn clip_index(&self, id: u64) -> Option<usize> {
        self.clips.iter().position(|clip| clip.id == id)
    }

    pub fn track(&self, id: u64) -> Option<&TimelineTrack> {
        self.tracks.iter().find(|track| track.id == id)
    }

    pub fn track_mut(&mut self, id: u64) -> Option<&mut TimelineTrack> {
        self.tracks.iter_mut().find(|track| track.id == id)
    }

    pub fn clips_on_track(&self, track_id: u64) -> impl Iterator<Item = &TimelineClip> {
        self.clips
            .iter()
            .filter(move |clip| clip.track_id == track_id)
    }

    pub fn trim_limits(&self, clip_id: u64) -> Option<(TimelineTime, TimelineTime)> {
        let clip = self.clip(clip_id)?;
        let previous_end = self
            .clips_on_track(clip.track_id)
            .filter(|other| other.id != clip_id && other.timeline_start < clip.timeline_start)
            .map(TimelineClip::timeline_end)
            .max()
            .unwrap_or(TimelineTime::ZERO);
        let next_start = self
            .clips_on_track(clip.track_id)
            .filter(|other| other.id != clip_id && other.timeline_start >= clip.timeline_end())
            .map(|other| other.timeline_start)
            .min()
            .unwrap_or(TimelineTime::MAX);
        Some((previous_end, next_start))
    }

    pub fn content_duration(&self) -> TimelineTime {
        self.clips
            .iter()
            .map(TimelineClip::timeline_end)
            .max()
            .unwrap_or(TimelineTime::ZERO)
    }

    pub fn timeline_duration(&self) -> TimelineTime {
        self.content_duration()
    }

    pub fn seconds(&self, time: TimelineTime) -> f64 {
        self.settings.frame_rate.seconds(time)
    }

    pub fn duration(&self, time: TimelineTime) -> Duration {
        self.settings.frame_rate.duration(time)
    }

    pub fn nearest_time(&self, seconds: f64) -> TimelineTime {
        self.settings.frame_rate.nearest(seconds)
    }

    pub fn floor_duration(&self, duration: Duration) -> TimelineTime {
        self.settings.frame_rate.floor_duration(duration)
    }

    pub fn audio_duration(&self, time: TimelineTime) -> Duration {
        let samples = self
            .settings
            .frame_rate
            .audio_samples(time, self.settings.audio_sample_rate);
        Duration::from_secs_f64(samples as f64 / self.settings.audio_sample_rate as f64)
    }

    /// Maps a timeline position within a clip to the source frame covering that instant.
    pub fn source_frame_at(
        &self,
        clip: &TimelineClip,
        timeline_position: TimelineTime,
    ) -> Option<i64> {
        let asset = self.asset(clip.asset_id)?;
        let source_rate = asset.frame_rate()?;
        let source_time = clip.source_time_at(timeline_position);
        Some(
            self.settings
                .frame_rate
                .rescale_floor(source_time, source_rate)
                .frames(),
        )
    }

    /// Returns an exact source-frame timestamp for video and a timeline-clock timestamp otherwise.
    pub fn source_position_at(
        &self,
        clip: &TimelineClip,
        timeline_position: TimelineTime,
    ) -> Duration {
        let Some(asset) = self.asset(clip.asset_id) else {
            return Duration::ZERO;
        };
        if let (Some(source_rate), Some(source_frame)) = (
            asset.frame_rate(),
            self.source_frame_at(clip, timeline_position),
        ) {
            return source_rate.duration(TimelineTime::from_frames(source_frame));
        }
        self.audio_duration(clip.source_time_at(timeline_position))
    }

    pub fn source_start_seconds(&self, clip: &TimelineClip) -> f64 {
        self.source_position_at(clip, clip.timeline_start)
            .as_secs_f64()
    }

    pub fn set_frame_rate(&mut self, frame_rate: FrameRate) {
        let frame_rate = FrameRate::new(frame_rate.numerator.max(1), frame_rate.denominator.max(1));
        let previous = self.settings.frame_rate;
        if previous == frame_rate {
            return;
        }

        for clip in &mut self.clips {
            let old_start = clip.timeline_start;
            let old_end = clip.timeline_end();
            let old_source_in = clip.source_in;
            clip.timeline_start = previous.rescale_nearest(old_start, frame_rate);
            let new_end = previous.rescale_nearest(old_end, frame_rate);
            let new_duration = (new_end - clip.timeline_start).max(TimelineTime::ONE_FRAME);
            clip.source_in = previous.rescale_nearest(old_source_in, frame_rate);
            clip.source_out = clip.source_in + new_duration;
        }
        self.settings.frame_rate = frame_rate;
        self.normalize();
    }

    pub fn ceil_time(&self, seconds: f64) -> TimelineTime {
        self.settings.frame_rate.ceil(seconds)
    }

    pub fn next_id(&self) -> u64 {
        self.assets
            .iter()
            .map(|asset| asset.id)
            .chain(self.tracks.iter().map(|track| track.id))
            .chain(self.clips.iter().map(|clip| clip.id))
            .max()
            .unwrap_or(0)
            + 1
    }

    fn normalize(&mut self) {
        self.view.normalize();
        if self.settings.frame_rate.numerator == 0 {
            self.settings.frame_rate.numerator = 30;
        }
        if self.settings.frame_rate.denominator == 0 {
            self.settings.frame_rate.denominator = 1;
        }
        self.settings.width = self.settings.width.max(2);
        self.settings.height = self.settings.height.max(2);
        self.settings.audio_sample_rate = self.settings.audio_sample_rate.max(8_000);
        self.clips.retain(|clip| {
            self.tracks.iter().any(|track| track.id == clip.track_id)
                && self.assets.iter().any(|asset| asset.id == clip.asset_id)
                && clip.timeline_start >= TimelineTime::ZERO
                && clip.source_in >= TimelineTime::ZERO
                && clip.source_out - clip.source_in >= TimelineTime::ONE_FRAME
        });
        let frame_rate = self.settings.frame_rate;
        for clip in &mut self.clips {
            if let Some(asset) = self.assets.iter().find(|asset| asset.id == clip.asset_id) {
                if asset.kind == MediaKind::Image {
                    // An image has no time-based source to exhaust. Its five-second
                    // asset duration is only the initial clip length, not a maximum.
                    clip.source_in = clip.source_in.max(TimelineTime::ZERO);
                    clip.source_out = clip
                        .source_out
                        .max(clip.source_in + TimelineTime::ONE_FRAME);
                } else {
                    let asset_duration = frame_rate.ceil(asset.duration);
                    let maximum_in =
                        (asset_duration - TimelineTime::ONE_FRAME).max(TimelineTime::ZERO);
                    clip.source_in = clip.source_in.clamp(TimelineTime::ZERO, maximum_in);
                    clip.source_out = clip
                        .source_out
                        .clamp(clip.source_in + TimelineTime::ONE_FRAME, asset_duration);
                }
            }
        }
        for track in &self.tracks {
            let mut indices = self
                .clips
                .iter()
                .enumerate()
                .filter(|(_, clip)| clip.track_id == track.id)
                .map(|(index, _)| index)
                .collect::<Vec<_>>();
            indices.sort_by(|left, right| {
                self.clips[*left]
                    .timeline_start
                    .cmp(&self.clips[*right].timeline_start)
                    .then_with(|| self.clips[*left].id.cmp(&self.clips[*right].id))
            });
            let mut next_available = TimelineTime::ZERO;
            for index in indices {
                self.clips[index].timeline_start =
                    self.clips[index].timeline_start.max(next_available);
                next_available = self.clips[index].timeline_end();
            }
        }
    }
}

impl TimelineState {
    pub(super) fn new(path: PathBuf, data: Timeline, pixels_per_second: f32) -> Self {
        let playhead = TimelineTime::from_frames(data.view.saved_playhead_frame)
            .clamp(TimelineTime::ZERO, data.timeline_duration());
        let scroll = ScrollHandle::new();
        scroll.set_offset(point(px(-data.view.horizontal_scroll), px(0.0)));
        let vertical_scroll = ScrollHandle::new();
        vertical_scroll.set_offset(point(px(0.0), px(-data.view.vertical_scroll)));
        let snapping_enabled = data.view.snapping_enabled;
        let magnet_enabled = data.view.track_magnet_enabled;
        let selected_clip_id = data.clips.first().map(|clip| clip.id);
        let selected_clip_ids = selected_clip_id.into_iter().collect();
        let next_id = data.next_id();

        Self {
            path,
            data,
            playhead,
            pixels_per_second,
            zoom_save_due: None,
            interaction: TimelineInteractionState {
                active_tool: TimelineTool::Selection,
                snapping_enabled,
                magnet_enabled,
                selected_clip_id,
                selected_clip_ids,
                blade_guide: None,
                snap_guide: None,
                trim_drag: None,
                clip_move_drag: None,
                marquee_selection: None,
                scrubbing_playhead: false,
                last_scrub_seek: None,
                context_menu: None,
            },
            scroll,
            vertical_scroll,
            clipboard: None,
            next_id,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub(super) fn capture_view(&mut self) {
        self.data.view = TimelineViewState {
            saved_playhead_frame: self.playhead.frames().max(0),
            horizontal_scroll: finite_nonnegative(-f32::from(self.scroll.offset().x)),
            vertical_scroll: finite_nonnegative(-f32::from(self.vertical_scroll.offset().y)),
            snapping_enabled: self.interaction.snapping_enabled,
            track_magnet_enabled: self.interaction.magnet_enabled,
        };
    }
}

fn default_enabled() -> bool {
    true
}

fn finite_nonnegative(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

const MAX_RULER_TICKS: usize = 240;
const MIN_RULER_LABEL_SPACING: f32 = 72.0;
const MIN_FRAME_TICK_SPACING: f32 = 4.0;
const FRAME_TICK_OVERSCAN: f32 = 120.0;
const TICK_STEPS: [f64; 12] = [
    1.0, 2.0, 5.0, 10.0, 15.0, 20.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0,
];

impl Editor {
    pub(super) fn timeline(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(timeline) = self.timeline.as_ref() else {
            return div()
                .id("editor-timeline-empty")
                .h(px(TIMELINE_HEIGHT))
                .flex_shrink_0()
                .flex()
                .items_center()
                .justify_center()
                .border_t_1()
                .border_color(rgb(BORDER))
                .bg(rgb(0x0a0a0c))
                .text_color(rgb(MUTED))
                .child("Create or select a timeline to begin editing")
                .into_any_element();
        };
        let frames_per_second = timeline.data.settings.frame_rate.frames_per_second();

        div()
            .id("editor-timeline")
            .relative()
            .h(px(TIMELINE_HEIGHT))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(rgb(BORDER))
            .bg(rgb(0x0a0a0c))
            .on_mouse_move(cx.listener(Self::update_trim))
            .on_mouse_move(cx.listener(Self::update_clip_move))
            .on_mouse_move(cx.listener(Self::update_playhead_scrub))
            .on_mouse_move(cx.listener(Self::update_marquee_selection))
            .on_scroll_wheel(cx.listener(Self::finish_timeline_scroll))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_trim))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_clip_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_playhead_scrub))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(Self::finish_marquee_selection),
            )
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_trim))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_clip_move))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_playhead_scrub))
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(Self::finish_marquee_selection),
            )
            .child(self.timeline_toolbar(frames_per_second, cx))
            .child(self.timeline_tracks_container(cx))
            .when_some(self.timeline_marquee(), |this, marquee| this.child(marquee))
            .into_any_element()
    }

    fn timeline_marquee(&self) -> Option<gpui::AnyElement> {
        let selection = self
            .timeline
            .as_ref()?
            .interaction
            .marquee_selection
            .as_ref()?;
        let left = selection.start_x.min(selection.current_x);
        let top = selection.start_y.min(selection.current_y);
        let width = (selection.start_x - selection.current_x).abs();
        let height = (selection.start_y - selection.current_y).abs();

        Some(
            div()
                .absolute()
                .left(px(left))
                .top(px(top))
                .w(px(width))
                .h(px(height))
                .border_1()
                .border_color(rgb(ACCENT))
                .bg(gpui::rgba(0xf0b75e24))
                .into_any_element(),
        )
    }

    fn timeline_tracks_container(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let timeline = self
            .timeline
            .as_ref()
            .expect("timeline view requires timeline state");
        let duration = timeline
            .data
            .seconds(timeline.data.timeline_duration())
            .max(12.0);
        let timeline_width =
            (duration as f32 * timeline.pixels_per_second + TIMELINE_PADDING * 2.0).max(900.0);
        let track_headers = timeline
            .data
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| self.track_header(index, track, cx))
            .collect::<Vec<_>>();
        let track_rows = timeline
            .data
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| self.track_row(index, track, timeline_width, cx))
            .collect::<Vec<_>>();
        div()
            .id("timeline-tracks-vertical-scroll")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .track_scroll(&timeline.vertical_scroll)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, event: &MouseDownEvent, window, cx| {
                    editor.begin_marquee_selection(event, window, cx);
                }),
            )
            .child(
                div()
                    .h(px(
                        RULER_HEIGHT + timeline.data.tracks.len() as f32 * TRACK_HEIGHT
                    ))
                    .min_h_full()
                    .w_full()
                    .flex()
                    .child(
                        div()
                            .w(px(TRACK_HEADER_WIDTH))
                            .h_full()
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .border_r_1()
                            .border_color(rgb(BORDER))
                            .child(
                                div()
                                    .h(px(RULER_HEIGHT))
                                    .flex_shrink_0()
                                    .border_b_1()
                                    .border_color(rgb(BORDER)),
                            )
                            .children(track_headers),
                    )
                    .child(
                        div()
                            .id("editor-timeline-scroll")
                            .min_w_0()
                            .flex_1()
                            .h_full()
                            .overflow_x_scroll()
                            .track_scroll(&timeline.scroll)
                            .cursor(match timeline.interaction.active_tool {
                                TimelineTool::Blade => CursorStyle::Crosshair,
                                TimelineTool::Selection | TimelineTool::Trim => CursorStyle::Arrow,
                            })
                            .on_mouse_move(cx.listener(Self::update_blade_guide))
                            .on_hover(cx.listener(Self::update_blade_guide_hover))
                            .child(
                                div()
                                    .relative()
                                    .w(px(timeline_width))
                                    .min_h_full()
                                    .on_drag_move::<ExplorerMediaDrag>(
                                        cx.listener(Self::update_explorer_media_drag),
                                    )
                                    .on_drop(cx.listener(
                                        |editor, drag: &ExplorerMediaDrag, _, cx| {
                                            editor.drop_explorer_media(drag, cx);
                                        },
                                    ))
                                    .child(self.timeline_ruler(duration, cx))
                                    .children(track_rows)
                                    .child(self.timeline_playhead(cx))
                                    .when_some(timeline.interaction.snap_guide, |this, guide| {
                                        let guide_left = TIMELINE_PADDING
                                            + timeline.data.seconds(guide) as f32
                                                * timeline.pixels_per_second;
                                        this.child(
                                            div()
                                                .absolute()
                                                .top_0()
                                                .bottom_0()
                                                .left(px(guide_left))
                                                .w(px(2.0))
                                                .bg(rgb(0x63c8ff))
                                                .child(
                                                    div()
                                                        .absolute()
                                                        .top_0()
                                                        .left(px(-3.0))
                                                        .size_2()
                                                        .rounded_full()
                                                        .bg(rgb(0x63c8ff)),
                                                ),
                                        )
                                    })
                                    .when_some(
                                        timeline.interaction.blade_guide,
                                        |this, position| {
                                            let guide_left = TIMELINE_PADDING
                                                + timeline.data.seconds(position) as f32
                                                    * timeline.pixels_per_second;
                                            this.child(
                                                div()
                                                    .absolute()
                                                    .top_0()
                                                    .bottom_0()
                                                    .left(px(guide_left))
                                                    .w(px(2.0))
                                                    .bg(rgb(ERROR))
                                                    .cursor(CursorStyle::Crosshair)
                                                    .child(
                                                        div()
                                                            .absolute()
                                                            .top_0()
                                                            .left(px(-4.0))
                                                            .size_2()
                                                            .bg(rgb(ERROR)),
                                                    ),
                                            )
                                        },
                                    ),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn timeline_playhead(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let timeline = self
            .timeline
            .as_ref()
            .expect("timeline view requires timeline state");
        let left = TIMELINE_PADDING
            + timeline.data.seconds(timeline.playhead) as f32 * timeline.pixels_per_second;

        div()
            .absolute()
            .top_0()
            .bottom_0()
            .left(px(left))
            .w(px(1.0))
            .bg(rgb(ACCENT))
            .cursor(if timeline.interaction.active_tool == TimelineTool::Blade {
                CursorStyle::Crosshair
            } else {
                CursorStyle::ResizeLeftRight
            })
            .when(
                timeline.interaction.active_tool != TimelineTool::Blade,
                |this| {
                    this.on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|editor, event: &MouseDownEvent, _, cx| {
                            editor.begin_playhead_scrub(event);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                },
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left(px(-4.0))
                    .size_2()
                    .bg(rgb(ACCENT)),
            )
            .into_any_element()
    }

    fn timeline_ruler(&self, duration: f64, cx: &mut Context<Self>) -> gpui::AnyElement {
        let timeline = self
            .timeline
            .as_ref()
            .expect("timeline view requires timeline state");
        let frame_rate = timeline.data.settings.frame_rate;
        let frames_per_second = frame_rate.frames_per_second();
        let displayed_frames = frame_rate.ceil(duration).frames().max(1);
        let pixels_per_frame = timeline.pixels_per_second / frames_per_second as f32;
        let frame_step = frame_tick_step(pixels_per_frame);
        let scroll_left = (-f32::from(timeline.scroll.offset().x)).max(0.0);
        let viewport_width = {
            let width = f32::from(timeline.scroll.bounds().size.width);
            if width > 0.0 { width } else { 1_200.0 }
        };
        let visible_start = ((scroll_left - FRAME_TICK_OVERSCAN - TIMELINE_PADDING).max(0.0)
            / pixels_per_frame.max(f32::EPSILON))
        .floor() as i64;
        let visible_end = ((scroll_left + viewport_width + FRAME_TICK_OVERSCAN - TIMELINE_PADDING)
            .max(0.0)
            / pixels_per_frame.max(f32::EPSILON))
        .ceil() as i64;
        let first_frame = visible_start
            .div_euclid(frame_step)
            .saturating_mul(frame_step)
            .max(frame_step);
        let last_frame = visible_end.min(displayed_frames);
        let nominal_fps = frames_per_second.round().max(1.0) as i64;
        let frame_ticks = (first_frame..=last_frame)
            .step_by(frame_step as usize)
            .map(|frame| {
                let emphasized = frame % nominal_fps == 0;
                let medium = !emphasized && frame % 5 == 0;
                let height = if emphasized {
                    12.0
                } else if medium {
                    8.0
                } else {
                    5.0
                };
                div()
                    .absolute()
                    .left(px(TIMELINE_PADDING
                        + frame_rate.seconds(TimelineTime::from_frames(frame)) as f32
                            * timeline.pixels_per_second))
                    .bottom_0()
                    .h(px(height))
                    .border_l_1()
                    .border_color(rgb(if emphasized { 0x5a5a62 } else { 0x3a3a40 }))
            });
        let tick_step = ruler_tick_step(duration, timeline.pixels_per_second);
        let tick_count = (duration / tick_step).ceil() as usize + 1;
        let ruler_ticks = (0..tick_count).map(|index| {
            let time = index as f64 * tick_step;
            div()
                .absolute()
                .left(px(
                    TIMELINE_PADDING + time as f32 * timeline.pixels_per_second
                ))
                .top_0()
                .h_full()
                .border_l_1()
                .border_color(rgb(0x333338))
                .pl_1()
                .font_family("monospace")
                .text_xs()
                .text_color(rgb(MUTED))
                .child(format_time_precise(time))
        });
        div()
            .id("timeline-ruler")
            .relative()
            .w_full()
            .h(px(RULER_HEIGHT))
            .border_b_1()
            .border_color(rgb(BORDER))
            .cursor(CursorStyle::PointingHand)
            .children(frame_ticks)
            .children(ruler_ticks)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, event: &MouseDownEvent, _, cx| {
                    editor.begin_playhead_scrub(event);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .into_any_element()
    }

    fn timeline_toolbar(&self, frames_per_second: f64, cx: &mut Context<Self>) -> gpui::AnyElement {
        let timeline = self
            .timeline
            .as_ref()
            .expect("timeline view requires timeline state");
        div()
            .id("timeline-toolbar")
            .h(px(TIMELINE_HEADER_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px_3()
            .border_b_1()
            .border_color(rgb(BORDER))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        timeline_tool_button(
                            "timeline-selection-tool",
                            "V Select",
                            timeline.interaction.active_tool == TimelineTool::Selection,
                        )
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.activate_timeline_tool(TimelineTool::Selection);
                            cx.notify();
                        })),
                    )
                    .child(
                        timeline_tool_button(
                            "timeline-blade-tool",
                            "B Blade",
                            timeline.interaction.active_tool == TimelineTool::Blade,
                        )
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.activate_timeline_tool(TimelineTool::Blade);
                            cx.notify();
                        })),
                    )
                    .child(
                        timeline_tool_button(
                            "timeline-trim-tool",
                            "T Trim",
                            timeline.interaction.active_tool == TimelineTool::Trim,
                        )
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.activate_timeline_tool(TimelineTool::Trim);
                            cx.notify();
                        })),
                    )
                    .child(
                        timeline_icon_button(
                            "timeline-play",
                            if self.preview.playing { "Ⅱ" } else { "▶" },
                        )
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.toggle_playback();
                            cx.notify();
                        })),
                    )
                    .child(
                        div()
                            .w(px(108.0))
                            .font_family("monospace")
                            .text_sm()
                            .child(format!(
                                "{} / {}",
                                format_time(timeline.data.seconds(timeline.playhead), false),
                                format_time(
                                    timeline.data.seconds(timeline.data.timeline_duration()),
                                    false
                                )
                            )),
                    )
                    .child(
                        timeline_icon_button("add-video-track", "+V").on_click(cx.listener(
                            |editor, _, _, cx| {
                                editor.add_track(TrackKind::Video);
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        timeline_icon_button("add-audio-track", "+A").on_click(cx.listener(
                            |editor, _, _, cx| {
                                editor.add_track(TrackKind::Audio);
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        timeline_icon_button(
                            "toggle-timeline-snapping",
                            if timeline.interaction.snapping_enabled {
                                "Snap on"
                            } else {
                                "Snap off"
                            },
                        )
                        .border_1()
                        .border_color(rgb(if timeline.interaction.snapping_enabled {
                            ACCENT
                        } else {
                            BORDER
                        }))
                        .text_color(rgb(if timeline.interaction.snapping_enabled {
                            ACCENT
                        } else {
                            MUTED
                        }))
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.toggle_snapping();
                            cx.notify();
                        })),
                    )
                    .child(
                        timeline_icon_button(
                            "toggle-track-magnet",
                            if timeline.interaction.magnet_enabled {
                                "Magnet on"
                            } else {
                                "Magnet off"
                            },
                        )
                        .border_1()
                        .border_color(rgb(if timeline.interaction.magnet_enabled {
                            ACCENT
                        } else {
                            BORDER
                        }))
                        .text_color(rgb(if timeline.interaction.magnet_enabled {
                            ACCENT
                        } else {
                            MUTED
                        }))
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.toggle_track_magnet();
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(timeline_icon_button("zoom-out", "−").on_click(cx.listener(
                        |editor, _, _, cx| {
                            editor.zoom(0.8);
                            cx.notify();
                        },
                    )))
                    .child(
                        div()
                            .w(px(58.0))
                            .text_center()
                            .font_family("monospace")
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(format!("{:.0}px/s", timeline.pixels_per_second)),
                    )
                    .child(
                        div()
                            .w(px(66.0))
                            .text_center()
                            .font_family("monospace")
                            .text_xs()
                            .text_color(rgb(MUTED))
                            .child(format!("{frames_per_second:.2} fps")),
                    )
                    .child(timeline_icon_button("zoom-in", "+").on_click(cx.listener(
                        |editor, _, _, cx| {
                            editor.zoom(1.25);
                            cx.notify();
                        },
                    ))),
            )
            .into_any_element()
    }
}

fn frame_tick_step(pixels_per_frame: f32) -> i64 {
    (MIN_FRAME_TICK_SPACING / pixels_per_frame.max(f32::EPSILON))
        .ceil()
        .max(1.0) as i64
}

fn ruler_tick_step(duration: f64, pixels_per_second: f32) -> f64 {
    let spacing_step = MIN_RULER_LABEL_SPACING as f64 / pixels_per_second.max(f32::EPSILON) as f64;
    let count_step = duration.max(0.0) / MAX_RULER_TICKS as f64;
    let minimum_step = spacing_step.max(count_step);

    TICK_STEPS
        .iter()
        .copied()
        .find(|step| *step >= minimum_step)
        .unwrap_or_else(|| {
            let largest_step = *TICK_STEPS
                .last()
                .expect("timeline tick steps are not empty");
            (minimum_step / largest_step).ceil() * largest_step
        })
}

fn timeline_icon_button(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h_7()
        .min_w(px(28.0))
        .px_2()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .bg(rgb(SURFACE))
        .cursor(CursorStyle::PointingHand)
        .text_xs()
        .hover(|style| style.bg(rgb(SURFACE_HOVER)))
        .child(label)
}

fn timeline_tool_button(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
    active: bool,
) -> gpui::Stateful<gpui::Div> {
    timeline_icon_button(id, label)
        .border_1()
        .border_color(rgb(if active { ACCENT } else { BORDER }))
        .text_color(rgb(if active { ACCENT } else { MUTED }))
}

fn format_time_precise(seconds: f64) -> String {
    let minutes = (seconds / 60.0).floor() as u64;
    let seconds = seconds % 60.0;
    format!("{minutes:02}:{seconds:04.1}")
}

#[cfg(test)]
#[path = "timeline.test.rs"]
mod tests;
