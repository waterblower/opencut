use crate::editor::Editor;
use crate::explorer_file_entry::{is_audio_path, is_image_path, is_srt_path, is_video_path};
use crate::properties_text::text_clip_view;
use crate::properties_transform::{VideoTransformInputs, properties_section_label, properties_tab};
use crate::theme::{ACCENT, BORDER, CLIP_BLUE, ERROR, MUTED, PANEL, SURFACE, TEXT};
use crate::time_format::format_time;
use crate::timeline::{FrameRate, FrameRateLabel, TimelineFrameIndex, TimelineRuntimeState};
use crate::timeline_clip::{AudioClip, Clip, TextClip, VideoClip};
use crate::timeline_document;
use anyhow::Result;
use gpui::prelude::FluentBuilder;
use gpui::{
    FontWeight, InteractiveElement, IntoElement, ObjectFit, ParentElement,
    StatefulInteractiveElement, Styled, StyledImage, div, img, px, relative, rgb, rgba,
};
use std::fs;
use std::path::{Path, PathBuf};
use timeline::{TimelineEditingState, TimelineSettings};
use transcribe::SRT;
use ulid::Ulid;

pub enum PropertiesPanel {
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

pub struct VideoFilePanel {
    pub path: PathBuf,           // 绝对路径
    pub size_bytes: Option<u64>, // 取自 Explorer 条目，不在 render 里读磁盘
}

pub struct AudioFilePanel {
    path: PathBuf, // 相对于项目根目录
}

pub struct ImageFilePanel {
    path: PathBuf, // 绝对路径
}

pub struct SrtFilePanel {
    path: PathBuf,          // 绝对路径
    subtitles: Result<SRT>, // 构造面板时读取并解析
}

pub struct VideoClipPanel {
    clip: VideoClip,
    asset_name: String,
    track_name: String,
    frame_rate: FrameRate,
}

pub struct AudioClipPanel {
    clip: AudioClip,
}

pub struct TextClipPanel {
    pub clip: TextClip,
    pub track_name: String,
    pub frame_rate: FrameRate,
}

pub struct TimelinePanel {
    path: PathBuf,
    settings: TimelineSettings,
    duration_seconds: f64,
    playhead_seconds: f64, // 播放时每帧变化
    track_count: usize,
    clip_count: usize,
}

pub fn current_properties_panel(editor: &Editor) -> PropertiesPanel {
    if let Some(timeline) = editor.timeline.as_ref()
        && let Some(clip_id) = timeline.interaction.selected_clip_id
    {
        let Some(clip) = timeline.editing_state.clip(clip_id) else {
            return PropertiesPanel::None;
        };
        return match clip {
            Clip::Video(clip) => {
                PropertiesPanel::VideoClip(video_clip_panel(&timeline.editing_state, clip))
            }
            Clip::Audio(clip) => PropertiesPanel::AudioClip(AudioClipPanel { clip: clip.clone() }),
            Clip::Text(clip) => {
                PropertiesPanel::TextClip(text_clip_panel(&timeline.editing_state, clip))
            }
        };
    }

    if let Some(path) = editor.explorer.selected_file.as_deref() {
        let absolute_path = editor.project_root.join(path);
        if is_video_path(path) {
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
            return PropertiesPanel::VideoFile(VideoFilePanel {
                path: absolute_path,
                size_bytes,
            });
        }
        if is_audio_path(path) {
            return PropertiesPanel::AudioFile(AudioFilePanel {
                path: path.to_path_buf(),
            });
        }
        if is_image_path(path) {
            return PropertiesPanel::ImageFile(ImageFilePanel {
                path: absolute_path,
            });
        }
        if is_srt_path(path) {
            return PropertiesPanel::SrtFile(srt_file_panel(absolute_path));
        }
        if timeline_document::is_timeline_path(path) {
            let timeline = editor
                .timeline
                .as_ref()
                .filter(|timeline| timeline.path == absolute_path)
                .expect("should have the timeline");
            return PropertiesPanel::Timeline(timeline_panel(timeline));
        }
        return PropertiesPanel::None;
    }

    let Some(timeline) = editor.timeline.as_ref() else {
        return PropertiesPanel::None;
    };
    PropertiesPanel::Timeline(timeline_panel(timeline))
}

pub fn video_clip_panel(editing_state: &TimelineEditingState, clip: &VideoClip) -> VideoClipPanel {
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

pub fn text_clip_panel(editing_state: &TimelineEditingState, clip: &TextClip) -> TextClipPanel {
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

/// Reads and parses the subtitles; failures are logged here and shown by the panel.
pub fn srt_file_panel(path: PathBuf) -> SrtFilePanel {
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

pub(crate) struct PropertiesPanelState {
    pub(crate) transform_inputs: VideoTransformInputs,
    pub(crate) transform_input_clip_id: Option<Ulid>,
    pub(crate) text_input_clip_id: Option<Ulid>,
}

pub(super) fn properties_panel(panel: &PropertiesPanel) -> gpui::AnyElement {
    match panel {
        PropertiesPanel::TextClip(panel) => text_clip_view(panel),
        PropertiesPanel::VideoClip(panel) => video_clip_view(panel),
        PropertiesPanel::AudioClip(panel) => audio_clip(panel),
        PropertiesPanel::VideoFile(panel) => video_file_view(panel),
        PropertiesPanel::Timeline(panel) => timeline_file(panel),
        PropertiesPanel::ImageFile(panel) => image_file(panel),
        PropertiesPanel::AudioFile(panel) => media_file(&panel.path, "Audio"),
        PropertiesPanel::SrtFile(panel) => srt_file_view(panel),
        PropertiesPanel::None => div()
            .p_4()
            .text_color(rgb(MUTED))
            .child("No properties available")
            .into_any_element(),
    }
}

pub fn properties_header_view(badge: gpui::Div, title: String, subtitle: String) -> gpui::Div {
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

pub fn readonly_properties_panel_view(
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

pub fn properties_heading_view(label: &'static str) -> gpui::Div {
    div()
        .text_size(px(11.0))
        .line_height(px(16.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(rgb(MUTED))
        .child(label)
}

pub fn properties_divider_view() -> gpui::Div {
    div().h(px(1.0)).my(px(16.0)).bg(rgb(0x1f1f24))
}

pub fn format_properties_time(seconds: f64) -> String {
    let centiseconds = (seconds.max(0.0) * 100.0).round() as u64;
    format!(
        "{}:{:02}.{:02}",
        centiseconds / 6000,
        (centiseconds / 100) % 60,
        centiseconds % 100,
    )
}

fn image_file(panel: &ImageFilePanel) -> gpui::AnyElement {
    let path = &panel.path;
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
                    img(path.clone())
                        .absolute()
                        .size_full()
                        .object_fit(ObjectFit::Contain),
                ),
        )
        .into_any_element()
}

fn timeline_panel(timeline: &TimelineRuntimeState) -> TimelinePanel {
    let settings = timeline.editing_state.settings;
    TimelinePanel {
        path: timeline.path.clone(),
        settings,
        duration_seconds: settings
            .frame_rate
            .seconds(timeline.editing_state.content_duration()),
        playhead_seconds: settings.frame_rate.seconds(timeline.playhead()),
        track_count: timeline.editing_state.tracks.len(),
        clip_count: timeline.editing_state.clips.len(),
    }
}

fn timeline_file(panel: &TimelinePanel) -> gpui::AnyElement {
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
    let name = panel
        .path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| panel.path.display().to_string());

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
                .child(property_field("Path", panel.path.display().to_string(), ""))
                .child(property_field(
                    "Duration",
                    format_time(panel.duration_seconds, false),
                    "",
                ))
                .child(property_field(
                    "Playhead",
                    format_time(panel.playhead_seconds, false),
                    "",
                ))
                .child(property_field(
                    "Frame rate",
                    panel.settings.frame_rate.label(),
                    "",
                ))
                .child(property_field(
                    "Resolution",
                    format!("{} × {}", panel.settings.width, panel.settings.height),
                    "px",
                ))
                .child(property_field(
                    "Audio rate",
                    panel.settings.audio_sample_rate.to_string(),
                    "Hz",
                ))
                .child(property_field("Tracks", panel.track_count.to_string(), ""))
                .child(property_field("Clips", panel.clip_count.to_string(), "")),
        )
        .into_any_element()
}

fn audio_clip(panel: &AudioClipPanel) -> gpui::AnyElement {
    let clip = &panel.clip;
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

fn video_clip_view(panel: &VideoClipPanel) -> gpui::AnyElement {
    let clip = &panel.clip;
    let asset_name = panel.asset_name.as_str();
    let track_name = panel.track_name.as_str();
    let frame_rate = panel.frame_rate;
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
                .child(properties_heading_view("TIMING"))
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
                        .child(properties_heading_view("TRANSFORM"))
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

fn video_clip_timing_card_view(
    label: &'static str,
    frame_index: TimelineFrameIndex,
    frame_rate: FrameRate,
) -> gpui::Div {
    let time = format_properties_time(frame_rate.seconds(frame_index));
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

fn video_file_view(panel: &VideoFilePanel) -> gpui::AnyElement {
    let path = panel.path.as_path();
    let name = match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => path.display().to_string(),
    };
    let format = match path.extension() {
        Some(extension) => extension.to_string_lossy().to_ascii_uppercase(),
        None => "Unknown".to_owned(),
    };
    let folder = match path.parent().and_then(Path::file_name) {
        Some(folder) => folder.to_string_lossy().into_owned(),
        None => "/".to_owned(),
    };
    let size = match panel.size_bytes {
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
    let header = properties_header_view(
        file_badge_view(&format, 0x355b78, 0x8fb9dd),
        name,
        "Video file".to_owned(),
    );
    let content = div()
        .child(properties_heading_view("FILE"))
        .child(
            div()
                .mt(px(12.0))
                .child(file_property_row_view("Format", format))
                .child(file_property_row_view("Size", size))
                .child(file_property_row_view("Folder", folder)),
        )
        .child(properties_divider_view())
        .child(properties_heading_view("LOCATION"))
        .child(
            div()
                .mt(px(8.0))
                .font_family("monospace")
                .text_size(px(12.0))
                .line_height(px(18.0))
                .text_color(rgb(0xb4b4bc))
                .whitespace_normal()
                .child(path.display().to_string().replace('/', "/\u{200b}")),
        )
        .child(
            div()
                .mt(px(20.0))
                .text_size(px(12.0))
                .line_height(px(17.0))
                .text_color(rgb(MUTED))
                .child("Drag onto a video track to add it to the timeline."),
        );
    readonly_properties_panel_view("video-file-properties", header, content)
}

fn file_badge_view(label: &str, border: u32, foreground: u32) -> gpui::Div {
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

fn file_property_row_view(label: &'static str, value: String) -> gpui::Div {
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

fn srt_file_view(panel: &SrtFilePanel) -> gpui::AnyElement {
    let name = match panel.path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => panel.path.display().to_string(),
    };
    let mut content = div().child(properties_heading_view("CUES"));
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
                                            format_properties_time(cue.start.as_secs_f64()),
                                            format_properties_time(cue.end.as_secs_f64()),
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
    let header = properties_header_view(file_badge_view("SRT", 0x46464e, 0x8b8b94), name, subtitle);
    readonly_properties_panel_view("srt-file-properties", header, content)
}
