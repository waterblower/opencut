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
        self.active_timeline_tool = tool;
        self.blade_guide_position = None;
        self.trim_drag = None;
        self.clip_move_drag = None;
        self.marquee_selection = None;
        self.snap_guide = None;
    }

    pub(super) fn update_blade_guide(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.active_timeline_tool != TimelineTool::Blade {
            return;
        }
        let position = self
            .timeline_position_from_x(event.position.x.into())
            .clamp(TimelineTime::ZERO, self.project.timeline_duration());
        if self.blade_guide_position != Some(position) {
            self.blade_guide_position = Some(position);
            cx.notify();
        }
    }

    pub(super) fn update_blade_guide_hover(
        &mut self,
        hovered: &bool,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !*hovered && self.blade_guide_position.take().is_some() {
            cx.notify();
        }
    }

    pub(super) fn begin_clip_interaction(
        &mut self,
        clip_id: u64,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        match self.active_timeline_tool {
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
        if self.active_timeline_tool != TimelineTool::Selection {
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
            self.selected_clip_ids.clone()
        } else {
            HashSet::new()
        };
        self.marquee_selection = Some(MarqueeSelection {
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
        if self.marquee_selection.is_none() || !event.dragging() {
            return;
        }
        let (x, y) = Self::timeline_pointer_position(
            event.position.x.into(),
            event.position.y.into(),
            window,
        );
        if let Some(selection) = self.marquee_selection.as_mut() {
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
        if self.marquee_selection.is_none() {
            return;
        }
        let (x, y) = Self::timeline_pointer_position(
            event.position.x.into(),
            event.position.y.into(),
            window,
        );
        if let Some(selection) = self.marquee_selection.as_mut() {
            selection.current_x = x;
            selection.current_y = y;
        }
        self.select_clips_in_marquee();
        self.marquee_selection = None;
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
        let Some(selection) = self.marquee_selection.as_ref() else {
            return;
        };
        let left = selection.start_x.min(selection.current_x);
        let right = selection.start_x.max(selection.current_x);
        let top = selection.start_y.min(selection.current_y);
        let bottom = selection.start_y.max(selection.current_y);
        let scroll_x = f32::from(self.timeline_scroll.offset().x);
        let scroll_y = f32::from(self.timeline_vertical_scroll.offset().y);

        let mut selected = selection.initial_selection.clone();
        for (track_index, track) in self.project.tracks.iter().enumerate() {
            let clip_top = TIMELINE_HEADER_HEIGHT
                + RULER_HEIGHT
                + track_index as f32 * TRACK_HEIGHT
                + scroll_y
                + 5.0;
            let clip_bottom = clip_top + TRACK_HEIGHT - 10.0;
            for clip in self.project.clips_on_track(track.id) {
                let clip_left = TRACK_HEADER_WIDTH
                    + scroll_x
                    + TIMELINE_PADDING
                    + self.project.seconds(clip.timeline_start) as f32 * self.pixels_per_second;
                let clip_right = clip_left
                    + (self.project.seconds(clip.duration()) as f32 * self.pixels_per_second)
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
        self.selected_clip_id = self
            .project
            .clips
            .iter()
            .find(|clip| selected.contains(&clip.id))
            .map(|clip| clip.id);
        self.selected_clip_ids = selected;
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
        if !self.selected_clip_ids.contains(&clip_id) {
            self.select_only_clip(Some(clip_id));
        }
        if !self.selected_clips_editable() {
            return;
        }
        let Some(anchor) = self.project.clip(clip_id).cloned() else {
            return;
        };
        let Some(original_anchor_track_index) = self
            .project
            .tracks
            .iter()
            .position(|track| track.id == anchor.track_id)
        else {
            return;
        };
        let items = self
            .selected_clip_ids_in_project_order()
            .into_iter()
            .filter_map(|selected_id| {
                let clip = self.project.clip(selected_id)?;
                let original_track_index = self
                    .project
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
        if items.len() != self.selected_clip_ids.len() {
            return;
        }
        if let Some(video) = &self.preview.video {
            video.set_paused(true);
        }
        self.preview.playing = false;
        self.preview.timeline_clock = None;
        self.snap_guide = None;
        self.clip_move_drag = Some(ClipMoveDrag {
            anchor_clip_id: clip_id,
            start_x: event.position.x.into(),
            original_anchor_start: anchor.timeline_start,
            original_anchor_track_index,
            placements: items
                .iter()
                .filter_map(|item| {
                    self.project.clip(item.clip_id)?;
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
        let Some(drag) = self.clip_move_drag.as_ref() else {
            return;
        };
        let anchor_clip_id = drag.anchor_clip_id;
        let start_x = drag.start_x;
        let original_anchor_start = drag.original_anchor_start;
        let original_anchor_track_index = drag.original_anchor_track_index;
        let items = drag.items.clone();
        let raw_delta =
            self.project.settings.frame_rate.delta(
                (f32::from(event.position.x) - start_x) as f64 / self.pixels_per_second as f64,
            );
        let earliest_start = items
            .iter()
            .map(|item| item.original_timeline_start)
            .min()
            .unwrap_or(TimelineTime::ZERO);
        let raw_anchor_start = original_anchor_start
            + TimelineTime::from_frames(raw_delta.frames().max(-earliest_start.frames()));
        let anchor_duration = self
            .project
            .clip(anchor_clip_id)
            .map(TimelineClip::duration)
            .unwrap_or(TimelineTime::ZERO);
        let (snapped_start, snap_guide) = self.snap_clip_start_ignoring(
            raw_anchor_start,
            anchor_duration,
            &self.selected_clip_ids,
        );
        let timeline_delta = snapped_start - original_anchor_start;
        let viewport_height = f32::from(window.viewport_size().height);
        let track_top = viewport_height - TIMELINE_HEIGHT + TIMELINE_HEADER_HEIGHT + RULER_HEIGHT;
        let scroll_y: f32 = self.timeline_vertical_scroll.offset().y.into();
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
        let maximum_track_index = self.project.tracks.len().saturating_sub(1);
        let track_delta = requested_track_delta.clamp(
            -(first_track_index as isize),
            maximum_track_index.saturating_sub(last_track_index) as isize,
        );
        let placements_for_delta = |track_delta: isize| {
            items
                .iter()
                .filter_map(|item| {
                    let target_index = item.original_track_index.checked_add_signed(track_delta)?;
                    let track_id = self.project.tracks.get(target_index)?.id;
                    self.project.clip(item.clip_id)?;
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
            self.validate_clip_move_placements(&placements, &self.selected_clip_ids)
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
        if let Some(drag) = &mut self.clip_move_drag {
            drag.placements = placements;
            drag.invalid_reason = invalid_reason;
            drag.changed = moved_from_origin;
        }
        self.snap_guide = snap_guide;
        cx.notify();
    }

    pub(super) fn finish_clip_move(
        &mut self,
        _: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.clip_move_drag.take() else {
            return;
        };
        self.snap_guide = None;
        if drag.changed && drag.invalid_reason.is_none() {
            self.checkpoint();
            for (clip_id, track_id, start) in drag.placements {
                if let Some(clip) = self.project.clip_mut(clip_id) {
                    clip.timeline_start = start;
                    clip.track_id = track_id;
                }
            }
            self.save_project();
            self.load_timeline_position(self.preview.playhead, false);
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
        if !self.snapping_enabled {
            return (time.max(TimelineTime::ZERO), None);
        }
        let threshold = self
            .project
            .settings
            .frame_rate
            .ceil(SNAP_DISTANCE_PX as f64 / self.pixels_per_second as f64)
            .frames()
            .max(1) as u64;
        let mut candidates = vec![TimelineTime::ZERO, self.preview.playhead];
        for clip in &self.project.clips {
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
        self.project
            .clip(clip_id)
            .and_then(|clip| self.project.track(clip.track_id))
            .is_some_and(|track| track.locked)
    }

    pub(super) fn begin_trim(&mut self, clip_id: u64, edge: TrimEdge, x: f32) {
        if self.selected_clip_ids.len() > 1 {
            return;
        }
        let Some(clip) = self.project.clip(clip_id).cloned() else {
            return;
        };
        if self.clip_locked(clip_id) {
            return;
        }
        let maximum_source_out = self.project.asset(clip.asset_id).and_then(|asset| {
            (asset.kind != MediaKind::Image).then(|| self.project.ceil_time(asset.duration))
        });
        if let Some(video) = &self.preview.video {
            video.set_paused(true);
        }
        self.preview.playing = false;
        self.preview.timeline_clock = None;
        self.select_only_clip(Some(clip_id));
        self.snap_guide = None;
        self.trim_drag = Some(TrimDrag {
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
        let Some(drag) = self.trim_drag.as_ref() else {
            return;
        };
        let clip_id = drag.clip_id;
        let edge = drag.edge;
        let original_in = drag.original_in;
        let original_out = drag.original_out;
        let original_timeline_start = drag.original_timeline_start;
        let maximum_source_out = drag.maximum_source_out;
        let Some((previous_end, next_start)) = self.project.trim_limits(clip_id) else {
            return;
        };
        let raw_delta = self.project.settings.frame_rate.delta(
            (f32::from(event.position.x) - drag.start_x) as f64 / self.pixels_per_second as f64,
        );
        if raw_delta == TimelineTime::ZERO {
            self.snap_guide = None;
            cx.notify();
            return;
        }
        if !drag.changed {
            self.checkpoint();
            if let Some(drag) = &mut self.trim_drag {
                drag.changed = true;
            }
        }
        let Some(index) = self.project.clip_index(clip_id) else {
            return;
        };
        self.snap_guide = match edge {
            TrimEdge::Left => {
                let raw_start = (original_timeline_start + raw_delta).max(TimelineTime::ZERO);
                let original_end = original_timeline_start + original_out - original_in;
                let earliest_start = previous_end.max(original_timeline_start - original_in);
                let latest_start = original_end - TimelineTime::ONE_FRAME;
                let (snapped_start, guide) = self.snap_time(raw_start, Some(clip_id));
                let start = snapped_start.clamp(earliest_start, latest_start);
                self.project.clips[index].source_in = original_in + start - original_timeline_start;
                self.project.clips[index].timeline_start = start;
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
                self.project.clips[index].source_out = original_in + end - original_timeline_start;
                (end == snapped_end).then_some(guide).flatten()
            }
        };
        cx.notify();
    }

    pub(super) fn finish_trim(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let changed = self.trim_drag.take().is_some_and(|drag| drag.changed);
        self.snap_guide = None;
        if changed {
            self.save_project();
            if let Some(clip_id) = self.selected_clip_id
                && let Some(index) = self.project.clip_index(clip_id)
            {
                self.load_timeline_position(self.project.clips[index].timeline_start, false);
            }
        }
        cx.notify();
    }

    pub(super) fn zoom(&mut self, factor: f32) {
        let previous_pixels_per_second = self.pixels_per_second;
        let pixels_per_second = (self.pixels_per_second * factor).clamp(
            MIN_TIMELINE_PIXELS_PER_SECOND,
            MAX_TIMELINE_PIXELS_PER_SECOND,
        );
        if pixels_per_second != previous_pixels_per_second {
            let mut scroll_offset = self.timeline_scroll.offset();
            let playhead_seconds = self.project.seconds(self.preview.playhead);
            scroll_offset.x = px(zoom_scroll_offset(
                f32::from(scroll_offset.x),
                playhead_seconds,
                previous_pixels_per_second,
                pixels_per_second,
            ));
            self.timeline_scroll.set_offset(scroll_offset);
            self.pixels_per_second = pixels_per_second;
            self.timeline_zoom_save_due = Some(Instant::now() + TIMELINE_ZOOM_SAVE_DELAY);
        }
    }

    pub(super) fn toggle_snapping(&mut self) {
        self.snapping_enabled = !self.snapping_enabled;
        self.snap_guide = None;
    }

    pub(super) fn log_timeline_trackpad_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        _: &mut Window,
        _: &mut Context<Self>,
    ) {
        if !event.delta.precise() {
            return;
        }

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

        let previous_zoom = self.pixels_per_second;
        let factor = (gesture.magnification as f32).exp().clamp(0.5, 2.0);
        self.zoom(factor);
        log::debug!(
            target: "opencut::timeline",
            "trackpad-pinch magnification={:.4} location_y={:.1} action=zoom factor={factor:.4} px_per_second={previous_zoom:.2}->{:.2}",
            gesture.magnification,
            gesture.location_y,
            self.pixels_per_second,
        );
        self.pixels_per_second != previous_zoom
    }

    pub(super) fn timeline_position_from_x(&self, x: f32) -> TimelineTime {
        let scroll_x: f32 = self.timeline_scroll.offset().x.into();
        let content_x = x - TRACK_HEADER_WIDTH - scroll_x - TIMELINE_PADDING;
        self.project
            .nearest_time(content_x as f64 / self.pixels_per_second as f64)
            .clamp(TimelineTime::ZERO, self.project.timeline_duration())
    }

    pub(super) fn begin_playhead_scrub(&mut self, event: &MouseDownEvent) {
        self.is_scrubbing_playhead = true;
        if let Some(video) = &self.preview.video {
            video.set_paused(true);
        }
        self.preview.playing = false;
        self.preview.timeline_clock = None;
        let position = self.timeline_position_from_x(event.position.x.into());
        self.last_playhead_scrub_seek = Some(Instant::now());
        self.load_timeline_position_for_scrub(position, false, false);
    }

    pub(super) fn update_playhead_scrub(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_scrubbing_playhead && event.dragging() {
            let position = self.timeline_position_from_x(event.position.x.into());
            self.preview.playhead = position;

            let now = Instant::now();
            let should_seek = self
                .last_playhead_scrub_seek
                .is_none_or(|last_seek| now.duration_since(last_seek) >= SCRUB_SEEK_INTERVAL);
            if should_seek {
                self.last_playhead_scrub_seek = Some(now);
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
        if self.is_scrubbing_playhead {
            self.is_scrubbing_playhead = false;
            self.last_playhead_scrub_seek = None;
            let position = self.timeline_position_from_x(event.position.x.into());
            self.load_timeline_position_for_scrub(position, true, true);
            cx.notify();
        }
    }

    pub(super) fn step_playhead(&mut self, frames: i64) {
        if self.project.clips.is_empty() {
            return;
        }
        let target = (self.preview.playhead + TimelineTime::from_frames(frames))
            .clamp(TimelineTime::ZERO, self.project.timeline_duration());
        if target != self.preview.playhead || self.preview.target != PreviewTarget::Timeline {
            self.load_timeline_position(target, false);
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
