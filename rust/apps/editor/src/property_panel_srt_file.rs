use crate::property_panel::{
    file_badge_view, format_property_panel_time, property_panel_header_view,
    property_panel_heading_view, readonly_property_panel_view,
};
use crate::theme::{ERROR, MUTED};
use crate::time_format::format_time;
use anyhow::Result;
use gpui::{ParentElement, Styled, div, px, rgb};
use std::fs;
use std::path::PathBuf;
use transcribe::SRT;

pub(crate) struct SrtFilePanel {
    path: PathBuf,          // 绝对路径
    subtitles: Result<SRT>, // 构造面板时读取并解析
}

/// Reads and parses the subtitles; failures are logged here and shown by the panel.
pub(crate) fn srt_file_panel(path: PathBuf) -> SrtFilePanel {
    let subtitles = (|| -> Result<SRT> {
        let text = fs::read_to_string(&path)?;
        SRT::from_string(&text)
    })();
    match &subtitles {
        Ok(_) => {
            // do nothing
        }
        Err(error) => log::error!("Could not load subtitles {}: {error:?}", path.display()),
    }
    SrtFilePanel { path, subtitles }
}

pub(super) fn srt_file_view(panel: &SrtFilePanel) -> gpui::AnyElement {
    let name = match panel.path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => panel.path.display().to_string(),
    };
    let mut content = div().child(property_panel_heading_view("CUES"));
    let subtitle = match &panel.subtitles {
        Ok(srt) => {
            let end = srt
                .subtitles
                .iter()
                .map(|cue| cue.end)
                .max()
                .unwrap_or_default();
            let summary = format!(
                "Subtitles · {} cues · {}",
                srt.subtitles.len(),
                format_time(end.as_secs_f64(), false),
            );
            if srt.subtitles.is_empty() {
                content = content.child(
                    div()
                        .mt(px(12.0))
                        .text_size(px(13.0))
                        .text_color(rgb(MUTED))
                        .child("No subtitle cues."),
                );
            }
            for (index, cue) in srt.subtitles.iter().enumerate() {
                content = content.child(
                    div()
                        .mt(px(12.0))
                        .flex()
                        .items_start()
                        .gap(px(8.0))
                        .child(
                            div()
                                .min_w(px(18.0))
                                .flex_shrink_0()
                                .font_family("monospace")
                                .text_size(px(11.0))
                                .line_height(px(16.0))
                                .text_color(rgb(0x55555e))
                                .child(format!("{:02}", index + 1)),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .pb(px(12.0))
                                .border_b_1()
                                .border_color(rgb(0x1f1f24))
                                .child(
                                    div()
                                        .font_family("monospace")
                                        .text_size(px(11.0))
                                        .line_height(px(16.0))
                                        .text_color(rgb(MUTED))
                                        .child(format!(
                                            "{} → {}",
                                            format_property_panel_time(cue.start.as_secs_f64()),
                                            format_property_panel_time(cue.end.as_secs_f64()),
                                        )),
                                )
                                .child(
                                    div()
                                        .mt(px(4.0))
                                        .text_size(px(13.0))
                                        .line_height(px(19.0))
                                        .children(cue.text.lines().map(|line| {
                                            div().min_h(px(19.0)).child(line.to_owned())
                                        })),
                                ),
                        ),
                );
            }
            content = content.child(
                div()
                    .mt(px(20.0))
                    .text_size(px(12.0))
                    .line_height(px(17.0))
                    .text_color(rgb(MUTED))
                    .child("Drag onto a text track to add one text clip per cue."),
            );
            summary
        }
        Err(_error) => {
            content = content.child(
                div()
                    .mt(px(12.0))
                    .text_size(px(13.0))
                    .line_height(px(19.0))
                    .text_color(rgb(ERROR))
                    .child("Could not load subtitles. Check that the file is readable and contains valid SRT cues."),
            );
            "Subtitles · Unavailable".to_owned()
        }
    };
    let header =
        property_panel_header_view(file_badge_view("SRT", 0x46464e, 0x8b8b94), name, subtitle);
    readonly_property_panel_view("srt-file-property-panel", header, content)
}
