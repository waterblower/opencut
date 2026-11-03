use crate::edit_action::EditAction;
use crate::event_bus::{AppEvent, EventBus};
use crate::property_panel::{
    audio_properties_view, clip_property_panel_frame_view, clip_property_panel_header_view,
    property_panel_heading_view, slider_view, timing_card_view, value_field_view,
};
use crate::theme::{ACCENT, BORDER, MUTED, PANEL, SURFACE, TEXT};
use crate::timeline::FrameRate;
use crate::timeline_clip::VideoClip;
use crate::track::TrackKind;
use crate::ui_components::text_input_v2::{InputConstraint, TextInputV2};
use gpui::prelude::FluentBuilder;
use gpui::{AppContext, FocusHandle};
use gpui::{Bounds, DragMoveEvent, Empty, Pixels, canvas};
use gpui::{
    Entity, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px, rgb,
};
use std::{cell::Cell, rc::Rc};
use timeline::TimelineEditingState;
use ulid::Ulid;

pub(crate) struct VideoClipPanel {
    clip: VideoClip,
    asset_name: String,
    track_name: String,
    frame_rate: FrameRate,
    event_bus: Entity<EventBus>,
    return_focus: FocusHandle,
    locked: bool, // 所在轨道的锁定状态，构造面板时读取。
}

pub(crate) fn video_clip_panel(
    editing_state: &TimelineEditingState,
    clip: &VideoClip,
    event_bus: Entity<EventBus>,
    return_focus: FocusHandle,
) -> VideoClipPanel {
    let asset_name = match editing_state.asset(clip.asset_id) {
        Some(asset) => asset.name().as_str(),
        None => "Missing media",
    };
    let track_name = match editing_state
        .tracks
        .iter()
        .find(|track| track.id == clip.track_id)
    {
        Some(track) => track.name.as_str(),
        None => "Unknown track",
    };
    VideoClipPanel {
        clip: clip.clone(),
        asset_name: asset_name.to_owned(),
        track_name: track_name.to_owned(),
        frame_rate: editing_state.settings.frame_rate,
        event_bus,
        return_focus,
        locked: editing_state.clip_locked(clip.id()),
    }
}

pub(super) fn video_clip_view(panel: &VideoClipPanel) -> gpui::AnyElement {
    let clip = &panel.clip;
    let asset_name = panel.asset_name.as_str();
    let track_name = panel.track_name.as_str();
    let frame_rate = panel.frame_rate;
    let duration = clip.source_out - clip.source_in;
    let scale_percent = clip.video_properties.scale * 100.0;
    let scale_fraction = (scale_percent / 200.0).clamp(0.0, 1.0) as f32;

    let content = div()
        .id("video-clip-property-panel-v3")
        .h_full()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(rgb(PANEL))
        .text_color(rgb(TEXT))
        .child(clip_property_panel_header_view(
            TrackKind::Video,
            asset_name.to_owned(),
            track_name,
            panel.locked,
        ))
        .child(
            div()
                .id("video-clip-property-panel-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p(px(16.0))
                .child(property_panel_heading_view("TIMING"))
                .child(
                    div()
                        .mt(px(12.0))
                        .grid()
                        .grid_cols(2)
                        .gap(px(8.0))
                        .child(timing_card_view(
                            "Start",
                            clip.timeline_start,
                            frame_rate,
                            panel.locked,
                        ))
                        .child(timing_card_view(
                            "Duration",
                            duration,
                            frame_rate,
                            panel.locked,
                        ))
                        .child(timing_card_view(
                            "Source in",
                            clip.source_in,
                            frame_rate,
                            panel.locked,
                        ))
                        .child(timing_card_view(
                            "Source out",
                            clip.source_out,
                            frame_rate,
                            panel.locked,
                        )),
                )
                .child(div().h(px(1.0)).my(px(16.0)).bg(rgb(0x1f1f24)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(property_panel_heading_view("TRANSFORM"))
                        .when(!panel.locked, |heading| {
                            heading.child(
                                div()
                                    .text_size(px(12.0))
                                    .line_height(px(16.0))
                                    .text_color(rgb(ACCENT))
                                    .child("Reset"),
                            )
                        }),
                )
                .child(
                    div()
                        .mt(px(12.0))
                        .text_size(px(13.0))
                        .text_color(rgb(MUTED))
                        .child("Position"),
                )
                .child(
                    div()
                        .mt(px(8.0))
                        .grid()
                        .grid_cols(2)
                        .gap(px(8.0))
                        .child(value_field_view(
                            "X",
                            trimmed_number(clip.video_properties.position_x),
                            "px",
                            panel.locked,
                        ))
                        .child(value_field_view(
                            "Y",
                            trimmed_number(clip.video_properties.position_y),
                            "px",
                            panel.locked,
                        )),
                )
                .child(
                    div()
                        .mt(px(16.0))
                        .flex()
                        .items_center()
                        .gap(px(12.0))
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_size(px(13.0))
                                .text_color(rgb(MUTED))
                                .child("Scale"),
                        )
                        .child(scale_slider_view(panel, scale_fraction))
                        .child(div().w(px(84.0)).flex_shrink_0().child(if panel.locked {
                            value_field_view("", trimmed_number(scale_percent), "%", true)
                                .into_any_element()
                        } else {
                            let event_bus = panel.event_bus.clone();
                            let clip_id = clip.id();
                            div()
                                .h(px(30.0))
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .px(px(9.0))
                                .rounded(px(6.0))
                                .border_1()
                                .border_color(rgb(BORDER))
                                .bg(rgb(SURFACE))
                                .child(
                                    div().flex_1().min_w_0().text_size(px(13.0)).child(
                                        TextInputV2::new(
                                            format!("clip-scale-{clip_id}"),
                                            trimmed_number(scale_percent),
                                            panel.return_focus.clone(),
                                        )
                                        .constraint(InputConstraint::Number)
                                        .on_text_change(
                                            move |text, _, cx| {
                                                let Ok(percent) = text.parse::<f64>() else {
                                                    return;
                                                };
                                                if !percent.is_finite()
                                                    || !(0.0..=10000.0).contains(&percent)
                                                {
                                                    return;
                                                }
                                                event_bus.update(cx, |_, cx| {
                                                    cx.emit(AppEvent::Edit(
                                                        EditAction::SetClipScale {
                                                            clip_id,
                                                            scale: percent / 100.0,
                                                        },
                                                    ))
                                                });
                                            },
                                        ),
                                    ),
                                )
                                .child(div().text_size(px(12.0)).text_color(rgb(MUTED)).child("%"))
                                .into_any_element()
                        })),
                )
                .child(div().h(px(1.0)).my(px(16.0)).bg(rgb(0x1f1f24)))
                .child(property_panel_heading_view("AUDIO"))
                .child(
                    audio_properties_view(
                        clip.id(),
                        &clip.audio_properties,
                        panel.locked,
                        panel.event_bus.clone(),
                        panel.return_focus.clone(),
                    )
                    .mt(px(12.0)),
                ),
        )
        .into_any_element();
    clip_property_panel_frame_view(content, panel.locked)
}

fn trimmed_number(value: f64) -> String {
    let formatted = format!("{value:.2}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

struct ScaleDrag;

fn scale_slider_view(panel: &VideoClipPanel, fraction: f32) -> impl IntoElement {
    let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
    let measured_bounds = bounds.clone();
    let drag_bounds = bounds.clone();
    let click_bus = panel.event_bus.clone();
    let drag_bus = panel.event_bus.clone();
    let clip_id = panel.clip.id();
    div()
        .id("clip-scale-slider")
        .relative()
        .flex_1()
        .min_w_0()
        .h(px(20.0))
        .flex()
        .items_center()
        .child(
            canvas(move |area, _, _| measured_bounds.set(area), |_, _, _, _| {})
                .absolute()
                .size_full(),
        )
        .child(slider_view(fraction, 0.5, panel.locked))
        .when(!panel.locked, |slider| {
            slider
                .cursor_pointer()
                .on_click(move |event, _, cx| {
                    ui_callback_scale(clip_id, event.position().x, bounds.get(), &click_bus, cx);
                })
                .on_drag(ScaleDrag, |_, _, _, cx| cx.new(|_| Empty))
                .on_drag_move::<ScaleDrag>(move |event: &DragMoveEvent<ScaleDrag>, _, cx| {
                    ui_callback_scale(
                        clip_id,
                        event.event.position.x,
                        drag_bounds.get(),
                        &drag_bus,
                        cx,
                    );
                })
        })
}

fn ui_callback_scale(
    clip_id: Ulid,
    x: Pixels,
    bounds: Bounds<Pixels>,
    event_bus: &Entity<EventBus>,
    cx: &mut gpui::App,
) {
    let width = f32::from(bounds.size.width);
    if width <= 0.0 {
        return;
    }
    let scale = f64::from((f32::from(x - bounds.origin.x) / width).clamp(0.0, 1.0)) * 2.0;
    event_bus.update(cx, |_, cx| {
        cx.emit(AppEvent::Edit(EditAction::SetClipScale { clip_id, scale }))
    });
}
