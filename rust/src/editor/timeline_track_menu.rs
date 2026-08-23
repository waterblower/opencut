use super::*;

pub(super) struct TextTrackContextMenu {
    pub(super) track_id: Ulid,
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
                            .text_color(rgb(TEXT))
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
        timeline.interaction.context_menu =
            Some(TimelineContextMenu::TextTrack(TextTrackContextMenu {
                track_id,
                x: event.position.x.into(),
                y: event.position.y.into(),
            }));
        cx.stop_propagation();
        cx.notify();
    }
}
