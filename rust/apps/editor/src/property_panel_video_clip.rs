use crate::property_panel::{
    clip_track_subtitle_view, property_panel_heading_view, slider_view, timing_card_view,
    value_field_view,
};
use crate::theme::{ACCENT, BORDER, CLIP_BLUE, MUTED, PANEL, TEXT};
use crate::timeline::FrameRate;
use crate::timeline_clip::VideoClip;
use crate::track::TrackKind;
use gpui::{
    FontWeight, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px, rgb,
};
use timeline::TimelineEditingState;

pub(crate) struct VideoClipPanel {
    clip: VideoClip,
    asset_name: String,
    track_name: String,
    frame_rate: FrameRate,
}

pub(crate) fn video_clip_panel(
    editing_state: &TimelineEditingState,
    clip: &VideoClip,
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

    div()
        .id("video-clip-property-panel-v3")
        .h_full()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(rgb(PANEL))
        .text_color(rgb(TEXT))
        .child(
            div()
                .h(px(64.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap(px(12.0))
                .px(px(16.0))
                .border_b_1()
                .border_color(rgb(BORDER))
                .child(
                    div()
                        .size(px(32.0))
                        .flex_shrink_0()
                        .rounded(px(8.0))
                        .border_1()
                        .border_color(rgb(0x395d85))
                        .bg(rgb(CLIP_BLUE))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(rgb(0xffffff))
                        .child("V"),
                )
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(px(14.0))
                                .line_height(px(20.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_ellipsis()
                                .child(asset_name.to_owned()),
                        )
                        .child(clip_track_subtitle_view(TrackKind::Video, track_name)),
                ),
        )
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
                        .child(timing_card_view("Start", clip.timeline_start, frame_rate))
                        .child(timing_card_view("Duration", duration, frame_rate))
                        .child(timing_card_view("Source in", clip.source_in, frame_rate))
                        .child(timing_card_view("Source out", clip.source_out, frame_rate)),
                )
                .child(div().h(px(1.0)).my(px(16.0)).bg(rgb(0x1f1f24)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(property_panel_heading_view("TRANSFORM"))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .line_height(px(16.0))
                                .text_color(rgb(ACCENT))
                                .child("Reset"),
                        ),
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
                        ))
                        .child(value_field_view(
                            "Y",
                            trimmed_number(clip.video_properties.position_y),
                            "px",
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
                        .child(slider_view(scale_fraction, 0.5))
                        .child(div().w(px(84.0)).flex_shrink_0().child(value_field_view(
                            "",
                            trimmed_number(scale_percent),
                            "%",
                        ))),
                ),
        )
        .into_any_element()
}

fn trimmed_number(value: f64) -> String {
    let formatted = format!("{value:.2}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}
