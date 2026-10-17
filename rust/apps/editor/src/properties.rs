use crate::editor::Editor;
use crate::event_bus::EventBus;
use crate::explorer_file_entry::{is_audio_path, is_image_path, is_srt_path, is_video_path};
use crate::properties_text::TextClipPropertiesView;
use crate::properties_transform::{VideoTransformInputs, properties_section_label, properties_tab};
use crate::theme::{ACCENT, BORDER, CLIP_BLUE, MUTED, PANEL, SURFACE, TEXT};
use crate::time_format::format_time;
use crate::timeline::{FrameRate, FrameRateLabel, TimelineFrameIndex, TimelineRuntimeState};
use crate::timeline_clip::{AudioClip, Clip, TextClip, VideoClip};
use crate::timeline_document;
use gpui::prelude::FluentBuilder;
use gpui::{
    Entity, FocusHandle, FontWeight, InteractiveElement, IntoElement, ObjectFit, ParentElement,
    StatefulInteractiveElement, Styled, StyledImage, div, img, px, relative, rgb, rgba,
};
use std::fs;
use std::path::{Path, PathBuf};
use ulid::Ulid;

pub(super) enum PropertiesPanelViewable<'a> {
    VideoClip {
        clip: &'a VideoClip,
        asset_name: &'a str,
        track_name: &'a str,
        frame_rate: FrameRate,
    },
    AudioClip(&'a AudioClip),
    TextClip(&'a TextClip),
    VideoFile(&'a Path),
    AudioFile(&'a Path),
    ImageFile(PathBuf),
    SrtFile(PathBuf),
    TimelineFile(&'a TimelineRuntimeState),
    None,
}

pub fn current_properties_panel_viewable(editor: &Editor) -> PropertiesPanelViewable<'_> {
    if let Some(timeline) = editor.timeline.as_ref()
        && let Some(clip_id) = timeline.interaction.selected_clip_id
    {
        let Some(clip) = timeline.editing_state.clip(clip_id) else {
            return PropertiesPanelViewable::None;
        };
        return match clip {
            Clip::Video(clip) => {
                let asset_name = match timeline.editing_state.asset(clip.asset_id) {
                    Some(asset) => asset.name.as_str(),
                    None => "Missing media",
                };
                let track_name = match timeline
                    .editing_state
                    .tracks
                    .iter()
                    .find(|track| track.id == clip.track_id)
                {
                    Some(track) => track.name.as_str(),
                    None => "Unknown track",
                };
                PropertiesPanelViewable::VideoClip {
                    clip,
                    asset_name,
                    track_name,
                    frame_rate: timeline.editing_state.settings.frame_rate,
                }
            }
            Clip::Audio(clip) => PropertiesPanelViewable::AudioClip(clip),
            Clip::Text(clip) => PropertiesPanelViewable::TextClip(clip),
        };
    }

    if let Some(path) = editor.explorer.selected_file.as_deref() {
        if is_video_path(path) {
            return PropertiesPanelViewable::VideoFile(path);
        }
        if is_audio_path(path) {
            return PropertiesPanelViewable::AudioFile(path);
        }
        if is_image_path(path) {
            return PropertiesPanelViewable::ImageFile(editor.project_root.join(path));
        }
        if is_srt_path(path) {
            return PropertiesPanelViewable::SrtFile(editor.project_root.join(path));
        }
        if timeline_document::is_timeline_path(path) {
            let timeline = editor
                .timeline
                .as_ref()
                .filter(|timeline| timeline.path == editor.project_root.join(path))
                .expect("should have the timeline");
            return PropertiesPanelViewable::TimelineFile(timeline);
        }
        return PropertiesPanelViewable::None;
    }

    let Some(timeline) = editor.timeline.as_ref() else {
        return PropertiesPanelViewable::None;
    };
    PropertiesPanelViewable::TimelineFile(timeline)
}

pub(crate) struct PropertiesPanelState {
    pub(crate) transform_inputs: VideoTransformInputs,
    pub(crate) transform_input_clip_id: Option<Ulid>,
    pub(crate) text_input_clip_id: Option<Ulid>,
}

pub(super) fn properties_panel(
    data: PropertiesPanelViewable<'_>,
    event_bus: Entity<EventBus>,
    return_focus: FocusHandle,
) -> gpui::AnyElement {
    match data {
        PropertiesPanelViewable::TextClip(clip) => {
            TextClipPropertiesView::new(clip.clone(), event_bus, return_focus).into_any_element()
        }
        PropertiesPanelViewable::VideoClip {
            clip,
            asset_name,
            track_name,
            frame_rate,
        } => video_clip_view(clip, asset_name, track_name, frame_rate),
        PropertiesPanelViewable::AudioClip(clip) => audio_clip(clip),
        PropertiesPanelViewable::VideoFile(file) => media_file(file, "Video"),
        PropertiesPanelViewable::TimelineFile(timeline) => timeline_file(timeline),
        PropertiesPanelViewable::ImageFile(path) => image_file(path),
        PropertiesPanelViewable::AudioFile(path) => media_file(path, "Audio"),
        PropertiesPanelViewable::SrtFile(path) => srt_file_view(&path),
        PropertiesPanelViewable::None => div()
            .p_4()
            .text_color(rgb(MUTED))
            .child("No properties available")
            .into_any_element(),
    }
}

fn image_file(path: PathBuf) -> gpui::AnyElement {
    let name = path
        .file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned();

    div()
        .id("image-file-properties")
        .h_full()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(rgb(PANEL))
        .child(
            div()
                .h(px(58.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .px_5()
                .border_b_1()
                .border_color(rgb(BORDER))
                .child(properties_tab("Image", true)),
        )
        .child(
            div()
                .px_5()
                .py_3()
                .flex_shrink_0()
                .text_sm()
                .text_ellipsis()
                .child(name),
        )
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .bg(rgb(SURFACE))
                .child(
                    img(path)
                        .absolute()
                        .size_full()
                        .object_fit(ObjectFit::Contain),
                ),
        )
        .into_any_element()
}

fn timeline_file(timeline: &TimelineRuntimeState) -> gpui::AnyElement {
    let property_field = |label: &'static str, value: String, unit: &'static str| {
        div()
            .h(px(48.0))
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .w(px(112.0))
                    .flex_shrink_0()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child(label),
            )
            .child(
                div()
                    .h(px(48.0))
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap_1()
                    .px_3()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(SURFACE))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .text_sm()
                            .text_ellipsis()
                            .child(value),
                    )
                    .when(!unit.is_empty(), |field| {
                        field.child(div().text_sm().text_color(rgb(MUTED)).child(unit))
                    }),
            )
    };
    let name = timeline
        .path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| timeline.path.display().to_string());
    let duration = timeline
        .editing_state
        .settings
        .frame_rate
        .seconds(timeline.editing_state.content_duration());
    let playhead = timeline
        .editing_state
        .settings
        .frame_rate
        .seconds(timeline.playhead());

    div()
        .id("timeline-file-properties-v2")
        .h_full()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(rgb(PANEL))
        .child(
            div()
                .h(px(58.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap_5()
                .px_5()
                .border_b_1()
                .border_color(rgb(BORDER))
                .child(properties_tab("Timeline", true)),
        )
        .child(
            div()
                .id("timeline-file-properties-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap_3()
                .px_5()
                .py_2()
                .child(property_field("Name", name, ""))
                .child(property_field(
                    "Path",
                    timeline.path.display().to_string(),
                    "",
                ))
                .child(property_field("Duration", format_time(duration, false), ""))
                .child(property_field("Playhead", format_time(playhead, false), ""))
                .child(property_field(
                    "Frame rate",
                    timeline.editing_state.settings.frame_rate.label(),
                    "",
                ))
                .child(property_field(
                    "Resolution",
                    format!(
                        "{} × {}",
                        timeline.editing_state.settings.width,
                        timeline.editing_state.settings.height
                    ),
                    "px",
                ))
                .child(property_field(
                    "Audio rate",
                    timeline
                        .editing_state
                        .settings
                        .audio_sample_rate
                        .to_string(),
                    "Hz",
                ))
                .child(property_field(
                    "Tracks",
                    timeline.editing_state.tracks.len().to_string(),
                    "",
                ))
                .child(property_field(
                    "Clips",
                    timeline.editing_state.clips.len().to_string(),
                    "",
                )),
        )
        .into_any_element()
}

fn audio_clip(clip: &AudioClip) -> gpui::AnyElement {
    let property_field = |label: &'static str, value: String, unit: &'static str| {
        div()
            .h(px(48.0))
            .flex()
            .items_center()
            .gap_4()
            .child(
                div()
                    .w(px(112.0))
                    .flex_shrink_0()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child(label),
            )
            .child(
                div()
                    .h(px(48.0))
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_3()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(SURFACE))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .text_sm()
                            .text_ellipsis()
                            .child(value),
                    )
                    .when(!unit.is_empty(), |field| {
                        field.child(div().text_sm().text_color(rgb(MUTED)).child(unit))
                    }),
            )
    };
    let duration = clip.source_out - clip.source_in;

    div()
        .id("audio-clip-properties-v2")
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(rgb(PANEL))
        .child(
            div()
                .h(px(58.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap_5()
                .px_5()
                .border_b_1()
                .border_color(rgb(BORDER))
                .child(properties_tab("Audio", true)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .px_5()
                .py_5()
                .child(properties_section_label("CLIP"))
                .child(property_field(
                    "Timeline start",
                    i64::from(clip.timeline_start).to_string(),
                    "frames",
                ))
                .child(property_field(
                    "Source in",
                    i64::from(clip.source_in).to_string(),
                    "frames",
                ))
                .child(property_field(
                    "Source out",
                    i64::from(clip.source_out).to_string(),
                    "frames",
                ))
                .child(property_field(
                    "Duration",
                    i64::from(duration).to_string(),
                    "frames",
                ))
                .child(properties_section_label("AUDIO"))
                .child(property_field(
                    "Gain",
                    format!("{:.2}", clip.audio_properties.gain_db),
                    "dB",
                ))
                .child(property_field(
                    "Muted",
                    if clip.audio_properties.muted {
                        "Yes".to_string()
                    } else {
                        "No".to_string()
                    },
                    "",
                )),
        )
        .into_any_element()
}

fn media_file(path: &Path, tab: &'static str) -> gpui::AnyElement {
    let property_field = |label: &'static str, value: String| {
        div()
            .h(px(48.0))
            .flex()
            .items_center()
            .gap_4()
            .child(
                div()
                    .w(px(112.0))
                    .flex_shrink_0()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child(label),
            )
            .child(
                div()
                    .h(px(48.0))
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .items_center()
                    .px_3()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(SURFACE))
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .text_sm()
                            .text_ellipsis()
                            .child(value),
                    ),
            )
    };
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let format = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_uppercase())
        .unwrap_or_else(|| "Unknown".to_string());
    let directory = path
        .parent()
        .filter(|directory| !directory.as_os_str().is_empty())
        .map(|directory| directory.display().to_string())
        .unwrap_or_else(|| ".".to_string());

    div()
        .id("media-file-properties")
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(rgb(PANEL))
        .child(
            div()
                .h(px(58.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .gap_5()
                .px_5()
                .border_b_1()
                .border_color(rgb(BORDER))
                .child(properties_tab(tab, true)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .px_5()
                .py_5()
                .child(properties_section_label("FILE"))
                .child(property_field("Name", name))
                .child(property_field("Format", format))
                .child(property_field("Folder", directory))
                .child(property_field("Path", path.display().to_string())),
        )
        .into_any_element()
}

fn video_clip_view(
    clip: &VideoClip,
    asset_name: &str,
    track_name: &str,
    frame_rate: FrameRate,
) -> gpui::AnyElement {
    let duration = clip.source_out - clip.source_in;
    let scale_percent = clip.video_properties.scale * 100.0;
    let scale_fraction = (scale_percent / 200.0).clamp(0.0, 1.0) as f32;
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
                .inset(px(5.0))
                .rounded(px(2.0))
                .bg(rgba(0x294d7599))
                .border_1()
                .border_color(rgb(ACCENT))
                .flex()
                .items_center()
                .justify_center()
                .child(div().size(px(6.0)).rounded_full().bg(rgb(ACCENT))),
        );

    div()
        .id("video-clip-properties-v3")
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
                        .child(
                            div()
                                .text_size(px(12.0))
                                .line_height(px(16.0))
                                .text_color(rgb(MUTED))
                                .text_ellipsis()
                                .child(format!("Video clip · {track_name}")),
                        ),
                ),
        )
        .child(
            div()
                .id("video-clip-properties-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .p(px(16.0))
                .child(video_clip_section_heading_view("TIMING"))
                .child(
                    div()
                        .mt(px(12.0))
                        .grid()
                        .grid_cols(2)
                        .gap(px(8.0))
                        .child(video_clip_timing_card_view(
                            "Start",
                            clip.timeline_start,
                            frame_rate,
                        ))
                        .child(video_clip_timing_card_view(
                            "Duration", duration, frame_rate,
                        ))
                        .child(video_clip_timing_card_view(
                            "Source in",
                            clip.source_in,
                            frame_rate,
                        ))
                        .child(video_clip_timing_card_view(
                            "Source out",
                            clip.source_out,
                            frame_rate,
                        )),
                )
                .child(div().h(px(1.0)).my(px(16.0)).bg(rgb(0x1f1f24)))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(video_clip_section_heading_view("TRANSFORM"))
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
                                .child(video_clip_transform_value_view(
                                    "X",
                                    clip.video_properties.position_x,
                                    "px",
                                ))
                                .child(video_clip_transform_value_view(
                                    "Y",
                                    clip.video_properties.position_y,
                                    "px",
                                )),
                        ),
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
                        .child(
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
                                        .w(relative(scale_fraction))
                                        .rounded_full()
                                        .bg(rgb(ACCENT)),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .left(relative(0.5))
                                        .top(px(-3.0))
                                        .w(px(1.0))
                                        .h(px(10.0))
                                        .bg(rgb(0x55555e)),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .left(relative(scale_fraction))
                                        .ml(px(-7.0))
                                        .top(px(-5.0))
                                        .size(px(14.0))
                                        .rounded_full()
                                        .bg(rgb(TEXT))
                                        .border_2()
                                        .border_color(rgb(PANEL)),
                                ),
                        )
                        .child(
                            div()
                                .w(px(84.0))
                                .flex_shrink_0()
                                .child(video_clip_transform_value_view("", scale_percent, "%")),
                        ),
                ),
        )
        .into_any_element()
}

fn video_clip_section_heading_view(label: &'static str) -> gpui::Div {
    div()
        .text_size(px(11.0))
        .line_height(px(16.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(MUTED))
        .child(label)
}

fn video_clip_timing_card_view(
    label: &'static str,
    frame_index: TimelineFrameIndex,
    frame_rate: FrameRate,
) -> gpui::Div {
    let centiseconds = (frame_rate.seconds(frame_index).max(0.0) * 100.0).round() as u64;
    let time = format!(
        "{}:{:02}.{:02}",
        centiseconds / 6000,
        (centiseconds / 100) % 60,
        centiseconds % 100,
    );
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

fn video_clip_transform_value_view(
    label: &'static str,
    value: f64,
    unit: &'static str,
) -> gpui::Div {
    let formatted = format!("{value:.2}");
    let value_text = formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned();
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
                .child(value_text),
        )
        .child(div().flex_shrink_0().text_color(rgb(MUTED)).child(unit))
}

fn srt_file_view(path: &Path) -> gpui::AnyElement {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(error) => format!("Could not read {}: {error}", path.display()),
    };

    div()
        .id("srt-file-properties")
        .h_full()
        .min_h_0()
        .flex()
        .flex_col()
        .overflow_hidden()
        .bg(rgb(PANEL))
        .child(
            div()
                .h(px(58.0))
                .flex_shrink_0()
                .flex()
                .items_center()
                .px_5()
                .border_b_1()
                .border_color(rgb(BORDER))
                .child(properties_tab("Subtitles", true)),
        )
        .child(
            div()
                .id("srt-file-properties-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .px_5()
                .py_5()
                .child(
                    div()
                        .w_full()
                        .font_family("monospace")
                        .text_sm()
                        .child(content),
                ),
        )
        .into_any_element()
}
