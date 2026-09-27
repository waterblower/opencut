use serde::{Deserialize, Serialize};

use super::super::*;

pub const HORIZONTAL_SPLIT_DIVIDER_WIDTH: f32 = 1.0;

#[derive(Clone, Copy)]
pub struct HorizontalSplitConstraints {
    pub min_left: f32,
    pub min_center: f32,
    pub min_right: f32,
}

#[derive(Clone, Copy)]
pub struct HorizontalSplitWidths {
    pub left: f32,
    pub center: f32,
    pub right: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct HorizontalSplitState {
    left_width: f32,
    right_width: f32,
    // it only matters during an active drag
    #[serde(skip)]
    drag_offset: f32,
}

impl HorizontalSplitState {
    pub fn new(left_width: f32, right_width: f32) -> Self {
        Self {
            left_width,
            right_width,
            drag_offset: 0.0,
        }
    }

    pub fn widths(
        &self,
        total_width: f32,
        constraints: HorizontalSplitConstraints,
    ) -> HorizontalSplitWidths {
        let available = (total_width - HORIZONTAL_SPLIT_DIVIDER_WIDTH * 2.0).max(0.0);
        let left_max =
            (available - constraints.min_center - constraints.min_right).max(constraints.min_left);
        let left = self.left_width.clamp(constraints.min_left, left_max);
        let right_max = (available - constraints.min_center - left).max(constraints.min_right);
        let right = self.right_width.clamp(constraints.min_right, right_max);
        let center = (available - left - right).max(0.0);
        HorizontalSplitWidths {
            left,
            center,
            right,
        }
    }

    fn begin_drag(&mut self, pointer_offset: f32) {
        self.drag_offset = pointer_offset + DIVIDER_HANDLE_LEFT;
    }

    fn resize(
        &mut self,
        divider: usize,
        pointer_x: f32,
        total_width: f32,
        constraints: HorizontalSplitConstraints,
    ) {
        let divider_x = pointer_x - self.drag_offset;
        let widths = self.widths(total_width, constraints);
        match divider {
            0 => {
                let max = (total_width
                    - HORIZONTAL_SPLIT_DIVIDER_WIDTH * 2.0
                    - widths.right
                    - constraints.min_center)
                    .max(constraints.min_left);
                self.left_width = divider_x.clamp(constraints.min_left, max);
            }
            1 => {
                let max = (total_width
                    - HORIZONTAL_SPLIT_DIVIDER_WIDTH * 2.0
                    - widths.left
                    - constraints.min_center)
                    .max(constraints.min_right);
                self.right_width = (total_width - divider_x - HORIZONTAL_SPLIT_DIVIDER_WIDTH)
                    .clamp(constraints.min_right, max);
            }
            _ => panic!("horizontal split divider index must be 0 or 1"),
        }
    }
}

#[derive(IntoElement)]
pub struct HorizontalSplit {
    id: &'static str,
    state: Entity<HorizontalSplitState>,
    event_bus: Entity<EventBus>,
    total_width: f32,
    constraints: HorizontalSplitConstraints,
    left: gpui::AnyElement,
    center: gpui::AnyElement,
    right: gpui::AnyElement,
}

impl HorizontalSplit {
    pub fn new(
        id: &'static str,
        state: Entity<HorizontalSplitState>,
        event_bus: Entity<EventBus>,
        total_width: f32,
        constraints: HorizontalSplitConstraints,
        left: impl IntoElement,
        center: impl IntoElement,
        right: impl IntoElement,
    ) -> Self {
        Self {
            id,
            state,
            event_bus,
            total_width,
            constraints,
            left: left.into_any_element(),
            center: center.into_any_element(),
            right: right.into_any_element(),
        }
    }
}

impl RenderOnce for HorizontalSplit {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let widths = self
            .state
            .read(cx)
            .widths(self.total_width, self.constraints);
        let divider = |index| {
            let state = self.state.clone();
            div()
                .id(("horizontal-split-divider", index))
                .relative()
                .w(px(HORIZONTAL_SPLIT_DIVIDER_WIDTH))
                .h_full()
                .flex_shrink_0()
                .bg(rgb(DIVIDER_COLOR))
                .child(
                    div()
                        .id(("horizontal-split-divider-handle", index))
                        .absolute()
                        .left(px(DIVIDER_HANDLE_LEFT))
                        .w(px(DIVIDER_HANDLE_WIDTH))
                        .h_full()
                        .cursor_col_resize()
                        .block_mouse_except_scroll()
                        .on_drag(
                            HorizontalSplitDrag { divider: index },
                            move |_, offset, _, cx| {
                                state.update(cx, |state, _| {
                                    state.begin_drag(offset.x.into());
                                });
                                cx.new(|_| gpui::Empty)
                            },
                        ),
                )
        };
        let state = self.state.clone();
        let constraints = self.constraints;
        let total_width = self.total_width;
        let finish_state = self.state.clone();

        div()
            .id(self.id)
            .w_full()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .flex()
            .capture_any_mouse_up(move |event, _, cx| {
                if event.button == MouseButton::Left {
                    let state = finish_state.read(cx).clone();
                    self.event_bus
                        .update(cx, |_, cx| cx.emit(AppEvent::HorizontalSplitResized(state)));
                }
            })
            .on_drag_move::<HorizontalSplitDrag>(move |event, _, cx| {
                let divider = event.drag(cx).divider;
                let pointer_x: f32 = (event.event.position.x - event.bounds.left()).into();
                state.update(cx, |state, cx| {
                    state.resize(divider, pointer_x, total_width, constraints);
                    cx.notify();
                });
            })
            .child(
                div()
                    .w(px(widths.left))
                    .h_full()
                    .min_w_0()
                    .flex_shrink_0()
                    .overflow_hidden()
                    .child(self.left),
            )
            .child(divider(0))
            .child(
                div()
                    .w(px(widths.center))
                    .h_full()
                    .min_w_0()
                    .flex_shrink_0()
                    .overflow_hidden()
                    .child(self.center),
            )
            .child(divider(1))
            .child(
                div()
                    .w(px(widths.right))
                    .h_full()
                    .min_w_0()
                    .flex_shrink_0()
                    .overflow_hidden()
                    .child(self.right),
            )
    }
}

const DIVIDER_HANDLE_LEFT: f32 = -4.0;
const DIVIDER_HANDLE_WIDTH: f32 = 9.0;
const DIVIDER_COLOR: u32 = 0x4a4a52;

#[derive(Clone)]
struct HorizontalSplitDrag {
    divider: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONSTRAINTS: HorizontalSplitConstraints = HorizontalSplitConstraints {
        min_left: 220.0,
        min_center: 320.0,
        min_right: 240.0,
    };

    #[test]
    fn serialization_does_not_restore_an_active_drag() {
        let mut state = HorizontalSplitState::new(410.0, 320.0);
        state.begin_drag(12.0);
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"left_width": 410.0, "right_width": 320.0})
        );
        let restored: HorizontalSplitState = serde_json::from_value(json).unwrap();
        assert_eq!(restored.drag_offset, 0.0);
    }

    #[test]
    fn both_dividers_resize_their_outer_pane() {
        let mut state = HorizontalSplitState::new(340.0, 420.0);

        state.resize(0, 400.0, 1440.0, CONSTRAINTS);
        let widths = state.widths(1440.0, CONSTRAINTS);
        assert_eq!(widths.left, 400.0);
        assert_eq!(widths.center, 618.0);
        assert_eq!(widths.right, 420.0);

        state.resize(1, 900.0, 1440.0, CONSTRAINTS);
        let widths = state.widths(1440.0, CONSTRAINTS);
        assert_eq!(widths.left, 400.0);
        assert_eq!(widths.center, 499.0);
        assert_eq!(widths.right, 539.0);
    }
}
