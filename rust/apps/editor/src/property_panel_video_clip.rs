use crate::event_bus::EventBus;
use crate::property_panel::{
    audio_properties_view, clip_property_panel_frame_view, clip_property_panel_header_view,
    property_panel_heading_view, slider_view, timing_card_view, value_field_view,
};
use crate::theme::{ACCENT, MUTED, PANEL, TEXT};
use crate::timeline::FrameRate;
use crate::timeline_clip::VideoClip;
use crate::track::TrackKind;
use gpui::prelude::FluentBuilder;
use gpui::{
    Entity, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px, rgb,
};
use timeline::TimelineEditingState;

pub(crate) struct VideoClipPanel {
    clip: VideoClip,
    asset_name: String,
    track_name: String,
    frame_rate: FrameRate,
    event_bus: Entity<EventBus>,
    locked: bool, // 所在轨道的锁定状态，构造面板时读取。
}

pub(crate) fn video_clip_panel(
    editing_state: &TimelineEditingState,
    clip: &VideoClip,
    event_bus: Entity<EventBus>,
) -> VideoClipPanel {
    let asset_name = match editing_state.asset(clip.asset_id) {
        Some(asset) => asset.name.as_str(),
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
                        .child(slider_view(scale_fraction, 0.5, panel.locked))
                        .child(div().w(px(84.0)).flex_shrink_0().child(value_field_view(
                            "",
                            trimmed_number(scale_percent),
                            "%",
                            panel.locked,
                        ))),
                )
                .child(div().h(px(1.0)).my(px(16.0)).bg(rgb(0x1f1f24)))
                .child(property_panel_heading_view("AUDIO"))
                .child(
                    audio_properties_view(
                        clip.id(),
                        &clip.audio_properties,
                        panel.locked,
                        panel.event_bus.clone(),
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
