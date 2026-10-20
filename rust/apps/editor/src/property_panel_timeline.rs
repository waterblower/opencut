use crate::property_panel::{
    file_location_view, file_property_row_view, property_panel_divider_view,
    property_panel_header_view, property_panel_heading_view, readonly_property_panel_view,
};
use crate::theme::{ACCENT, CLIP_BLUE, MUTED};
use crate::time_format::format_time;
use crate::timeline::{FrameRateLabel, TimelineRuntimeState};
use crate::track::TrackKind;
use gpui::{FontWeight, ParentElement, Styled, div, px, rgb};
use std::path::{Path, PathBuf};
use timeline::TimelineSettings;

pub(crate) struct TimelinePanel {
    path: PathBuf,         // 绝对路径
    project_root: PathBuf, // 用于显示相对路径
    settings: TimelineSettings,
    duration_seconds: f64,
    text_track_count: usize,
    video_track_count: usize,
    audio_track_count: usize,
    clip_count: usize,
}

pub(super) fn timeline_panel(
    timeline: &TimelineRuntimeState,
    project_root: &Path,
) -> TimelinePanel {
    let settings = timeline.editing_state.settings;
    let track_count = |kind: TrackKind| {
        timeline
            .editing_state
            .tracks
            .iter()
            .filter(|track| track.kind == kind)
            .count()
    };
    TimelinePanel {
        path: timeline.path.clone(),
        project_root: project_root.to_path_buf(),
        settings,
        duration_seconds: settings
            .frame_rate
            .seconds(timeline.editing_state.content_duration()),
        text_track_count: track_count(TrackKind::Text),
        video_track_count: track_count(TrackKind::Video),
        audio_track_count: track_count(TrackKind::Audio),
        clip_count: timeline.editing_state.clips.len(),
    }
}

pub(super) fn timeline_view(panel: &TimelinePanel) -> gpui::AnyElement {
    let name = match panel.path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => panel.path.display().to_string(),
    };
    let relative_path = match panel.path.strip_prefix(&panel.project_root) {
        Ok(relative_path) => relative_path.display().to_string(),
        Err(_) => panel.path.display().to_string(),
    };
    let badge = div()
        .size(px(32.0))
        .flex_shrink_0()
        .rounded(px(8.0))
        .border_1()
        .border_color(rgb(ACCENT))
        .bg(rgb(ACCENT))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0x17120a))
        .child("TL");
    let header = property_panel_header_view(badge, name, relative_path);
    let content = div()
        .child(property_panel_heading_view("DURATION"))
        .child(
            div()
                .mt(px(6.0))
                .font_family("monospace")
                .text_size(px(24.0))
                .line_height(px(30.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(format_time(panel.duration_seconds, false)),
        )
        .child(property_panel_divider_view())
        .child(property_panel_heading_view("FORMAT"))
        .child(
            div()
                .mt(px(12.0))
                .child(file_property_row_view(
                    "Frame rate",
                    panel.settings.frame_rate.label(),
                ))
                .child(file_property_row_view(
                    "Resolution",
                    format!("{} × {}", panel.settings.width, panel.settings.height),
                ))
                .child(file_property_row_view(
                    "Audio rate",
                    format!("{} kHz", panel.settings.audio_sample_rate as f64 / 1000.0),
                )),
        )
        .child(property_panel_divider_view())
        .child(property_panel_heading_view("CONTENTS"))
        .child(
            div()
                .mt(px(12.0))
                .child(file_property_row_view(
                    "Clips",
                    panel.clip_count.to_string(),
                ))
                .child(
                    div()
                        .min_h(px(28.0))
                        .flex()
                        .items_center()
                        .gap(px(8.0))
                        .text_size(px(13.0))
                        .child(div().flex_1().text_color(rgb(MUTED)).child("Tracks"))
                        .child(track_count_badge_view(
                            "T",
                            panel.text_track_count,
                            0x7251a3,
                            0x8261b3,
                        ))
                        .child(track_count_badge_view(
                            "V",
                            panel.video_track_count,
                            CLIP_BLUE,
                            0x395d85,
                        ))
                        .child(track_count_badge_view(
                            "A",
                            panel.audio_track_count,
                            0x24656b,
                            0x34757b,
                        )),
                ),
        )
        .child(property_panel_divider_view())
        .child(property_panel_heading_view("LOCATION"))
        .child(file_location_view(&panel.path));
    readonly_property_panel_view("timeline-property-panel", header, content)
}

fn track_count_badge_view(
    label: &'static str,
    count: usize,
    background: u32,
    border: u32,
) -> gpui::Div {
    div()
        .h(px(19.0))
        .min_w(px(33.0))
        .px(px(5.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(4.0))
        .border_1()
        .border_color(rgb(border))
        .bg(rgb(background))
        .text_size(px(11.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(0xffffff))
        .child(format!("{label} {count}"))
}
