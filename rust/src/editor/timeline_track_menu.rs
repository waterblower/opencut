use super::*;

pub(super) struct TextTrackContextMenu {
    pub(super) track_id: Ulid,
    pub(super) position: TimelineTime,
    pub(super) x: f32,
    pub(super) y: f32,
}

impl Editor {
    pub(super) fn text_track_menu_overlay(
        &self,
        menu: &TextTrackContextMenu,
        viewport: gpui::Size<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let width = 180.0;
        let height = 56.0;
        let left = menu
            .x
            .clamp(8.0, (f32::from(viewport.width) - width - 8.0).max(8.0));
        let top = menu
            .y
            .clamp(8.0, (f32::from(viewport.height) - height - 8.0).max(8.0));
        let track_id = menu.track_id;
        let position = menu.position;

        div()
            .id("text-track-context-menu-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _, _, cx| {
                    if let Some(timeline) = editor.timeline.as_mut() {
                        timeline.interaction.context_menu = None;
                    }
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|editor, _, _, cx| {
                    if let Some(timeline) = editor.timeline.as_mut() {
                        timeline.interaction.context_menu = None;
                    }
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("text-track-context-menu")
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .w(px(width))
                    .p_1()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(0x1b1b1e))
                    .shadow_lg()
                    .occlude()
                    .child(
                        div()
                            .id(gpui::SharedString::from(format!(
                                "add-text-to-track-{}",
                                menu.track_id
                            )))
                            .h(px(40.0))
                            .px_3()
                            .flex()
                            .items_center()
                            .rounded_md()
                            .cursor(CursorStyle::PointingHand)
                            .text_color(rgb(TEXT))
                            .hover(|style| style.bg(rgb(0x34343a)))
                            .on_click(cx.listener(move |editor, _, _, cx| {
                                editor.add_text(track_id, position, cx);
                            }))
                            .child(div().text_sm().child("Add text")),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn show_text_track_context_menu(
        &mut self,
        track_id: Ulid,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        self.explorer.context_menu = None;
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        if !timeline
            .data
            .track(track_id)
            .is_some_and(|track| track.kind == TrackKind::Text)
        {
            return;
        }
        let scroll_x: f32 = timeline.h_scroll.offset().x.into();
        let content_x =
            f32::from(event.position.x) - TRACK_HEADER_WIDTH - scroll_x - TIMELINE_PADDING;
        let position = timeline
            .data
            .nearest_time(content_x as f64 / timeline.data.view.pixels_per_second as f64)
            .max(TimelineTime::ZERO);
        timeline.interaction.context_menu =
            Some(TimelineContextMenu::TextTrack(TextTrackContextMenu {
                track_id,
                position,
                x: event.position.x.into(),
                y: event.position.y.into(),
            }));
        cx.stop_propagation();
        cx.notify();
    }

    fn add_text(&mut self, track_id: Ulid, position: TimelineTime, cx: &mut Context<Self>) {
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.interaction.context_menu = None;
        let clip = match text_clip_at(&timeline.data, track_id, position) {
            Ok(clip) => clip,
            Err(error) => {
                self.status = Some(error.to_string());
                cx.notify();
                return;
            }
        };
        timeline.record_editing_history();
        let clip_id = clip.id();
        edit_and_rebuild_timeline(
            &mut self.preview,
            &self.global_settings.project_root,
            timeline,
            EditAction::AddClips {
                clips: vec![clip],
                assets: Vec::new(),
            },
        )
        .expect("text clip placement was validated before recording history");
        timeline.interaction.selected_clip_id = Some(clip_id);
        timeline.interaction.selected_clip_ids.clear();
        timeline.interaction.selected_clip_ids.insert(clip_id);
        timeline.save(&self.global_settings.project_root);
        self.status = Some("Added text clip.".to_string());
        cx.notify();
    }
}

fn text_clip_at(
    timeline: &TimelineSerialization,
    track_id: Ulid,
    position: TimelineTime,
) -> Result<Clip, &'static str> {
    let Some(track) = timeline.track(track_id) else {
        return Err("The text track is unavailable.");
    };
    if track.kind != TrackKind::Text {
        return Err("Text can only be added to a text track.");
    }
    if track.locked {
        return Err("Unlock the text track before adding text.");
    }
    if timeline.clips_on_track(track_id).any(|clip| {
        clip.timeline_start() <= position
            && position < clip.timeline_end(timeline.settings.frame_rate)
    }) {
        return Err("A text clip already exists at this position.");
    }

    let default_duration = timeline.ceil_time(5.0).max(TimelineTime::ONE_FRAME);
    let duration = timeline
        .clips_on_track(track_id)
        .filter(|clip| clip.timeline_start() > position)
        .map(|clip| clip.timeline_start() - position)
        .min()
        .map_or(default_duration, |available| {
            available.min(default_duration)
        });
    Ok(Clip::Text(TextClip {
        id: Ulid::generate(),
        track_id,
        timeline_start: position,
        length: timeline.duration(duration),
        properties: TextClipProperties::default(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_text_when_the_position_is_inside_an_existing_clip() {
        let track_id = Ulid::generate();
        let mut timeline = TimelineSerialization::default();
        timeline.tracks.push(Track {
            id: track_id,
            name: "Text 1".to_string(),
            kind: TrackKind::Text,
            locked: false,
            muted: false,
            visible: true,
        });
        let clip = text_clip_at(&timeline, track_id, TimelineTime::ZERO).unwrap();
        let clip_end = clip.timeline_end(timeline.settings.frame_rate);
        timeline.clips.push(clip);

        assert_eq!(
            text_clip_at(&timeline, track_id, TimelineTime::ONE_FRAME).unwrap_err(),
            "A text clip already exists at this position."
        );
        assert!(text_clip_at(&timeline, track_id, clip_end).is_ok());
    }
}
