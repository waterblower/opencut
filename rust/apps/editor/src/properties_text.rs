use crate::properties::{
    format_properties_time, properties_divider_view, properties_header_view,
    properties_heading_view, readonly_properties_panel_view,
};
use crate::theme::{ACCENT, BORDER, MUTED, SURFACE};
use crate::timeline::FrameRate;
use crate::timeline_clip::TextClip;
use gpui::prelude::*;
use gpui::{FontWeight, div, px, relative, rgb, rgba};

pub fn text_clip_view(
    clip: &TextClip,
    track_name: &str,
    frame_rate: FrameRate,
) -> gpui::AnyElement {
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
    let header = properties_header_view(badge, title, format!("Text clip · {track_name}"));
    let alpha = clip.properties.color >> 24;
    let color_rgb = clip.properties.color & 0x00ff_ffff;
    let color_text = if alpha == 0xff {
        format!("#{color_rgb:06X}")
    } else {
        format!("#{color_rgb:06X}{alpha:02X}")
    };
    let position_diagram = div()
        .relative()
        .h(px(68.0))
        .min_w_0()
        .overflow_hidden()
        .rounded(px(4.0))
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(0x131316))
        .child(
            div()
                .absolute()
                .left(relative(0.5))
                .top(px(3.0))
                .bottom(px(3.0))
                .border_l_1()
                .border_dashed()
                .border_color(rgb(BORDER)),
        )
        .child(
            div()
                .absolute()
                .top(relative(0.5))
                .left(px(3.0))
                .right(px(3.0))
                .border_t_1()
                .border_dashed()
                .border_color(rgb(BORDER)),
        )
        .child(
            div()
                .absolute()
                .left(relative(clip.properties.position.x.clamp(0.0, 1.0) as f32))
                .top(relative(clip.properties.position.y.clamp(0.0, 1.0) as f32))
                .ml(px(-28.0))
                .mt(px(-6.5))
                .w(px(57.0))
                .h(px(13.0))
                .p(px(2.0))
                .rounded(px(3.0))
                .border_1()
                .border_color(rgb(ACCENT))
                .child(div().size_full().rounded(px(2.0)).bg(rgb(0xe8e8ec))),
        );
    let content = div()
        .child(properties_heading_view("CONTENT"))
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
        .child(properties_divider_view())
        .child(properties_heading_view("TIMING"))
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
                                        .child(format_properties_time(
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
        .child(properties_divider_view())
        .child(properties_heading_view("STYLE"))
        .child(
            div()
                .mt(px(12.0))
                .child(text_value_view("", clip.properties.font.clone(), "")),
        )
        .child(
            div()
                .mt(px(8.0))
                .grid()
                .grid_cols(2)
                .gap(px(8.0))
                .child(text_value_view(
                    "",
                    clip.properties.font_size.to_string(),
                    "px",
                ))
                .child(
                    div()
                        .h(px(30.0))
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
                                .text_size(px(11.0))
                                .text_ellipsis()
                                .child(color_text),
                        ),
                ),
        )
        .child(properties_divider_view())
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.0))
                .child(properties_heading_view("POSITION"))
                .child(
                    div()
                        .text_size(px(12.0))
                        .text_color(rgb(MUTED))
                        .child("Drag in preview"),
                ),
        )
        .child(
            div()
                .mt(px(12.0))
                .grid()
                .grid_cols(2)
                .gap(px(8.0))
                .child(position_diagram)
                .child(
                    div()
                        .min_w_0()
                        .flex()
                        .flex_col()
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
                ),
        );
    readonly_properties_panel_view("text-clip-properties", header, content)
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
