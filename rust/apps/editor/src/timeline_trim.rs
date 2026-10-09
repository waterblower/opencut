use crate::edit_action::EditAction;
use crate::editor::Editor;
use crate::event_bus::AppEvent;
use crate::preview::PreviewTarget;
use crate::theme::ACCENT;
use crate::timeline_clip::Clip;
use crate::timeline_interactions::TimelineTool;
use gpui::prelude::*;
use gpui::{Context, CursorStyle, MouseButton, MouseMoveEvent, MouseUpEvent, Window, div, px, rgb};
use ulid::Ulid;

pub struct ClipTrimDrag {
    pub original: Clip, // 拖动起点快照；每次移动都从这里计算，避免累计误差。
    pub start_edge: bool,
    pub pointer_x: f32,
    pub history_recorded: bool,
}

// Public APIs only
impl Editor {
    pub fn clip_trim_handles(
        &self,
        clip_id: Ulid,
        cx: &mut Context<Self>,
    ) -> Vec<gpui::AnyElement> {
        let Some(timeline) = self.timeline.as_ref() else {
            return Vec::new();
        };
        if timeline.interaction.active_tool != TimelineTool::Selection
            || timeline.editing_state.clip_locked(clip_id)
        {
            return Vec::new();
        }
        let Some(clip) = timeline.editing_state.clip(clip_id) else {
            return Vec::new();
        };
        let width = (timeline
            .editing_state
            .seconds(clip.frame_length(timeline.editing_state.settings.frame_rate))
            as f32
            * timeline.pixels_per_second)
            .max(4.0);
        let handle_width = (width / 2.0).min(6.0);
        [true, false]
            .into_iter()
            .map(|start_edge| {
                div()
                    .id(if start_edge { "trim-start" } else { "trim-end" })
                    .absolute()
                    .top_0()
                    .bottom_0()
                    .w(px(handle_width))
                    .when(start_edge, |handle| handle.left_0())
                    .when(!start_edge, |handle| handle.right_0())
                    .cursor(CursorStyle::ResizeLeftRight)
                    .hover(|handle| handle.bg(rgb(ACCENT)))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |editor, event: &gpui::MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            editor.select_only_clip(Some(clip_id));
                            let Some(timeline) = editor.timeline.as_mut() else {
                                return;
                            };
                            if timeline.editing_state.clip_locked(clip_id) {
                                return;
                            }
                            let Some(original) = timeline.editing_state.clip(clip_id).cloned()
                            else {
                                return;
                            };
                            timeline.clip_trim_drag = Some(ClipTrimDrag {
                                original,
                                start_edge,
                                pointer_x: f32::from(event.position.x)
                                    - f32::from(timeline.h_scroll.offset().x),
                                history_recorded: false,
                            });
                            timeline.interaction.clip_move_drag = None;
                            timeline.interaction.snap_guide = None;
                            if let PreviewTarget::Timeline { player, .. } = &editor.preview.target {
                                player.update(cx, |player, cx| {
                                    player.backend_mut().pause();
                                    cx.notify();
                                });
                            }
                            cx.notify();
                        }),
                    )
                    .into_any_element()
            })
            .collect()
    }

    pub fn move_clip_trim(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(timeline) = &self.timeline else {
            return;
        };
        if timeline.clip_trim_drag.is_none() {
            return;
        }
        self.emit_event(
            cx,
            AppEvent::Edit(EditAction::TrimClip {
                timeline_path: timeline.path.clone(),
                pointer_x: f32::from(event.position.x),
                finished: event.pressed_button != Some(MouseButton::Left),
            }),
        );
    }

    pub fn release_clip_trim(
        &mut self,
        event: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(timeline) = &self.timeline else {
            return;
        };
        if timeline.clip_trim_drag.is_none() {
            return;
        }
        self.emit_event(
            cx,
            AppEvent::Edit(EditAction::TrimClip {
                timeline_path: timeline.path.clone(),
                pointer_x: f32::from(event.position.x),
                finished: true,
            }),
        );
    }
}
