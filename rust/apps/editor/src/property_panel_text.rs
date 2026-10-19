use crate::property_panel::{
    clip_track_subtitle_view, format_property_panel_time, property_panel_divider_view,
    property_panel_header_view, property_panel_heading_view, readonly_property_panel_view,
};
use crate::theme::{BORDER, MUTED, SURFACE};
use crate::timeline::FrameRate;
use crate::timeline_clip::TextClip;
use crate::track::TrackKind;
use gpui::prelude::*;
use gpui::{FontWeight, PathBuilder, canvas, div, point, px, rgb, rgba};
use timeline::TimelineEditingState;

pub(crate) struct TextClipPanel {
    clip: TextClip,
    track_name: String,
    frame_rate: FrameRate,
}

pub(crate) fn text_clip_panel(
    editing_state: &TimelineEditingState,
    clip: &TextClip,
) -> TextClipPanel {
    let track_name = match editing_state
        .tracks
        .iter()
        .find(|track| track.id == clip.track_id)
    {
        Some(track) => track.name.as_str(),
        None => "Unknown track",
    };
    TextClipPanel {
        clip: clip.clone(),
        track_name: track_name.to_owned(),
        frame_rate: editing_state.settings.frame_rate,
    }
}

pub(crate) fn text_clip_view(panel: &TextClipPanel) -> gpui::AnyElement {
    let clip = &panel.clip;
    let track_name = panel.track_name.as_str();
    let frame_rate = panel.frame_rate;
    let title = match clip
        .properties
        .text
        .lines()
        .find(|line| !line.trim().is_empty())
    {
        Some(line) => line.to_owned(),
        None => "Text clip".to_owned(),
    };
    let badge = div()
        .size(px(32.0))
        .flex_shrink_0()
        .rounded(px(8.0))
        .border_1()
        .border_color(rgb(0x8261b3))
        .bg(rgb(0x7251a3))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0xffffff))
        .child("T");
    let header = property_panel_header_view(
        badge,
        title,
        clip_track_subtitle_view(TrackKind::Text, track_name),
    );
    let alpha = clip.properties.color >> 24;
    let color_rgb = clip.properties.color & 0x00ff_ffff;
    let color_text = if alpha == 0xff {
        format!("#{color_rgb:06X}")
    } else {
        format!("#{color_rgb:06X}{alpha:02X}")
    };
    let content = div()
        .child(property_panel_heading_view("CONTENT"))
        .child(
            div()
                .mt(px(12.0))
                .min_h(px(58.0))
                .px(px(10.0))
                .py(px(8.0))
                .rounded(px(6.0))
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(SURFACE))
                .text_size(px(13.0))
                .line_height(px(19.0))
                .children(
                    clip.properties
                        .text
                        .lines()
                        .map(|line| div().min_h(px(19.0)).child(line.to_owned())),
                ),
        )
        .child(property_panel_divider_view())
        .child(property_panel_heading_view("TIMING"))
        .child(
            div()
                .mt(px(8.0))
                .grid()
                .grid_cols(2)
                .gap(px(8.0))
                .child(
                    div()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(11.0))
                                .line_height(px(16.0))
                                .text_color(rgb(MUTED))
                                .child("Start"),
                        )
                        .child(
                            div()
                                .mt(px(4.0))
                                .min_h(px(30.0))
                                .px(px(10.0))
                                .py(px(6.0))
                                .rounded(px(6.0))
                                .bg(rgb(0x131316))
                                .flex()
                                .flex_wrap()
                                .items_baseline()
                                .justify_between()
                                .gap_x(px(4.0))
                                .font_family("monospace")
                                .child(
                                    div()
                                        .min_w_0()
                                        .text_size(px(13.0))
                                        .line_height(px(18.0))
                                        .text_ellipsis()
                                        .child(format_property_panel_time(
                                            frame_rate.seconds(clip.timeline_start),
                                        )),
                                )
                                .child(
                                    div()
                                        .min_w_0()
                                        .text_size(px(11.0))
                                        .line_height(px(18.0))
                                        .text_color(rgb(0x55555e))
                                        .text_ellipsis()
                                        .child(format!("{} f", i64::from(clip.timeline_start))),
                                ),
                        ),
                )
                .child(
                    div()
                        .min_w_0()
                        .child(
                            div()
                                .text_size(px(11.0))
                                .line_height(px(16.0))
                                .text_color(rgb(MUTED))
                                .child("Duration"),
                        )
                        .child(div().mt(px(4.0)).child(text_value_view(
                            "",
                            format!("{:.2}", clip.duration.as_secs_f64()),
                            "s",
                        ))),
                ),
        )
        .child(property_panel_divider_view())
        .child(property_panel_heading_view("STYLE"))
        .child(
            div().mt(px(12.0)).child(style_field_view(
                "Font",
                div()
                    .h(px(34.0))
                    .min_w_0()
                    .flex()
                    .items_center()
                    .gap(px(8.0))
                    .px(px(10.0))
                    .rounded(px(6.0))
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(SURFACE))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(px(13.0))
                            .text_ellipsis()
                            .child(clip.properties.font.clone()),
                    )
                    .child(
                        canvas(
                            |_, _, _| (),
                            |bounds, (), window, _| {
                                let mut chevron = PathBuilder::stroke(px(1.5));
                                chevron.move_to(bounds.origin + point(px(2.0), px(4.0)));
                                chevron.line_to(bounds.origin + point(px(6.0), px(8.0)));
                                chevron.line_to(bounds.origin + point(px(10.0), px(4.0)));
                                if let Ok(path) = chevron.build() {
                                    window.paint_path(path, rgb(0xb4b4bc));
                                }
                            },
                        )
                        .size(px(12.0))
                        .flex_shrink_0(),
                    ),
            )),
        )
        .child(
            div()
                .mt(px(12.0))
                .grid()
                .grid_cols(2)
                .gap(px(8.0))
                .child(style_field_view(
                    "Font size",
                    text_value_view("", clip.properties.font_size.to_string(), "px").h(px(34.0)),
                ))
                .child(style_field_view(
                    "Font color",
                    div()
                        .h(px(34.0))
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .px(px(8.0))
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .bg(rgb(SURFACE))
                        .child(
                            div()
                                .size(px(16.0))
                                .flex_shrink_0()
                                .rounded(px(4.0))
                                .border_1()
                                .border_color(rgb(BORDER))
                                .bg(rgba(clip.properties.color.rotate_left(8))), // ARGB → RGBA。
                        )
                        .child(
                            div()
                                .min_w_0()
                                .font_family("monospace")
                                .text_size(px(13.0))
                                .text_ellipsis()
                                .child(color_text),
                        ),
                )),
        )
        .child(property_panel_divider_view())
        .child(property_panel_heading_view("POSITION"))
        .child(
            div()
                .mt(px(12.0))
                .grid()
                .grid_cols(2)
                .gap(px(8.0))
                .child(text_value_view(
                    "X",
                    format!("{:.2}", clip.properties.position.x),
                    "",
                ))
                .child(text_value_view(
                    "Y",
                    format!("{:.2}", clip.properties.position.y),
                    "",
                )),
        );
    readonly_property_panel_view("text-clip-property-panel", header, content)
}

fn style_field_view(label: &'static str, value: gpui::Div) -> gpui::Div {
    div()
        .min_w_0()
        .child(
            div()
                .text_size(px(12.0))
                .line_height(px(16.0))
                .text_color(rgb(0xb4b4bc))
                .child(label),
        )
        .child(div().mt(px(8.0)).child(value))
}

fn text_value_view(label: &'static str, value: String, unit: &'static str) -> gpui::Div {
    div()
        .h(px(30.0))
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(6.0))
        .px(px(10.0))
        .rounded(px(6.0))
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(SURFACE))
        .text_size(px(12.0))
        .when(!label.is_empty(), |field| {
            field.child(
                div()
                    .flex_shrink_0()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(MUTED))
                    .child(label),
            )
        })
        .child(
            div()
                .min_w_0()
                .flex_1()
                .font_family("monospace")
                .text_size(px(13.0))
                .text_ellipsis()
                .child(value),
        )
        .when(!unit.is_empty(), |field| {
            field.child(div().flex_shrink_0().text_color(rgb(MUTED)).child(unit))
        })
}
