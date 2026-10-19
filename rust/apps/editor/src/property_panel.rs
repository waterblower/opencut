use crate::editor::Editor;
use crate::explorer_file_entry::{is_audio_path, is_image_path, is_srt_path, is_video_path};
use crate::property_panel_audio_clip::{AudioClipPanel, audio_clip_panel, audio_clip_view};
use crate::property_panel_audio_file::{AudioFilePanel, audio_file_view};
use crate::property_panel_empty::empty_panel_view;
use crate::property_panel_image_file::{ImageFilePanel, image_file_view};
use crate::property_panel_srt_file::{SrtFilePanel, srt_file_panel, srt_file_view};
use crate::property_panel_text::{TextClipPanel, text_clip_panel, text_clip_view};
use crate::property_panel_timeline::{TimelinePanel, timeline_panel, timeline_view};
use crate::property_panel_transform::VideoTransformInputs;
use crate::property_panel_video_clip::{VideoClipPanel, video_clip_panel, video_clip_view};
use crate::property_panel_video_file::{VideoFilePanel, video_file_view};
use crate::theme::{ACCENT, BORDER, MUTED, PANEL, SURFACE, TEXT};
use crate::timeline::{FrameRate, TimelineFrameIndex};
use crate::timeline_clip::Clip;
use crate::timeline_document;
use crate::track::TrackKind;
use gpui::prelude::FluentBuilder;
use gpui::{
    FontWeight, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement, Styled,
    div, px, relative, rgb,
};
use std::path::Path;
use ulid::Ulid;

pub(crate) enum PropertyPanel {
    VideoFile(VideoFilePanel),
    AudioFile(AudioFilePanel),
    ImageFile(ImageFilePanel),
    SrtFile(SrtFilePanel),
    VideoClip(VideoClipPanel),
    AudioClip(AudioClipPanel),
    TextClip(TextClipPanel),
    Timeline(TimelinePanel),
    None,
}

pub(crate) struct PropertyPanelState {
    pub(crate) transform_inputs: VideoTransformInputs,
    pub(crate) transform_input_clip_id: Option<Ulid>,
    pub(crate) text_input_clip_id: Option<Ulid>,
}

pub(crate) fn current_property_panel(editor: &Editor) -> PropertyPanel {
    if let Some(timeline) = editor.timeline.as_ref()
        && let Some(clip_id) = timeline.interaction.selected_clip_id
    {
        let Some(clip) = timeline.editing_state.clip(clip_id) else {
            return PropertyPanel::None;
        };
        return match clip {
            Clip::Video(clip) => {
                PropertyPanel::VideoClip(video_clip_panel(&timeline.editing_state, clip))
            }
            Clip::Audio(clip) => {
                PropertyPanel::AudioClip(audio_clip_panel(timeline, clip, &editor.waveform_cache))
            }
            Clip::Text(clip) => {
                PropertyPanel::TextClip(text_clip_panel(&timeline.editing_state, clip))
            }
        };
    }

    if let Some(path) = editor.explorer.selected_file.as_deref() {
        let absolute_path = editor.project_root.join(path);
        let size_bytes = match editor
            .explorer
            .file_tree
            .iter()
            .chain(editor.explorer.search_results.iter())
            .find(|entry| entry.absolute_path == absolute_path)
        {
            Some(entry) => entry.size_bytes,
            None => None,
        };
        if is_video_path(path) {
            return PropertyPanel::VideoFile(VideoFilePanel {
                path: absolute_path,
                size_bytes,
            });
        }
        if is_audio_path(path) {
            return PropertyPanel::AudioFile(AudioFilePanel {
                path: absolute_path,
                size_bytes,
            });
        }
        if is_image_path(path) {
            return PropertyPanel::ImageFile(ImageFilePanel {
                path: absolute_path,
                size_bytes,
            });
        }
        if is_srt_path(path) {
            return PropertyPanel::SrtFile(srt_file_panel(absolute_path));
        }
        if timeline_document::is_timeline_path(path) {
            let timeline = editor
                .timeline
                .as_ref()
                .filter(|timeline| timeline.path == absolute_path)
                .expect("should have the timeline");
            return PropertyPanel::Timeline(timeline_panel(timeline, &editor.project_root));
        }
        return PropertyPanel::None;
    }

    let Some(timeline) = editor.timeline.as_ref() else {
        return PropertyPanel::None;
    };
    PropertyPanel::Timeline(timeline_panel(timeline, &editor.project_root))
}

pub(super) fn property_panel_view(panel: &PropertyPanel) -> gpui::AnyElement {
    match panel {
        PropertyPanel::TextClip(panel) => text_clip_view(panel),
        PropertyPanel::VideoClip(panel) => video_clip_view(panel),
        PropertyPanel::AudioClip(panel) => audio_clip_view(panel),
        PropertyPanel::VideoFile(panel) => video_file_view(panel),
        PropertyPanel::Timeline(panel) => timeline_view(panel),
        PropertyPanel::ImageFile(panel) => image_file_view(panel),
        PropertyPanel::AudioFile(panel) => audio_file_view(panel),
        PropertyPanel::SrtFile(panel) => srt_file_view(panel),
        PropertyPanel::None => empty_panel_view(),
    }
}

pub(crate) fn property_panel_header_view(
    badge: gpui::Div,
    title: String,
    subtitle: impl IntoElement,
) -> gpui::Div {
    div()
        .h(px(64.0))
        .flex_shrink_0()
        .flex()
        .items_center()
        .gap(px(12.0))
        .px(px(16.0))
        .border_b_1()
        .border_color(rgb(BORDER))
        .child(badge)
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
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .line_height(px(16.0))
                        .text_color(rgb(MUTED))
                        .text_ellipsis()
                        .child(subtitle),
                ),
        )
}

pub(crate) fn clip_track_subtitle_view(kind: TrackKind, track_name: &str) -> gpui::Div {
    let (label, color) = match kind {
        TrackKind::Text => ("Text clip · Track:", 0xb69bdc),
        TrackKind::Video => ("Video clip · Track:", 0x91b8df),
        TrackKind::Audio => ("Audio clip · Track:", 0x69c5cf),
    };
    div()
        .min_w_0()
        .flex()
        .gap(px(3.0))
        .text_size(px(11.0))
        .line_height(px(16.0))
        .child(div().flex_shrink_0().text_color(rgb(0x8a8a93)).child(label))
        .child(
            div()
                .min_w_0()
                .text_ellipsis()
                .text_color(rgb(color))
                .child(track_name.to_owned()),
        )
}

pub(crate) fn readonly_property_panel_view(
    id: &'static str,
    header: gpui::Div,
    content: gpui::Div,
) -> gpui::AnyElement {
    div()
        .id(id)
        .h_full()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(rgb(PANEL))
        .text_color(rgb(TEXT))
        .child(header)
        .child(
            div()
                .id(format!("{id}-scroll"))
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p(px(16.0))
                .child(content),
        )
        .into_any_element()
}

pub(crate) fn property_panel_heading_view(label: &'static str) -> gpui::Div {
    div()
        .text_size(px(11.0))
        .line_height(px(16.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(MUTED))
        .child(label)
}

pub(crate) fn property_panel_divider_view() -> gpui::Div {
    div().h(px(1.0)).my(px(16.0)).bg(rgb(0x1f1f24))
}

pub(crate) fn format_property_panel_time(seconds: f64) -> String {
    let centiseconds = (seconds.max(0.0) * 100.0).round() as u64;
    format!(
        "{}:{:02}.{:02}",
        centiseconds / 6000,
        (centiseconds / 100) % 60,
        centiseconds % 100,
    )
}

pub(super) fn timing_card_view(
    label: &'static str,
    frame_index: TimelineFrameIndex,
    frame_rate: FrameRate,
) -> gpui::Div {
    let time = format_property_panel_time(frame_rate.seconds(frame_index));
    div()
        .min_w_0()
        .min_h(px(50.0))
        .px(px(10.0))
        .py(px(8.0))
        .rounded(px(6.0))
        .bg(rgb(0x131316))
        .child(
            div()
                .text_size(px(11.0))
                .line_height(px(16.0))
                .text_color(rgb(MUTED))
                .child(label),
        )
        .child(
            div()
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
                        .child(time),
                )
                .child(
                    div()
                        .min_w_0()
                        .text_size(px(11.0))
                        .line_height(px(18.0))
                        .text_color(rgb(0x55555e))
                        .text_ellipsis()
                        .child(format!("{} f", i64::from(frame_index))),
                ),
        )
}

pub(super) fn slider_view(fraction: f32, reference_fraction: f32) -> gpui::Div {
    div()
        .relative()
        .flex_1()
        .min_w_0()
        .h(px(4.0))
        .rounded_full()
        .bg(rgb(BORDER))
        .child(
            div()
                .h_full()
                .w(relative(fraction))
                .rounded_full()
                .bg(rgb(ACCENT)),
        )
        .child(
            div()
                .absolute()
                .left(relative(reference_fraction))
                .top(px(-3.0))
                .w(px(1.0))
                .h(px(10.0))
                .bg(rgb(0x55555e)),
        )
        .child(
            div()
                .absolute()
                .left(relative(fraction))
                .ml(px(-7.0))
                .top(px(-5.0))
                .size(px(14.0))
                .rounded_full()
                .bg(rgb(TEXT))
                .border_2()
                .border_color(rgb(PANEL)),
        )
}

pub(super) fn value_field_view(
    label: &'static str,
    value: String,
    unit: &'static str,
) -> gpui::Div {
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
        .child(div().flex_shrink_0().text_color(rgb(MUTED)).child(unit))
}

pub(super) fn file_details_view(path: &Path, size_bytes: Option<u64>) -> gpui::Div {
    let format = match path.extension() {
        Some(extension) => extension.to_string_lossy().to_ascii_uppercase(),
        None => "Unknown".to_owned(),
    };
    let size = match size_bytes {
        Some(bytes) => {
            if bytes >= 1_000_000_000 {
                format!("{:.1} GB", bytes as f64 / 1_000_000_000.0)
            } else if bytes >= 1_000_000 {
                format!("{:.1} MB", bytes as f64 / 1_000_000.0)
            } else if bytes >= 1_000 {
                format!("{:.1} KB", bytes as f64 / 1_000.0)
            } else {
                format!("{bytes} B")
            }
        }
        None => "Unavailable".to_owned(),
    };
    div()
        .mt(px(12.0))
        .child(file_property_row_view("Format", format))
        .child(file_property_row_view("Size", size))
}

pub(super) fn file_location_view(path: &Path) -> gpui::Div {
    div()
        .mt(px(8.0))
        .font_family("monospace")
        .text_size(px(12.0))
        .line_height(px(18.0))
        .text_color(rgb(0xb4b4bc))
        .whitespace_normal()
        .child(path.display().to_string().replace('/', "/\u{200b}")) // 零宽空格让长路径在斜杠处换行
}

pub(super) fn file_hint_view(text: &'static str) -> gpui::Div {
    div()
        .text_size(px(12.0))
        .line_height(px(17.0))
        .text_color(rgb(MUTED))
        .child(text)
}

pub(super) fn file_badge_view(label: &str, border: u32, foreground: u32) -> gpui::Div {
    div()
        .size(px(32.0))
        .flex_shrink_0()
        .rounded(px(8.0))
        .border_1()
        .border_color(rgb(border))
        .bg(rgb(0x131316))
        .flex()
        .items_center()
        .justify_center()
        .font_family("monospace")
        .text_size(px(9.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(foreground))
        .child(label.to_owned())
}

pub(super) fn file_property_row_view(label: &'static str, value: String) -> gpui::Div {
    div()
        .min_h(px(28.0))
        .flex()
        .items_center()
        .gap(px(8.0))
        .text_size(px(13.0))
        .child(div().flex_shrink_0().text_color(rgb(MUTED)).child(label))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .text_right()
                .font_family("monospace")
                .text_ellipsis()
                .child(value),
        )
}
