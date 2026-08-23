use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TimelineTool {
    Selection,
    Blade,
}

pub(super) enum TimelineContextMenu {
    Clip(TimelineClipContextMenu),
    TextTrack(TextTrackContextMenu),
}

#[derive(Clone)]
pub(super) struct ClipMoveItem {
    pub(super) clip_id: Ulid,
    pub(super) original_timeline_start: TimelineTime,
    pub(super) original_track_id: Ulid,
    pub(super) original_track_index: usize,
}

pub(super) struct ClipMoveDrag {
    pub(super) anchor_clip_id: Ulid,
    pub(super) start_x: f32,
    pub(super) original_anchor_start: TimelineTime,
    pub(super) original_anchor_track_index: usize,
    pub(super) items: Vec<ClipMoveItem>,
    pub(super) placements: Vec<(Ulid, Ulid, TimelineTime)>,
    pub(super) invalid_reason: Option<&'static str>,
    pub(super) changed: bool,
}

#[derive(Clone)]
pub(super) struct MarqueeSelection {
    pub(super) start_x: f32,
    pub(super) start_y: f32,
    pub(super) current_x: f32,
    pub(super) current_y: f32,
    pub(super) initial_selection: HashSet<Ulid>,
}

pub(super) struct TimelineInteractionState {
    pub(super) active_tool: TimelineTool,
    pub(super) snapping_enabled: bool,
    pub(super) magnet_enabled: bool,
    pub(super) selected_clip_id: Option<Ulid>,
    pub(super) selected_clip_ids: HashSet<Ulid>,
    pub(super) blade_guide: Option<TimelineTime>,
    pub(super) snap_guide: Option<TimelineTime>,
    pub(super) clip_move_drag: Option<ClipMoveDrag>,
    pub(super) marquee_selection: Option<MarqueeSelection>,
    pub(super) scrubbing_playhead: bool,
    pub(super) last_scrub_seek: Option<Instant>,
    pub(super) context_menu: Option<TimelineContextMenu>,
}

impl TimelineRuntimeState {
    pub(super) fn selected_clips_editable(&self) -> bool {
        !self.interaction.selected_clip_ids.is_empty()
            && self.interaction.selected_clip_ids.iter().all(|clip_id| {
                self.data.clip(*clip_id).is_some() && !self.data.clip_locked(*clip_id)
            })
    }

    pub(super) fn selected_clip_ids_in_timeline_order(&self) -> Vec<Ulid> {
        self.data
            .clips
            .iter()
            .filter(|clip| self.interaction.selected_clip_ids.contains(&clip.id()))
            .map(Clip::id)
            .collect()
    }

    pub(super) fn activate_timeline_tool(&mut self, tool: TimelineTool) {
        self.interaction.active_tool = tool;
        self.interaction.blade_guide = None;
        self.interaction.clip_move_drag = None;
        self.interaction.marquee_selection = None;
        self.interaction.snap_guide = None;
    }

    pub(super) fn select_clips_in_marquee(&mut self) {
        let Some(selection) = self.interaction.marquee_selection.as_ref() else {
            return;
        };
        let left = selection.start_x.min(selection.current_x);
        let right = selection.start_x.max(selection.current_x);
        let top = selection.start_y.min(selection.current_y);
        let bottom = selection.start_y.max(selection.current_y);
        let scroll_x = f32::from(self.h_scroll.offset().x);
        let scroll_y = f32::from(self.v_scroll.offset().y);

        let mut selected = selection.initial_selection.clone();
        for (track_index, track) in self.data.tracks.iter().enumerate() {
            let clip_top = TIMELINE_HEADER_HEIGHT
                + RULER_HEIGHT
                + track_index as f32 * TRACK_HEIGHT
                + scroll_y
                + 5.0;
            let clip_bottom = clip_top + TRACK_HEIGHT - 10.0;
            for clip in self.data.clips_on_track(track.id) {
                let clip_left = TRACK_HEADER_WIDTH
                    + scroll_x
                    + TIMELINE_PADDING
                    + self.data.seconds(clip.timeline_start()) as f32
                        * self.data.view.pixels_per_second;
                let clip_right = clip_left
                    + (self
                        .data
                        .seconds(clip.frame_length(self.data.settings.frame_rate))
                        as f32
                        * self.data.view.pixels_per_second)
                        .max(4.0);
                if clip_left <= right
                    && clip_right >= left
                    && clip_top <= bottom
                    && clip_bottom >= top
                {
                    selected.insert(clip.id());
                }
            }
        }
        self.interaction.selected_clip_id = self
            .data
            .clips
            .iter()
            .find(|clip| selected.contains(&clip.id()))
            .map(Clip::id);
        self.interaction.selected_clip_ids = selected;
    }

    pub(super) fn snap_time_ignoring(
        &self,
        time: TimelineTime,
        ignored_clip_ids: &HashSet<Ulid>,
    ) -> (TimelineTime, Option<TimelineTime>) {
        if !self.interaction.snapping_enabled {
            return (time.max(TimelineTime::ZERO), None);
        }
        let threshold = self
            .data
            .settings
            .frame_rate
            .ceil(SNAP_DISTANCE_PX as f64 / self.data.view.pixels_per_second as f64)
            .frames()
            .max(1) as u64;
        let mut candidates = vec![TimelineTime::ZERO, self.playhead];
        for clip in &self.data.clips {
            if !ignored_clip_ids.contains(&clip.id()) {
                candidates.push(clip.timeline_start());
                candidates.push(clip.timeline_end(self.data.settings.frame_rate));
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
        ignored_clip_ids: &HashSet<Ulid>,
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

    pub(super) fn zoom(&mut self, factor: f32) {
        let previous_pixels_per_second = self.data.view.pixels_per_second;
        let pixels_per_second = (self.data.view.pixels_per_second * factor).clamp(
            MIN_TIMELINE_PIXELS_PER_SECOND,
            MAX_TIMELINE_PIXELS_PER_SECOND,
        );
        if pixels_per_second != previous_pixels_per_second {
            let mut scroll_offset = self.h_scroll.offset();
            let playhead_seconds = self.data.seconds(self.playhead);
            scroll_offset.x = px(zoom_scroll_offset(
                f32::from(scroll_offset.x),
                playhead_seconds,
                previous_pixels_per_second,
                pixels_per_second,
            ));
            self.h_scroll.set_offset(scroll_offset);
            edit_timeline(self, EditAction::SetTimelineZoom { pixels_per_second })
                .expect("changing timeline zoom cannot be rejected");
        }
    }

    pub(super) fn timeline_position_from_x(&self, x: f32) -> TimelineTime {
        let scroll_x: f32 = self.h_scroll.offset().x.into();
        let content_x = x - TRACK_HEADER_WIDTH - scroll_x - TIMELINE_PADDING;
        self.data
            .nearest_time(content_x as f64 / self.data.view.pixels_per_second as f64)
            .clamp(TimelineTime::ZERO, self.data.content_duration())
    }
}

pub(super) fn choose_clip_snap(
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

pub(super) fn zoom_scroll_offset(
    previous_offset: f32,
    anchor_seconds: f64,
    previous_pixels_per_second: f32,
    pixels_per_second: f32,
) -> f32 {
    let anchor_seconds = anchor_seconds as f32;
    (previous_offset + anchor_seconds * (previous_pixels_per_second - pixels_per_second)).min(0.0)
}

impl Editor {
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
        let position = timeline
            .timeline_position_from_x(event.position.x.into())
            .clamp(TimelineTime::ZERO, timeline.data.content_duration());
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

    pub(super) fn handle_clip_mouse_down(
        &mut self,
        clip_id: Ulid,
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
                let Some(timeline) = self.timeline.as_mut() else {
                    return;
                };
                let position = timeline.timeline_position_from_x(event.position.x.into());
                timeline.playhead = position;
                timeline.blade_at_playhead(&mut self.preview, &self.global_settings.project_root);
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
        if !event.dragging() {
            return;
        }
        let (x, y) = Self::timeline_pointer_position(
            event.position.x.into(),
            event.position.y.into(),
            window,
        );
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        let Some(selection) = timeline.interaction.marquee_selection.as_mut() else {
            return;
        };
        selection.current_x = x;
        selection.current_y = y;
        timeline.select_clips_in_marquee();
        cx.notify();
    }

    pub(super) fn finish_marquee_selection(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (x, y) = Self::timeline_pointer_position(
            event.position.x.into(),
            event.position.y.into(),
            window,
        );
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        let Some(selection) = timeline.interaction.marquee_selection.as_mut() else {
            return;
        };
        selection.current_x = x;
        selection.current_y = y;
        timeline.select_clips_in_marquee();
        timeline.interaction.marquee_selection = None;
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

    pub(super) fn begin_clip_move(
        &mut self,
        clip_id: Ulid,
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
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        if !timeline.selected_clips_editable() {
            return;
        }
        let Some(anchor) = timeline.data.clip(clip_id).cloned() else {
            return;
        };
        let Some(original_anchor_track_index) = timeline
            .data
            .tracks
            .iter()
            .position(|track| track.id == anchor.track_id())
        else {
            return;
        };
        let items = timeline
            .selected_clip_ids_in_timeline_order()
            .into_iter()
            .filter_map(|selected_id| {
                let clip = timeline.data.clip(selected_id)?;
                let original_track_index = timeline
                    .data
                    .tracks
                    .iter()
                    .position(|track| track.id == clip.track_id())?;
                Some(ClipMoveItem {
                    clip_id: selected_id,
                    original_timeline_start: clip.timeline_start(),
                    original_track_id: clip.track_id(),
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
        if let Some(video) = self.preview.target.video() {
            video.set_paused(true);
        }
        let timeline = self.timeline.as_mut().expect("timeline was checked above");
        timeline.interaction.snap_guide = None;
        timeline.interaction.clip_move_drag = Some(ClipMoveDrag {
            anchor_clip_id: clip_id,
            start_x: event.position.x.into(),
            original_anchor_start: anchor.timeline_start(),
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
            (f32::from(event.position.x) - start_x) as f64
                / timeline.data.view.pixels_per_second as f64,
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
            .map(|clip| clip.frame_length(timeline.data.settings.frame_rate))
            .unwrap_or(TimelineTime::ZERO);
        let (snapped_start, snap_guide) = timeline.snap_clip_start_ignoring(
            raw_anchor_start,
            anchor_duration,
            &timeline.interaction.selected_clip_ids,
        );
        let timeline_delta = snapped_start - original_anchor_start;
        let viewport_height = f32::from(window.viewport_size().height);
        let track_top = viewport_height - TIMELINE_HEIGHT + TIMELINE_HEADER_HEIGHT + RULER_HEIGHT;
        let scroll_y: f32 = timeline.v_scroll.offset().y.into();
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
            timeline
                .data
                .validate_clip_move_placements(&placements, &timeline.interaction.selected_clip_ids)
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
            timeline.record_editing_history();
            edit_and_rebuild_timeline(
                &mut self.preview,
                &self.global_settings.project_root,
                timeline,
                EditAction::MoveClips {
                    placements: drag.placements,
                },
            )
            .expect("clip move placements were validated during the drag");
            let playhead = timeline.playhead;
            timeline.save(&self.global_settings.project_root);

            self.load_timeline_position_with_options(playhead, true);
        }
        cx.notify();
    }

    pub(super) fn toggle_snapping(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.interaction.snapping_enabled = !timeline.interaction.snapping_enabled;
        timeline.interaction.snap_guide = None;
        edit_and_rebuild_timeline(
            &mut self.preview,
            &self.global_settings.project_root,
            timeline,
            EditAction::SetSnapping {
                enabled: timeline.interaction.snapping_enabled,
            },
        )
        .expect("changing snapping cannot be rejected");
        timeline.save(&self.global_settings.project_root);
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
            self.save_timeline_scroll();
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

        let Some(timeline) = self.timeline.as_mut() else {
            return false;
        };
        let previous_zoom = timeline.data.view.pixels_per_second;
        let factor = (gesture.magnification as f32).exp().clamp(0.5, 2.0);
        timeline.zoom(factor);
        let current_zoom = timeline.data.view.pixels_per_second;
        log::debug!(
            target: "opencut::timeline",
            "trackpad-pinch magnification={:.4} location_y={:.1} ended={} action=zoom factor={factor:.4} px_per_second={previous_zoom:.2}->{:.2}",
            gesture.magnification,
            gesture.location_y,
            gesture.ended,
            current_zoom,
        );
        let changed = current_zoom != previous_zoom;
        if gesture.ended {
            self.save_timeline_scroll();
        }
        changed
    }

    pub(super) fn begin_playhead_scrub(&mut self, event: &MouseDownEvent) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.interaction.scrubbing_playhead = true;
        if let Some(video) = self.preview.target.video() {
            video.set_paused(true);
        }
        let position = timeline.timeline_position_from_x(event.position.x.into());
        timeline.interaction.last_scrub_seek = Some(Instant::now());
        self.load_timeline_position_with_options(position, false);
    }

    pub(super) fn update_playhead_scrub(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !event.dragging() {
            return;
        }
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        if !timeline.interaction.scrubbing_playhead {
            return;
        }
        let position = timeline.timeline_position_from_x(event.position.x.into());
        timeline.playhead = position;

        let now = Instant::now();
        let should_seek = timeline
            .interaction
            .last_scrub_seek
            .is_none_or(|last_seek| now.duration_since(last_seek) >= SCRUB_SEEK_INTERVAL);
        if should_seek {
            timeline.interaction.last_scrub_seek = Some(now);
            self.load_timeline_position_with_options(position, false);
        }
        cx.notify();
    }

    pub(super) fn finish_playhead_scrub(
        &mut self,
        event: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        if !timeline.interaction.scrubbing_playhead {
            return;
        }
        timeline.interaction.scrubbing_playhead = false;
        timeline.interaction.last_scrub_seek = None;
        let position = timeline.timeline_position_from_x(event.position.x.into());
        self.load_timeline_position_with_options(position, true);
        self.save_timeline_playhead();
        cx.notify();
    }

    pub(super) fn step_playhead(&mut self, frames: i64) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        if timeline.data.clips.is_empty() {
            return;
        }
        let target = (timeline.playhead + TimelineTime::from_frames(frames))
            .clamp(TimelineTime::ZERO, timeline.data.content_duration());
        if target != timeline.playhead || !self.preview.target.is_timeline() {
            self.load_timeline_position_with_options(target, true);
            self.save_timeline_playhead();
        }
    }
}

#[cfg(test)]
#[path = "timeline_interactions.test.rs"]
mod tests;
