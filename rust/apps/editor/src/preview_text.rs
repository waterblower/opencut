use crate::edit_action::EditAction;
use crate::editor::Editor;
use crate::event_bus::AppEvent;
use crate::preview::PreviewTarget;
use crate::theme::ACCENT;
use engine::timeline_decoder::PreparedLayer;
use engine::timeline_view::text_layer_element;
use gpui::prelude::*;
use gpui::{
    AvailableSpace, BorderStyle, Bounds, Context, Entity, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, Pixels, Point, Size, Window, canvas, div, outline, point, px,
    rgb, size,
};
use player_ui::timeline_player::TimelinePlayer;
use player_ui::timeline_player_view::TIMELINE_CONTROLS_HEIGHT;
use std::{cell::RefCell, path::PathBuf, rc::Rc};
use timeline::Clip;
use ulid::Ulid;

pub struct PreviewTextDrag {
    pub timeline_path: PathBuf,
    pub clip_id: Ulid,
    pub start: Point<Pixels>,
    pub history_recorded: bool, // 首次实际移动时记录撤销，整次拖动只记录一次。
    pub original: Point<f64>,
    pub text_size: Size<Pixels>, // 拖动开始时的完整文字布局尺寸，边缘吸附不使用裁剪后的命中区域。
    pub canvas_size: Size<Pixels>, // 实际画布尺寸，不包含预览黑边和播放控件。
}

impl Editor {
    pub fn preview_timeline_picture(
        &self,
        player: &Entity<TimelinePlayer>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let editor = cx.entity().downgrade();
        let preview = player.clone();
        canvas(
            move |bounds, window, cx| {
                let mut element = match editor.upgrade() {
                    Some(editor) => match editor.read(cx).timeline.as_ref() {
                        Some(timeline) => preview.read(cx).backend().preview_frame().render_frame(
                            bounds.size.width.into(),
                            bounds.size.height.into(),
                            &timeline.editing_state,
                        ),
                        None => div().into_any_element(),
                    },
                    None => div().into_any_element(),
                };
                element.prepaint_as_root(
                    bounds.origin,
                    bounds.size.map(AvailableSpace::Definite),
                    window,
                    cx,
                );
                element
            },
            |_, mut element, window, cx| element.paint(window, cx),
        )
        .size_full()
        .into_any_element()
    }

    pub fn preview_text_overlay(
        &self,
        player: &Entity<TimelinePlayer>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let Some(timeline) = self.timeline.as_ref() else {
            return div().into_any_element();
        };
        let frame = player.read(cx).backend().preview_frame();
        let selected = timeline.interaction.selected_clip_id;
        let editable = timeline
            .editing_state
            .clips
            .iter()
            .filter_map(|clip| {
                let Clip::Text(text) = clip else {
                    return None;
                };
                let track = timeline
                    .editing_state
                    .tracks
                    .iter()
                    .find(|track| track.id == text.track_id)?;
                (track.visible && !track.locked).then_some(text.id())
            })
            .collect::<Vec<_>>();
        let hit_regions = Rc::new(RefCell::new(Vec::<(
            Ulid,
            Bounds<Pixels>,
            Size<Pixels>,
            Size<Pixels>,
        )>::new()));
        let editor = cx.entity().downgrade();
        let layout_regions = hit_regions.clone();
        let paint_regions = hit_regions.clone();
        div()
            .id("preview-text-overlay")
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom(px(TIMELINE_CONTROLS_HEIGHT))
            .overflow_hidden()
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let scale = (f32::from(bounds.size.width) / frame.width as f32)
                            .min(f32::from(bounds.size.height) / frame.height as f32);
                        let mut guides = Vec::new();
                        let mut regions = layout_regions.borrow_mut();
                        regions.clear();
                        if !scale.is_finite() || scale <= 0.0 {
                            return guides;
                        }
                        let canvas_size = size(
                            px(frame.width as f32 * scale),
                            px(frame.height as f32 * scale),
                        );
                        let origin = point(
                            bounds.origin.x + (bounds.size.width - canvas_size.width) / 2.0,
                            bounds.origin.y + (bounds.size.height - canvas_size.height) / 2.0,
                        );
                        for layer in &frame.layers {
                            let PreparedLayer::Text { clip_id } = layer else {
                                continue;
                            };
                            let Some(editor) = editor.upgrade() else {
                                continue;
                            };
                            let Some(timeline) = editor.read(cx).timeline.as_ref() else {
                                continue;
                            };
                            let Some(Clip::Text(clip)) = timeline.editing_state.clip(*clip_id)
                            else {
                                continue;
                            };
                            let show_guides = timeline.snapping_enabled
                                && timeline
                                    .text_drag
                                    .as_ref()
                                    .is_some_and(|drag| drag.clip_id == *clip_id);
                            let properties = &clip.properties;
                            if !editable.contains(clip_id) {
                                continue;
                            }
                            let position =
                                (properties.position.x as f32, properties.position.y as f32);
                            let mut text = text_layer_element(properties, scale);
                            let text_size = text.layout_as_root(
                                size(AvailableSpace::MinContent, AvailableSpace::MinContent),
                                window,
                                cx,
                            );
                            let text_origin = point(
                                origin.x + canvas_size.width * position.0 - text_size.width / 2.0,
                                origin.y + canvas_size.height * position.1 - text_size.height / 2.0,
                            );
                            if show_guides {
                                for (vertical, center, extent, text_extent) in [
                                    (
                                        true,
                                        position.0 * f32::from(canvas_size.width),
                                        f32::from(canvas_size.width),
                                        f32::from(text_size.width),
                                    ),
                                    (
                                        false,
                                        position.1 * f32::from(canvas_size.height),
                                        f32::from(canvas_size.height),
                                        f32::from(text_size.height),
                                    ),
                                ] {
                                    for (target, line) in [
                                        (extent / 2.0, extent / 2.0),
                                        (text_extent / 2.0, 0.0),
                                        (extent - text_extent / 2.0, extent - 1.0),
                                    ] {
                                        if (center - target).abs() > 0.01 {
                                            continue;
                                        }
                                        let guide = if vertical {
                                            Bounds::new(
                                                point(origin.x + px(line), origin.y),
                                                size(px(1.0), canvas_size.height),
                                            )
                                        } else {
                                            Bounds::new(
                                                point(origin.x, origin.y + px(line)),
                                                size(canvas_size.width, px(1.0)),
                                            )
                                        };
                                        guides.push(guide);
                                    }
                                }
                            }
                            let visible_bounds = Bounds::new(text_origin, text_size)
                                .intersect(&Bounds::new(origin, canvas_size));
                            if visible_bounds.size.width > px(0.0)
                                && visible_bounds.size.height > px(0.0)
                            {
                                regions.push((*clip_id, visible_bounds, canvas_size, text_size));
                            }
                        }
                        guides
                    },
                    move |_, guides, window, _| {
                        for guide in guides {
                            window.paint_quad(gpui::fill(guide, rgb(ACCENT)));
                        }
                        for (clip_id, bounds, _, _) in paint_regions.borrow().iter() {
                            if Some(*clip_id) == selected {
                                window.paint_quad(outline(
                                    *bounds,
                                    rgb(ACCENT),
                                    BorderStyle::Solid,
                                ));
                            }
                        }
                    },
                )
                .size_full(),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |editor, event: &MouseDownEvent, window, cx| {
                    let hit = hit_regions
                        .borrow()
                        .iter()
                        .rev()
                        .find(|(_, bounds, _, _)| bounds.contains(&event.position))
                        .copied();
                    let Some((clip_id, _, canvas_size, text_size)) = hit else {
                        return;
                    };
                    let Some(timeline) = editor.timeline.as_mut() else {
                        return;
                    };
                    let Some(Clip::Text(clip)) = timeline.editing_state.clip(clip_id) else {
                        return;
                    };
                    timeline.text_drag = Some(PreviewTextDrag {
                        timeline_path: timeline.path.clone(),
                        clip_id,
                        start: event.position,
                        history_recorded: false,
                        original: clip.properties.position,
                        canvas_size,
                        text_size,
                    });
                    editor.select_only_clip(Some(clip_id));
                    if let PreviewTarget::Timeline { player, .. } = &editor.preview.target {
                        player.update(cx, |player, cx| {
                            player.backend_mut().pause();
                            cx.notify();
                        });
                    }
                    editor.focus_handle.focus(window, cx);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .into_any_element()
    }

    pub fn move_preview_text(
        &mut self,
        event: &MouseMoveEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(timeline) = &self.timeline else {
            return;
        };
        let Some(drag) = &timeline.text_drag else {
            return;
        };
        self.emit_event(
            cx,
            AppEvent::Edit(EditAction::UpdateTextClipPosition {
                timeline_path: drag.timeline_path.clone(),
                clip_id: drag.clip_id,
                pointer: event.position,
                finished: event.pressed_button != Some(MouseButton::Left),
            }),
        );
    }

    pub fn release_preview_text(
        &mut self,
        event: &MouseUpEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(timeline) = &self.timeline else {
            return;
        };
        let Some(drag) = &timeline.text_drag else {
            return;
        };
        self.emit_event(
            cx,
            AppEvent::Edit(EditAction::UpdateTextClipPosition {
                timeline_path: drag.timeline_path.clone(),
                clip_id: drag.clip_id,
                pointer: event.position,
                finished: true,
            }),
        );
    }
}
