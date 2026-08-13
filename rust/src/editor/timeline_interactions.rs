use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TimelineTool {
    Selection,
    Blade,
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
                timeline.blade_at_playhead(&self.global_settings.project_root);
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
            .position(|track| track.id == anchor.track_id)
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
            .map(TimelineClip::duration)
            .unwrap_or(TimelineTime::ZERO);
        let (snapped_start, snap_guide) = timeline.snap_clip_start_ignoring(
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
            self.preview.timeline_needs_rebuild = true;
            for (clip_id, track_id, start) in drag.placements {
                if let Some(clip) = timeline.data.clip_mut(clip_id) {
                    clip.timeline_start = start;
                    clip.track_id = track_id;
                }
            }
            let playhead = timeline.playhead;
            timeline.save(&self.global_settings.project_root);
            self.rebuild_timeline_preview_if_needed();
            self.load_timeline_position_with_options(playhead, false, true);
        }
        cx.notify();
    }

    pub(super) fn toggle_snapping(&mut self) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.interaction.snapping_enabled = !timeline.interaction.snapping_enabled;
        timeline.interaction.snap_guide = None;
        timeline.data.view.snapping_enabled = timeline.interaction.snapping_enabled;
        timeline.save(&self.global_settings.project_root);
        self.rebuild_timeline_preview_if_needed();
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
        if let Some(video) = &self.preview.video {
            video.set_paused(true);
        }
        self.preview.playing = false;
        self.preview.timeline_clock = None;
        let position = timeline.timeline_position_from_x(event.position.x.into());
        timeline.interaction.last_scrub_seek = Some(Instant::now());
        self.load_timeline_position_with_options(position, false, false);
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
            self.load_timeline_position_with_options(position, false, false);
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
        self.load_timeline_position_with_options(position, false, true);
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
            .clamp(TimelineTime::ZERO, timeline.data.timeline_duration());
        if target != timeline.playhead || self.preview.target != PreviewTarget::Timeline {
            self.load_timeline_position_with_options(target, false, true);
            self.save_timeline_playhead();
        }
    }
}

#[cfg(test)]
#[path = "timeline_interactions.test.rs"]
mod tests;
