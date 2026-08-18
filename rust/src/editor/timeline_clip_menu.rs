use super::*;

#[derive(Clone, Copy)]
pub(super) struct TimelineClipContextMenu {
    clip_id: Ulid,
    x: f32,
    y: f32,
}

fn transform_targets(
    timeline: &Timeline,
    source_clip_id: Ulid,
) -> Option<(VideoClipProperties, Vec<usize>)> {
    let source = timeline.clip(source_clip_id)?;
    let track = timeline.track(source.track_id)?;
    let source_asset = timeline.asset(source.asset_id)?;
    if track.locked || track.kind != TrackKind::Video || source_asset.kind == MediaKind::Audio {
        return None;
    }
    let properties = source.video_properties;
    let targets = timeline
        .clips
        .iter()
        .enumerate()
        .filter(|(_, clip)| clip.id != source.id && clip.track_id == source.track_id)
        .filter(|(_, clip)| {
            timeline
                .asset(clip.asset_id)
                .is_some_and(|asset| asset.kind != MediaKind::Audio)
        })
        .filter(|(_, clip)| clip.video_properties != properties)
        .map(|(index, _)| index)
        .collect();
    Some((properties, targets))
}

impl Editor {
    pub(super) fn timeline_clip_menu_overlay(
        &self,
        menu: &TimelineClipContextMenu,
        viewport: gpui::Size<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let width = 292.0;
        let height = 56.0;
        let left = menu
            .x
            .clamp(8.0, (f32::from(viewport.width) - width - 8.0).max(8.0));
        let top = menu
            .y
            .clamp(8.0, (f32::from(viewport.height) - height - 8.0).max(8.0));
        let enabled = self
            .timeline
            .as_ref()
            .and_then(|timeline| transform_targets(&timeline.data, menu.clip_id))
            .is_some_and(|(_, targets)| !targets.is_empty());

        div()
            .id("timeline-clip-context-menu-overlay")
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
                    .id("timeline-clip-context-menu")
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
                            .id("apply-transform-to-track-clips")
                            .h(px(40.0))
                            .px_3()
                            .flex()
                            .items_center()
                            .rounded_md()
                            .cursor(if enabled {
                                CursorStyle::PointingHand
                            } else {
                                CursorStyle::Arrow
                            })
                            .text_color(rgb(if enabled { TEXT } else { MUTED }))
                            .when(enabled, |this| {
                                this.hover(|style| style.bg(rgb(0x34343a)))
                                    .on_click(cx.listener(|editor, _, _, cx| {
                                        editor.apply_transform_to_track_clips();
                                        cx.notify();
                                    }))
                            })
                            .child(div().text_sm().child("Apply Transforms to All Other Clips")),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn show_timeline_clip_context_menu(
        &mut self,
        clip_id: Ulid,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        self.explorer.context_menu = None;
        self.select_only_clip(Some(clip_id));
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.interaction.context_menu = Some(TimelineClipContextMenu {
            clip_id,
            x: event.position.x.into(),
            y: event.position.y.into(),
        });
        cx.stop_propagation();
        cx.notify();
    }

    fn apply_transform_to_track_clips(&mut self) {
        let Some(source_clip_id) = self
            .timeline
            .as_mut()
            .and_then(|timeline| timeline.interaction.context_menu.take())
            .map(|menu| menu.clip_id)
        else {
            return;
        };
        let Some((properties, targets)) = self
            .timeline
            .as_ref()
            .and_then(|timeline| transform_targets(&timeline.data, source_clip_id))
        else {
            return;
        };
        if targets.is_empty() {
            return;
        }
        let changed = targets.len();
        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.record_editing_history();
        self.preview.timeline_needs_rebuild = true;
        for index in targets {
            timeline.data.clips[index].video_properties = properties;
        }
        self.properties.transform_input_clip_id = None;

        timeline.save(&self.global_settings.project_root);
        self.rebuild_timeline_preview_if_needed();
        self.status = Some(format!(
            "Applied transforms to {changed} other clip{}.",
            if changed == 1 { "" } else { "s" }
        ));
    }
}

#[cfg(test)]
#[path = "timeline_clip_menu.test.rs"]
mod tests;
