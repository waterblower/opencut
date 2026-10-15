use crate::event_bus::{AppEvent, EventBus};
use gpui::prelude::*;
use gpui::{App, Entity, MouseButton, Window, div, px, rgb};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy)]
pub struct VerticalSplitConstraints {
    pub min_top: f32,
    pub min_bottom: f32,
}

#[derive(Clone, Copy)]
pub struct VerticalSplitHeights {
    pub top: f32,
    pub bottom: f32,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct VerticalSplitState {
    bottom_height: f32, // 用户期望的下方区域高度；实际高度由 heights() 按约束和总高度推导。
    #[serde(skip)]
    drag_offset: Option<f32>, // 拖动分隔条期间才有值；None：没有进行中的拖动。
}

// Public APIs only
impl VerticalSplitState {
    pub fn new(bottom_height: f32) -> Self {
        Self {
            bottom_height,
            drag_offset: None,
        }
    }

    pub fn heights(
        &self,
        total_height: f32,
        constraints: VerticalSplitConstraints,
    ) -> VerticalSplitHeights {
        let available = (total_height - DIVIDER_HEIGHT).max(0.0);
        let bottom_max = (available - constraints.min_top).max(constraints.min_bottom);
        let bottom = self.bottom_height.clamp(constraints.min_bottom, bottom_max);
        let top = (available - bottom).max(0.0);
        VerticalSplitHeights { top, bottom }
    }
}

// Private APIs only
impl VerticalSplitState {
    fn begin_drag(&mut self, pointer_offset: f32) {
        self.drag_offset = Some(pointer_offset + DIVIDER_HANDLE_TOP);
    }

    fn resize(&mut self, pointer_y: f32, total_height: f32, constraints: VerticalSplitConstraints) {
        let Some(drag_offset) = self.drag_offset else {
            return;
        };
        let divider_y = pointer_y - drag_offset;
        let max = (total_height - DIVIDER_HEIGHT - constraints.min_top).max(constraints.min_bottom);
        self.bottom_height =
            (total_height - divider_y - DIVIDER_HEIGHT).clamp(constraints.min_bottom, max);
    }
}

#[derive(IntoElement)]
pub struct VerticalSplit {
    id: &'static str,
    state: Entity<VerticalSplitState>,
    event_bus: Entity<EventBus>,
    total_height: f32,
    constraints: VerticalSplitConstraints,
    top: gpui::AnyElement,
    bottom: gpui::AnyElement,
}

// Public APIs only
impl VerticalSplit {
    pub fn new(
        id: &'static str,
        state: Entity<VerticalSplitState>,
        event_bus: Entity<EventBus>,
        total_height: f32,
        constraints: VerticalSplitConstraints,
        top: impl IntoElement,
        bottom: impl IntoElement,
    ) -> Self {
        Self {
            id,
            state,
            event_bus,
            total_height,
            constraints,
            top: top.into_any_element(),
            bottom: bottom.into_any_element(),
        }
    }
}

impl RenderOnce for VerticalSplit {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let heights = self
            .state
            .read(cx)
            .heights(self.total_height, self.constraints);
        let drag_state = self.state.clone();
        let release_state = self.state.clone();
        let release_event_bus = self.event_bus.clone();
        let release_out_state = self.state.clone();
        let release_out_event_bus = self.event_bus.clone();
        let divider = div()
            .id("vertical-split-divider")
            .relative()
            .w_full()
            .h(px(DIVIDER_HEIGHT))
            .flex_shrink_0()
            .bg(rgb(DIVIDER_COLOR))
            .child(
                div()
                    .id("vertical-split-divider-handle")
                    .absolute()
                    .top(px(DIVIDER_HANDLE_TOP))
                    .w_full()
                    .h(px(DIVIDER_HANDLE_HEIGHT))
                    .cursor_row_resize()
                    .block_mouse_except_scroll()
                    .on_drag(VerticalSplitDrag, move |_, offset, _, cx| {
                        drag_state.update(cx, |state, _| {
                            state.begin_drag(offset.y.into());
                        });
                        cx.new(|_| gpui::Empty)
                    })
                    // A drag usually ends over this handle, which hides the container from
                    // hover checks; a release elsewhere arrives as mouse-up-out.
                    .on_mouse_up(MouseButton::Left, move |_, _, cx| {
                        finish_drag(&release_state, &release_event_bus, cx);
                    })
                    .on_mouse_up_out(MouseButton::Left, move |_, _, cx| {
                        finish_drag(&release_out_state, &release_out_event_bus, cx);
                    }),
            );
        let state = self.state.clone();
        let constraints = self.constraints;
        let total_height = self.total_height;

        div()
            .id(self.id)
            .w_full()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .flex_col()
            .on_drag_move::<VerticalSplitDrag>(move |event, _, cx| {
                let pointer_y: f32 = (event.event.position.y - event.bounds.top()).into();
                state.update(cx, |state, cx| {
                    state.resize(pointer_y, total_height, constraints);
                    cx.notify();
                });
            })
            .child(
                div()
                    .w_full()
                    .h(px(heights.top))
                    .min_h_0()
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(self.top),
            )
            .child(divider)
            .child(
                div()
                    .w_full()
                    .h(px(heights.bottom))
                    .min_h_0()
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(self.bottom),
            )
    }
}

const DIVIDER_HEIGHT: f32 = 1.0;
const DIVIDER_HANDLE_TOP: f32 = -4.0;
const DIVIDER_HANDLE_HEIGHT: f32 = 9.0;
const DIVIDER_COLOR: u32 = 0x4a4a52;

#[derive(Clone)]
struct VerticalSplitDrag;

/// Saves the heights once when a divider drag ends. Plain clicks have no drag to finish.
fn finish_drag(state: &Entity<VerticalSplitState>, event_bus: &Entity<EventBus>, cx: &mut App) {
    let finished = state.update(cx, |state, _| state.drag_offset.take().is_some());
    if !finished {
        return;
    }
    let heights = state.read(cx).clone();
    event_bus.update(cx, |_, cx| cx.emit(AppEvent::VerticalSplitResized(heights)));
}
