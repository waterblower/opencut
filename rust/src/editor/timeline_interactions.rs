use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TimelineTool {
    Selection,
    Blade,
    Trim,
}

#[derive(Clone, Copy)]
pub(super) enum TrimEdge {
    Left,
    Right,
}

pub(super) struct TrimDrag {
    pub(super) clip_id: u64,
    pub(super) edge: TrimEdge,
    pub(super) start_x: f32,
    pub(super) original_in: TimelineTime,
    pub(super) original_out: TimelineTime,
    pub(super) original_timeline_start: TimelineTime,
    /// The last valid source frame for time-based media. Still images have no
    /// intrinsic source duration, so their right edge is intentionally unbounded.
    pub(super) maximum_source_out: Option<TimelineTime>,
    pub(super) changed: bool,
}

#[derive(Clone)]
pub(super) struct ClipMoveItem {
    pub(super) clip_id: u64,
    pub(super) original_timeline_start: TimelineTime,
    pub(super) original_track_id: u64,
    pub(super) original_track_index: usize,
}

pub(super) struct ClipMoveDrag {
    pub(super) anchor_clip_id: u64,
    pub(super) start_x: f32,
    pub(super) original_anchor_start: TimelineTime,
    pub(super) original_anchor_track_index: usize,
    pub(super) items: Vec<ClipMoveItem>,
    pub(super) placements: Vec<(u64, u64, TimelineTime)>,
    pub(super) invalid_reason: Option<&'static str>,
    pub(super) changed: bool,
}

#[derive(Clone)]
pub(super) struct MarqueeSelection {
    pub(super) start_x: f32,
    pub(super) start_y: f32,
    pub(super) current_x: f32,
    pub(super) current_y: f32,
    pub(super) initial_selection: HashSet<u64>,
}

impl Editor {
    pub(super) fn activate_timeline_tool(&mut self, tool: TimelineTool) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.interaction.active_tool = tool;
        timeline.interaction.blade_guide = None;
        timeline.interaction.trim_drag = None;
        timeline.interaction.clip_move_drag = None;
        timeline.interaction.marquee_selection = None;
        timeline.interaction.snap_guide = None;
    }

    pub(super) fn update_blade_guide(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        if timeline.interaction.active_tool != TimelineTool::Blade {
            return;
        }
        let position = self
            .timeline_position_from_x(event.position.x.into())
            .clamp(TimelineTime::ZERO, timeline.data.timeline_duration());
        let timeline = self.timeline.as_mut().expect("timeline was checked above");
        if timeline.interaction.blade_guide != Some(position) {
            timeline.interaction.blade_guide = Some(position);
            cx.notify();
        }
    }

    pub(super) fn update_blade_guide_hover(
        &mut self,
        hovered: &bool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !*hovered
            && self
                .timeline
                .as_mut()
                .is_some_and(|timeline| timeline.interaction.blade_guide.take().is_some())
        {
            cx.notify();
        }
    }

    pub(super) fn begin_clip_interaction(
        &mut self,
        clip_id: u64,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        let Some(tool) = self
            .timeline
            .as_ref()
            .map(|timeline| timeline.interaction.active_tool)
        else {
            return;
        };
        match tool {
            TimelineTool::Selection => self.begin_clip_move(clip_id, event, cx),
            TimelineTool::Blade => {
                cx.stop_propagation();
                let position = self.timeline_position_from_x(event.position.x.into());
                self.blade_split_clip_at(clip_id, position);
            }
            TimelineTool::Trim => {
                cx.stop_propagation();
                self.select_only_clip(Some(clip_id));
            }
        }
    }

    pub(super) fn begin_marquee_selection(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        if timeline.interaction.active_tool != TimelineTool::Selection {
            return;
        }
        if f32::from(event.position.x) < TRACK_HEADER_WIDTH {
            return;
        }
        let (x, y) = Self::timeline_pointer_position(
            event.position.x.into(),
            event.position.y.into(),
            window,
        );
        let initial_selection = if event.modifiers.secondary() {
            timeline.interaction.selected_clip_ids.clone()
        } else {
            HashSet::new()
        };
        self.timeline
            .as_mut()
            .expect("timeline was checked above")
            .interaction
            .marquee_selection = Some(MarqueeSelection {
            start_x: x,
            start_y: y,
            current_x: x,
            current_y: y,
            initial_selection,
        });
        if !event.modifiers.secondary() {
            self.select_only_clip(None);
        }
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn update_marquee_selection(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .timeline
            .as_ref()
            .is_none_or(|timeline| timeline.interaction.marquee_selection.is_none())
            || !event.dragging()
        {
            return;
        }
        let (x, y) = Self::timeline_pointer_position(
            event.position.x.into(),
            event.position.y.into(),
            window,
        );
        if let Some(selection) = self
            .timeline
            .as_mut()
            .and_then(|timeline| timeline.interaction.marquee_selection.as_mut())
        {
            selection.current_x = x;
            selection.current_y = y;
        }
        self.select_clips_in_marquee();
        cx.notify();
    }

    pub(super) fn finish_marquee_selection(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .timeline
            .as_ref()
            .is_none_or(|timeline| timeline.interaction.marquee_selection.is_none())
        {
            return;
        }
        let (x, y) = Self::timeline_pointer_position(
            event.position.x.into(),
            event.position.y.into(),
            window,
        );
        if let Some(selection) = self
            .timeline
            .as_mut()
            .and_then(|timeline| timeline.interaction.marquee_selection.as_mut())
        {
            selection.current_x = x;
            selection.current_y = y;
        }
        self.select_clips_in_marquee();
        if let Some(timeline) = self.timeline.as_mut() {
            timeline.interaction.marquee_selection = None;
        }
        cx.notify();
    }

    pub(super) fn timeline_pointer_position(x: f32, y: f32, window: &Window) -> (f32, f32) {
        let viewport = window.viewport_size();
        let viewport_width = f32::from(viewport.width);
        let viewport_height = f32::from(viewport.height);
        let timeline_top = (viewport_height - TIMELINE_HEIGHT).max(0.0);
        (
            x.clamp(TRACK_HEADER_WIDTH, viewport_width),
            (y - timeline_top).clamp(TIMELINE_HEADER_HEIGHT + RULER_HEIGHT, TIMELINE_HEIGHT),
        )
    }

    pub(super) fn select_clips_in_marquee(&mut self) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let Some(selection) = timeline.interaction.marquee_selection.as_ref() else {
            return;
        };
        let left = selection.start_x.min(selection.current_x);
        let right = selection.start_x.max(selection.current_x);
        let top = selection.start_y.min(selection.current_y);
        let bottom = selection.start_y.max(selection.current_y);
        let scroll_x = f32::from(timeline.scroll.offset().x);
        let scroll_y = f32::from(timeline.vertical_scroll.offset().y);

        let mut selected = selection.initial_selection.clone();
        for (track_index, track) in timeline.data.tracks.iter().enumerate() {
            let clip_top = TIMELINE_HEADER_HEIGHT
                + RULER_HEIGHT
                + track_index as f32 * TRACK_HEIGHT
                + scroll_y
                + 5.0;
            let clip_bottom = clip_top + TRACK_HEIGHT - 10.0;
            for clip in timeline.data.clips_on_track(track.id) {
                let clip_left = TRACK_HEADER_WIDTH
                    + scroll_x
                    + TIMELINE_PADDING
                    + timeline.data.seconds(clip.timeline_start) as f32
                        * timeline.pixels_per_second;
                let clip_right = clip_left
                    + (timeline.data.seconds(clip.duration()) as f32 * timeline.pixels_per_second)
                        .max(4.0);
                if clip_left <= right
                    && clip_right >= left
                    && clip_top <= bottom
                    && clip_bottom >= top
                {
                    selected.insert(clip.id);
                }
            }
        }
        let timeline = self.timeline.as_mut().expect("timeline was checked above");
        timeline.interaction.selected_clip_id = timeline
            .data
            .clips
            .iter()
            .find(|clip| selected.contains(&clip.id))
            .map(|clip| clip.id);
        timeline.interaction.selected_clip_ids = selected;
    }

    pub(super) fn begin_clip_move(
        &mut self,
        clip_id: u64,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        cx.stop_propagation();
        if event.modifiers.secondary() {
            self.toggle_clip_selection(clip_id);
            return;
        }
        if self
            .timeline
            .as_ref()
            .is_none_or(|timeline| !timeline.interaction.selected_clip_ids.contains(&clip_id))
        {
            self.select_only_clip(Some(clip_id));
        }
        if !self.selected_clips_editable() {
            return;
        }
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let Some(anchor) = timeline.data.clip(clip_id).cloned() else {
            return;
        };
        let Some(original_anchor_track_index) = self
            .timeline
            .as_ref()
            .expect("timeline was checked above")
            .data
            .tracks
            .iter()
            .position(|track| track.id == anchor.track_id)
        else {
            return;
        };
        let items = self
            .selected_clip_ids_in_timeline_order()
            .into_iter()
            .filter_map(|selected_id| {
                let timeline = self.timeline.as_ref()?;
                let clip = timeline.data.clip(selected_id)?;
                let original_track_index = timeline
                    .data
                    .tracks
                    .iter()
                    .position(|track| track.id == clip.track_id)?;
                Some(ClipMoveItem {
                    clip_id: selected_id,
                    original_timeline_start: clip.timeline_start,
                    original_track_id: clip.track_id,
                    original_track_index,
                })
            })
            .collect::<Vec<_>>();
        if items.len()
            != self
                .timeline
                .as_ref()
                .expect("timeline was checked above")
                .interaction
                .selected_clip_ids
                .len()
        {
            return;
        }
        if let Some(video) = &self.preview.video {
            video.set_paused(true);
        }
        self.preview.playing = false;
        self.preview.timeline_clock = None;
        let timeline = self.timeline.as_mut().expect("timeline was checked above");
        timeline.interaction.snap_guide = None;
        timeline.interaction.clip_move_drag = Some(ClipMoveDrag {
            anchor_clip_id: clip_id,
            start_x: event.position.x.into(),
            original_anchor_start: anchor.timeline_start,
            original_anchor_track_index,
            placements: items
                .iter()
                .filter_map(|item| {
                    timeline.data.clip(item.clip_id)?;
                    Some((
                        item.clip_id,
                        item.original_track_id,
                        item.original_timeline_start,
                    ))
                })
                .collect(),
            items,
            invalid_reason: None,
            changed: false,
        });
    }

    pub(super) fn update_clip_move(
        &mut self,
        event: &MouseMoveEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !event.dragging() {
            return;
        }
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let Some(drag) = timeline.interaction.clip_move_drag.as_ref() else {
            return;
        };
        let anchor_clip_id = drag.anchor_clip_id;
        let start_x = drag.start_x;
        let original_anchor_start = drag.original_anchor_start;
        let original_anchor_track_index = drag.original_anchor_track_index;
        let items = drag.items.clone();
        let raw_delta = timeline.data.settings.frame_rate.delta(
            (f32::from(event.position.x) - start_x) as f64 / timeline.pixels_per_second as f64,
        );
        let earliest_start = items
            .iter()
            .map(|item| item.original_timeline_start)
            .min()
            .unwrap_or(TimelineTime::ZERO);
        let raw_anchor_start = original_anchor_start
            + TimelineTime::from_frames(raw_delta.frames().max(-earliest_start.frames()));
        let anchor_duration = timeline
            .data
            .clip(anchor_clip_id)
            .map(TimelineClip::duration)
            .unwrap_or(TimelineTime::ZERO);
        let (snapped_start, snap_guide) = self.snap_clip_start_ignoring(
            raw_anchor_start,
            anchor_duration,
            &timeline.interaction.selected_clip_ids,
        );
        let timeline_delta = snapped_start - original_anchor_start;
        let viewport_height = f32::from(window.viewport_size().height);
        let track_top = viewport_height - TIMELINE_HEIGHT + TIMELINE_HEADER_HEIGHT + RULER_HEIGHT;
        let scroll_y: f32 = timeline.vertical_scroll.offset().y.into();
        let track_index =
            ((f32::from(event.position.y) - track_top - scroll_y) / TRACK_HEIGHT).floor() as isize;
        let requested_track_delta = usize::try_from(track_index)
            .ok()
            .map(|target| target as isize - original_anchor_track_index as isize)
            .unwrap_or(0);
        let first_track_index = items
            .iter()
            .map(|item| item.original_track_index)
            .min()
            .unwrap_or(0);
        let last_track_index = items
            .iter()
            .map(|item| item.original_track_index)
            .max()
            .unwrap_or(0);
        let maximum_track_index = timeline.data.tracks.len().saturating_sub(1);
        let track_delta = requested_track_delta.clamp(
            -(first_track_index as isize),
            maximum_track_index.saturating_sub(last_track_index) as isize,
        );
        let placements_for_delta = |track_delta: isize| {
            items
                .iter()
                .filter_map(|item| {
                    let target_index = item.original_track_index.checked_add_signed(track_delta)?;
                    let track_id = timeline.data.tracks.get(target_index)?.id;
                    timeline.data.clip(item.clip_id)?;
                    Some((
                        item.clip_id,
                        track_id,
                        item.original_timeline_start + timeline_delta,
                    ))
                })
                .collect::<Vec<_>>()
        };
        let placements = placements_for_delta(track_delta);
        let invalid_reason = if placements.len() != items.len() {
            Some("Destination track is unavailable")
        } else {
            self.validate_clip_move_placements(&placements, &timeline.interaction.selected_clip_ids)
                .err()
                .map(ClipPlacementRejection::message)
        };
        let moved_from_origin = placements.iter().any(|(clip_id, track_id, start)| {
            items
                .iter()
                .find(|item| item.clip_id == *clip_id)
                .is_some_and(|item| {
                    *start != item.original_timeline_start || *track_id != item.original_track_id
                })
        });
        let timeline = self.timeline.as_mut().expect("timeline was checked above");
        if let Some(drag) = &mut timeline.interaction.clip_move_drag {
            drag.placements = placements;
            drag.invalid_reason = invalid_reason;
            drag.changed = moved_from_origin;
        }
        timeline.interaction.snap_guide = snap_guide;
        cx.notify();
    }

    pub(super) fn finish_clip_move(
        &mut self,
        _: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        let Some(drag) = timeline.interaction.clip_move_drag.take() else {
            return;
        };
        timeline.interaction.snap_guide = None;
        if drag.changed && drag.invalid_reason.is_none() {
            self.checkpoint();
            for (clip_id, track_id, start) in drag.placements {
                if let Some(clip) = self
                    .timeline
                    .as_mut()
                    .and_then(|timeline| timeline.data.clip_mut(clip_id))
                {
                    clip.timeline_start = start;
                    clip.track_id = track_id;
                }
            }
            self.save_timeline();
            let playhead = self
                .timeline
                .as_ref()
                .expect("timeline was checked above")
                .playhead;
            self.load_timeline_position(playhead, false);
        }
        cx.notify();
    }

    pub(super) fn snap_time(
        &self,
        time: TimelineTime,
        ignored_clip: Option<u64>,
    ) -> (TimelineTime, Option<TimelineTime>) {
        let ignored = ignored_clip.into_iter().collect::<HashSet<_>>();
        self.snap_time_ignoring(time, &ignored)
    }

    pub(super) fn snap_time_ignoring(
        &self,
        time: TimelineTime,
        ignored_clip_ids: &HashSet<u64>,
    ) -> (TimelineTime, Option<TimelineTime>) {
        let Some(timeline) = self.timeline.as_ref() else {
            return (time.max(TimelineTime::ZERO), None);
        };
        if !timeline.interaction.snapping_enabled {
            return (time.max(TimelineTime::ZERO), None);
        }
        let threshold = timeline
            .data
            .settings
            .frame_rate
            .ceil(SNAP_DISTANCE_PX as f64 / timeline.pixels_per_second as f64)
            .frames()
            .max(1) as u64;
        let mut candidates = vec![TimelineTime::ZERO, timeline.playhead];
        for clip in &timeline.data.clips {
            if !ignored_clip_ids.contains(&clip.id) {
                candidates.push(clip.timeline_start);
                candidates.push(clip.timeline_end());
            }
        }
        let snapped = candidates
            .into_iter()
            .filter(|candidate| candidate.abs_diff(time) <= threshold)
            .min_by_key(|candidate| candidate.abs_diff(time))
            .map(|candidate| (candidate.max(TimelineTime::ZERO), Some(candidate)));
        snapped.unwrap_or((time.max(TimelineTime::ZERO), None))
    }

    pub(super) fn snap_clip_start_ignoring(
        &self,
        start: TimelineTime,
        duration: TimelineTime,
        ignored_clip_ids: &HashSet<u64>,
    ) -> (TimelineTime, Option<TimelineTime>) {
        let (start_candidate, start_guide) = self.snap_time_ignoring(start, ignored_clip_ids);
        let (snapped_end, end_guide) = self.snap_time_ignoring(start + duration, ignored_clip_ids);
        let end_candidate = snapped_end - duration;
        choose_clip_snap(
            start,
            start_candidate,
            start_guide,
            end_candidate,
            end_guide,
        )
    }

    pub(super) fn clip_locked(&self, clip_id: u64) -> bool {
        self.timeline
            .as_ref()
            .and_then(|timeline| {
                timeline
                    .data
                    .clip(clip_id)
                    .and_then(|clip| timeline.data.track(clip.track_id))
            })
            .is_some_and(|track| track.locked)
    }

    pub(super) fn begin_trim(&mut self, clip_id: u64, edge: TrimEdge, x: f32) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        if timeline.interaction.selected_clip_ids.len() > 1 {
            return;
        }
        let Some(clip) = timeline.data.clip(clip_id).cloned() else {
            return;
        };
        if self.clip_locked(clip_id) {
            return;
        }
        let maximum_source_out = timeline.data.asset(clip.asset_id).and_then(|asset| {
            (asset.kind != MediaKind::Image).then(|| timeline.data.ceil_time(asset.duration))
        });
        if let Some(video) = &self.preview.video {
            video.set_paused(true);
        }
        self.preview.playing = false;
        self.preview.timeline_clock = None;
        self.select_only_clip(Some(clip_id));
        let timeline = self.timeline.as_mut().expect("timeline was checked above");
        timeline.interaction.snap_guide = None;
        timeline.interaction.trim_drag = Some(TrimDrag {
            clip_id,
            edge,
            start_x: x,
            original_in: clip.source_in,
            original_out: clip.source_out,
            original_timeline_start: clip.timeline_start,
            maximum_source_out,
            changed: false,
        });
    }

    pub(super) fn update_trim(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !event.dragging() {
            return;
        }
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let Some(drag) = timeline.interaction.trim_drag.as_ref() else {
            return;
        };
        let clip_id = drag.clip_id;
        let edge = drag.edge;
        let original_in = drag.original_in;
        let original_out = drag.original_out;
        let original_timeline_start = drag.original_timeline_start;
        let maximum_source_out = drag.maximum_source_out;
        let Some((previous_end, next_start)) = timeline.data.trim_limits(clip_id) else {
            return;
        };
        let raw_delta = timeline.data.settings.frame_rate.delta(
            (f32::from(event.position.x) - drag.start_x) as f64 / timeline.pixels_per_second as f64,
        );
        if raw_delta == TimelineTime::ZERO {
            self.timeline
                .as_mut()
                .expect("timeline was checked above")
                .interaction
                .snap_guide = None;
            cx.notify();
            return;
        }
        if !drag.changed {
            self.checkpoint();
            if let Some(drag) = self
                .timeline
                .as_mut()
                .and_then(|timeline| timeline.interaction.trim_drag.as_mut())
            {
                drag.changed = true;
            }
        }
        let Some(index) = self
            .timeline
            .as_ref()
            .and_then(|timeline| timeline.data.clip_index(clip_id))
        else {
            return;
        };
        let snap_guide = match edge {
            TrimEdge::Left => {
                let raw_start = (original_timeline_start + raw_delta).max(TimelineTime::ZERO);
                let original_end = original_timeline_start + original_out - original_in;
                let earliest_start = previous_end.max(original_timeline_start - original_in);
                let latest_start = original_end - TimelineTime::ONE_FRAME;
                let (snapped_start, guide) = self.snap_time(raw_start, Some(clip_id));
                let start = snapped_start.clamp(earliest_start, latest_start);
                let timeline = self.timeline.as_mut().expect("timeline was checked above");
                timeline.data.clips[index].source_in =
                    original_in + start - original_timeline_start;
                timeline.data.clips[index].timeline_start = start;
                (start == snapped_start).then_some(guide).flatten()
            }
            TrimEdge::Right => {
                let original_end = original_timeline_start + original_out - original_in;
                let earliest_end = original_timeline_start + TimelineTime::ONE_FRAME;
                let latest_source_end = maximum_source_out
                    .map(|source_out| {
                        original_timeline_start
                            + (source_out - original_in).max(TimelineTime::ONE_FRAME)
                    })
                    .unwrap_or(TimelineTime::MAX);
                let latest_end = next_start.min(latest_source_end);
                let (snapped_end, guide) = self.snap_time(original_end + raw_delta, Some(clip_id));
                let end = snapped_end.clamp(earliest_end, latest_end);
                self.timeline
                    .as_mut()
                    .expect("timeline was checked above")
                    .data
                    .clips[index]
                    .source_out = original_in + end - original_timeline_start;
                (end == snapped_end).then_some(guide).flatten()
            }
        };
        self.timeline
            .as_mut()
            .expect("timeline was checked above")
            .interaction
            .snap_guide = snap_guide;
        cx.notify();
    }

    pub(super) fn finish_trim(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let changed = self
            .timeline
            .as_mut()
            .and_then(|timeline| timeline.interaction.trim_drag.take())
            .is_some_and(|drag| drag.changed);
        if let Some(timeline) = self.timeline.as_mut() {
            timeline.interaction.snap_guide = None;
        }
        if changed {
            self.save_timeline();
            if let Some(position) = self.timeline.as_ref().and_then(|timeline| {
                let clip_id = timeline.interaction.selected_clip_id?;
                let index = timeline.data.clip_index(clip_id)?;
                Some(timeline.data.clips[index].timeline_start)
            }) {
                self.load_timeline_position(position, false);
            }
        }
        cx.notify();
    }

    pub(super) fn zoom(&mut self, factor: f32) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        let previous_pixels_per_second = timeline.pixels_per_second;
        let pixels_per_second = (timeline.pixels_per_second * factor).clamp(
            MIN_TIMELINE_PIXELS_PER_SECOND,
            MAX_TIMELINE_PIXELS_PER_SECOND,
        );
        if pixels_per_second != previous_pixels_per_second {
            let mut scroll_offset = timeline.scroll.offset();
            let playhead_seconds = timeline.data.seconds(timeline.playhead);
            scroll_offset.x = px(zoom_scroll_offset(
                f32::from(scroll_offset.x),
                playhead_seconds,
                previous_pixels_per_second,
                pixels_per_second,
            ));
            timeline.scroll.set_offset(scroll_offset);
            timeline.pixels_per_second = pixels_per_second;
            timeline.zoom_save_due = Some(Instant::now() + TIMELINE_ZOOM_SAVE_DELAY);
        }
    }

    pub(super) fn toggle_snapping(&mut self) {
        if let Some(timeline) = self.timeline.as_mut() {
            timeline.interaction.snapping_enabled = !timeline.interaction.snapping_enabled;
            timeline.interaction.snap_guide = None;
        }
        self.save_timeline_view();
    }

    pub(super) fn finish_timeline_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        if event.delta.precise() {
            let delta = event.delta.pixel_delta(px(16.0));
            let horizontal = f32::from(delta.x);
            let vertical = f32::from(delta.y);
            let action = if horizontal.abs() < f32::EPSILON && vertical.abs() < f32::EPSILON {
                "idle"
            } else {
                "pan"
            };
            log::debug!(
                target: "opencut::timeline",
                "trackpad-scroll phase={:?} delta=({horizontal:.2}, {vertical:.2}) action={action}",
                event.touch_phase,
            );
        }
        if !event.delta.precise() || matches!(event.touch_phase, TouchPhase::Ended) {
            self.save_timeline_view();
        }
    }

    pub(super) fn apply_timeline_pinch(&mut self) -> bool {
        let Some(gesture) = crate::macos_pinch::take() else {
            return false;
        };
        if !(0.0..=TIMELINE_HEIGHT as f64).contains(&gesture.location_y) {
            log::debug!(
                target: "opencut::timeline",
                "trackpad-pinch magnification={:.4} location_y={:.1} action=ignored",
                gesture.magnification,
                gesture.location_y,
            );
            return false;
        }

        let Some(previous_zoom) = self
            .timeline
            .as_ref()
            .map(|timeline| timeline.pixels_per_second)
        else {
            return false;
        };
        let factor = (gesture.magnification as f32).exp().clamp(0.5, 2.0);
        self.zoom(factor);
        log::debug!(
            target: "opencut::timeline",
            "trackpad-pinch magnification={:.4} location_y={:.1} action=zoom factor={factor:.4} px_per_second={previous_zoom:.2}->{:.2}",
            gesture.magnification,
            gesture.location_y,
            self.timeline
                .as_ref()
                .map_or(previous_zoom, |timeline| timeline.pixels_per_second),
        );
        self.timeline
            .as_ref()
            .is_some_and(|timeline| timeline.pixels_per_second != previous_zoom)
    }

    pub(super) fn timeline_position_from_x(&self, x: f32) -> TimelineTime {
        let Some(timeline) = self.timeline.as_ref() else {
            return TimelineTime::ZERO;
        };
        let scroll_x: f32 = timeline.scroll.offset().x.into();
        let content_x = x - TRACK_HEADER_WIDTH - scroll_x - TIMELINE_PADDING;
        timeline
            .data
            .nearest_time(content_x as f64 / timeline.pixels_per_second as f64)
            .clamp(TimelineTime::ZERO, timeline.data.timeline_duration())
    }

    pub(super) fn begin_playhead_scrub(&mut self, event: &MouseDownEvent) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.interaction.scrubbing_playhead = true;
        if let Some(video) = &self.preview.video {
            video.set_paused(true);
        }
        self.preview.playing = false;
        self.preview.timeline_clock = None;
        let position = self.timeline_position_from_x(event.position.x.into());
        self.timeline
            .as_mut()
            .expect("timeline was checked above")
            .interaction
            .last_scrub_seek = Some(Instant::now());
        self.load_timeline_position_for_scrub(position, false, false);
    }

    pub(super) fn update_playhead_scrub(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .timeline
            .as_ref()
            .is_some_and(|timeline| timeline.interaction.scrubbing_playhead)
            && event.dragging()
        {
            let position = self.timeline_position_from_x(event.position.x.into());
            self.timeline
                .as_mut()
                .expect("scrubbing requires an active timeline")
                .playhead = position;

            let now = Instant::now();
            let should_seek = self
                .timeline
                .as_ref()
                .expect("scrubbing requires an active timeline")
                .interaction
                .last_scrub_seek
                .is_none_or(|last_seek| now.duration_since(last_seek) >= SCRUB_SEEK_INTERVAL);
            if should_seek {
                self.timeline
                    .as_mut()
                    .expect("scrubbing requires an active timeline")
                    .interaction
                    .last_scrub_seek = Some(now);
                self.load_timeline_position_for_scrub(position, false, false);
            }
            cx.notify();
        }
    }

    pub(super) fn finish_playhead_scrub(
        &mut self,
        event: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self
            .timeline
            .as_ref()
            .is_some_and(|timeline| timeline.interaction.scrubbing_playhead)
        {
            let timeline = self
                .timeline
                .as_mut()
                .expect("scrubbing requires an active timeline");
            timeline.interaction.scrubbing_playhead = false;
            timeline.interaction.last_scrub_seek = None;
            let position = self.timeline_position_from_x(event.position.x.into());
            self.load_timeline_position_for_scrub(position, true, true);
            self.save_timeline_view();
            cx.notify();
        }
    }

    pub(super) fn step_playhead(&mut self, frames: i64) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        if timeline.data.clips.is_empty() {
            return;
        }
        let target = (timeline.playhead + TimelineTime::from_frames(frames))
            .clamp(TimelineTime::ZERO, timeline.data.timeline_duration());
        if target != timeline.playhead || self.preview.target != PreviewTarget::Timeline {
            self.load_timeline_position(target, false);
            self.save_timeline_view();
        }
    }
}

fn zoom_scroll_offset(
    previous_offset: f32,
    anchor_seconds: f64,
    previous_pixels_per_second: f32,
    pixels_per_second: f32,
) -> f32 {
    let anchor_seconds = anchor_seconds as f32;
    (previous_offset + anchor_seconds * (previous_pixels_per_second - pixels_per_second)).min(0.0)
}

fn choose_clip_snap(
    original_start: TimelineTime,
    start_candidate: TimelineTime,
    start_guide: Option<TimelineTime>,
    end_candidate: TimelineTime,
    end_guide: Option<TimelineTime>,
) -> (TimelineTime, Option<TimelineTime>) {
    match (start_guide, end_guide) {
        (None, None) => (original_start.max(TimelineTime::ZERO), None),
        (Some(guide), None) => (start_candidate.max(TimelineTime::ZERO), Some(guide)),
        (None, Some(guide)) => (end_candidate.max(TimelineTime::ZERO), Some(guide)),
        (Some(start_guide), Some(end_guide)) => {
            if end_candidate.abs_diff(original_start) < start_candidate.abs_diff(original_start) {
                (end_candidate.max(TimelineTime::ZERO), Some(end_guide))
            } else {
                (start_candidate.max(TimelineTime::ZERO), Some(start_guide))
            }
        }
    }
}

#[cfg(test)]
#[path = "timeline_interactions.test.rs"]
mod tests;
