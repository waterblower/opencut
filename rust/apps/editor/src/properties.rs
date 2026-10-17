use crate::asset::IconName;
use crate::editor::Editor;
use crate::explorer_file_entry::{is_audio_path, is_image_path, is_srt_path, is_video_path};
use crate::properties_text::text_clip_view;
use crate::properties_transform::VideoTransformInputs;
use crate::theme::{ACCENT, BORDER, CLIP_BLUE, ERROR, MUTED, PANEL, SURFACE, TEXT};
use crate::time_format::format_time;
use crate::timeline::{FrameRate, FrameRateLabel, TimelineFrameIndex, TimelineRuntimeState};
use crate::timeline_clip::{AudioClip, Clip, TextClip, VideoClip};
use crate::timeline_document;
use crate::timeline_document::resolve_asset_path;
use crate::track::TrackKind;
use crate::waveform::WaveformData;
use anyhow::Result;
use gpui::prelude::FluentBuilder;
use gpui::{
    Bounds, FontWeight, InteractiveElement, IntoElement, ParentElement, StatefulInteractiveElement,
    Styled, canvas, div, fill, point, px, relative, rgb, rgba, size, svg,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
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
    path: PathBuf,           // 绝对路径
    size_bytes: Option<u64>, // 取自 Explorer 条目
}

pub struct ImageFilePanel {
    path: PathBuf,           // 绝对路径
    size_bytes: Option<u64>, // 取自 Explorer 条目
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
    asset_name: String,
    track_name: String,
    frame_rate: FrameRate,
    waveform: Option<Arc<WaveformData>>, // 构造面板时缓存里还没有波形则为 None
}

pub struct TextClipPanel {
    pub clip: TextClip,
    pub track_name: String,
    pub frame_rate: FrameRate,
}

pub struct TimelinePanel {
    path: PathBuf,         // 绝对路径
    project_root: PathBuf, // 用于显示相对路径
    settings: TimelineSettings,
    duration_seconds: f64,
    text_track_count: usize,
    video_track_count: usize,
    audio_track_count: usize,
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
            Clip::Audio(clip) => {
                let asset = timeline.editing_state.asset(clip.asset_id);
                let asset_name = match asset {
                    Some(asset) => asset.name.clone(),
                    None => "Missing media".to_owned(),
                };
                let track_name = match timeline
                    .editing_state
                    .tracks
                    .iter()
                    .find(|track| track.id == clip.track_id)
                {
                    Some(track) => track.name.clone(),
                    None => "Unknown track".to_owned(),
                };
                let waveform = (|| {
                    let Some(asset) = asset else {
                        return None;
                    };
                    let Some(timeline_directory) = timeline.path.parent() else {
                        return None;
                    };
                    let media_path = resolve_asset_path(timeline_directory, &asset.path); // 波形缓存以素材的绝对路径为键。
                    editor.waveform_cache.get(&media_path).cloned()
                })();
                PropertiesPanel::AudioClip(AudioClipPanel {
                    clip: clip.clone(),
                    asset_name,
                    track_name,
                    frame_rate: timeline.editing_state.settings.frame_rate,
                    waveform,
                })
            }
            Clip::Text(clip) => {
                PropertiesPanel::TextClip(text_clip_panel(&timeline.editing_state, clip))
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
            return PropertiesPanel::VideoFile(VideoFilePanel {
                path: absolute_path,
                size_bytes,
            });
        }
        if is_audio_path(path) {
            return PropertiesPanel::AudioFile(AudioFilePanel {
                path: absolute_path,
                size_bytes,
            });
        }
        if is_image_path(path) {
            return PropertiesPanel::ImageFile(ImageFilePanel {
                path: absolute_path,
                size_bytes,
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
            return PropertiesPanel::Timeline(timeline_panel(timeline, &editor.project_root));
        }
        return PropertiesPanel::None;
    }

    let Some(timeline) = editor.timeline.as_ref() else {
        return PropertiesPanel::None;
    };
    PropertiesPanel::Timeline(timeline_panel(timeline, &editor.project_root))
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
        PropertiesPanel::AudioClip(panel) => audio_clip_view(panel),
        PropertiesPanel::VideoFile(panel) => video_file_view(panel),
        PropertiesPanel::Timeline(panel) => timeline_view(panel),
        PropertiesPanel::ImageFile(panel) => image_file_view(panel),
        PropertiesPanel::AudioFile(panel) => audio_file_view(panel),
        PropertiesPanel::SrtFile(panel) => srt_file_view(panel),
        PropertiesPanel::None => empty_panel_view(),
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

fn image_file_view(panel: &ImageFilePanel) -> gpui::AnyElement {
    let path = panel.path.as_path();
    let name = match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => path.display().to_string(),
    };
    let format = match path.extension() {
        Some(extension) => extension.to_string_lossy().to_ascii_uppercase(),
        None => "Unknown".to_owned(),
    };
    let header = properties_header_view(
        file_badge_view(&format, 0x665184, 0xc3a9e8),
        name,
        "Image file".to_owned(),
    );
    let content = div()
        .child(properties_heading_view("FILE"))
        .child(file_details_view(path, panel.size_bytes))
        .child(properties_divider_view())
        .child(properties_heading_view("LOCATION"))
        .child(file_location_view(path));
    readonly_properties_panel_view("image-file-properties", header, content)
}

fn timeline_panel(timeline: &TimelineRuntimeState, project_root: &Path) -> TimelinePanel {
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

fn timeline_view(panel: &TimelinePanel) -> gpui::AnyElement {
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
    let header = properties_header_view(badge, name, relative_path);
    let content = div()
        .child(properties_heading_view("DURATION"))
        .child(
            div()
                .mt(px(6.0))
                .font_family("monospace")
                .text_size(px(24.0))
                .line_height(px(30.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(format_time(panel.duration_seconds, false)),
        )
        .child(properties_divider_view())
        .child(properties_heading_view("FORMAT"))
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
        .child(properties_divider_view())
        .child(properties_heading_view("CONTENTS"))
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
        .child(properties_divider_view())
        .child(properties_heading_view("LOCATION"))
        .child(file_location_view(&panel.path));
    readonly_properties_panel_view("timeline-properties", header, content)
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

fn audio_clip_view(panel: &AudioClipPanel) -> gpui::AnyElement {
    let clip = &panel.clip;
    let frame_rate = panel.frame_rate;
    let duration = clip.source_out - clip.source_in;
    let gain_db = clip.audio_properties.gain_db;
    let muted = clip.audio_properties.muted;
    let badge = div()
        .size(px(32.0))
        .flex_shrink_0()
        .rounded(px(8.0))
        .border_1()
        .border_color(rgb(0x34757b))
        .bg(rgb(0x24656b))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0xffffff))
        .child("A");
    let header = properties_header_view(
        badge,
        panel.asset_name.clone(),
        format!("Audio clip · {}", panel.track_name),
    );
    let content = div()
        .child(properties_heading_view("TIMING"))
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
        .child(properties_divider_view())
        .child(properties_heading_view("AUDIO"))
        .child(div().mt(px(12.0)).child(audio_waveform_view(
            panel.waveform.clone(),
            frame_rate.seconds(clip.source_in),
            frame_rate.seconds(clip.source_out),
        )))
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
                        .child("Gain"),
                )
                .child(slider_view(
                    ((gain_db + 60.0) / 80.0).clamp(0.0, 1.0) as f32,
                    0.75, // 0 dB 在 −60…+20 dB 范围中的位置
                ))
                .child(div().w(px(84.0)).flex_shrink_0().child(value_field_view(
                    "",
                    format!("{gain_db:.2}"),
                    "dB",
                ))),
        )
        .child(
            div()
                .mt(px(14.0))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(MUTED))
                        .child("Mute clip"),
                )
                .child(
                    div()
                        .relative()
                        .w(px(34.0))
                        .h(px(20.0))
                        .rounded_full()
                        .bg(rgb(if muted { ACCENT } else { BORDER }))
                        .child(
                            div()
                                .absolute()
                                .top(px(3.0))
                                .left(px(if muted { 17.0 } else { 3.0 }))
                                .size(px(14.0))
                                .rounded_full()
                                .bg(rgb(if muted { PANEL } else { MUTED })),
                        ),
                ),
        );
    readonly_properties_panel_view("audio-clip-properties", header, content)
}

fn audio_waveform_view(
    waveform: Option<Arc<WaveformData>>,
    source_start_seconds: f64,
    source_end_seconds: f64,
) -> gpui::Div {
    div()
        .h(px(44.0))
        .overflow_hidden()
        .rounded(px(6.0))
        .bg(rgb(0x131316))
        .child(
            canvas(
                move |bounds, _, _| match &waveform {
                    Some(waveform) => waveform.columns(
                        source_start_seconds,
                        source_end_seconds,
                        (f32::from(bounds.size.width) / 3.0).floor().max(1.0) as usize, // 每根柱 2px 宽、间隔 1px
                    ),
                    None => Vec::new(),
                },
                |bounds, columns, window, _| {
                    let height = f32::from(bounds.size.height);
                    for (index, peak) in columns.iter().enumerate() {
                        let bar_height =
                            (peak.max.abs().max(peak.min.abs()) * height).clamp(1.0, height);
                        window.paint_quad(fill(
                            Bounds::new(
                                point(
                                    bounds.left() + px(index as f32 * 3.0),
                                    bounds.top() + px((height - bar_height) / 2.0),
                                ),
                                size(px(2.0), px(bar_height)),
                            ),
                            rgba(0x69c5cfcc),
                        ));
                    }
                },
            )
            .size_full(),
        )
}

fn audio_file_view(panel: &AudioFilePanel) -> gpui::AnyElement {
    let path = panel.path.as_path();
    let name = match path.file_name() {
        Some(name) => name.to_string_lossy().into_owned(),
        None => path.display().to_string(),
    };
    let format = match path.extension() {
        Some(extension) => extension.to_string_lossy().to_ascii_uppercase(),
        None => "Unknown".to_owned(),
    };
    let header = properties_header_view(
        file_badge_view(&format, 0x32725a, 0x7fd0ae),
        name,
        "Audio file".to_owned(),
    );
    let content = div()
        .child(properties_heading_view("FILE"))
        .child(file_details_view(path, panel.size_bytes))
        .child(properties_divider_view())
        .child(properties_heading_view("LOCATION"))
        .child(file_location_view(path))
        .child(properties_divider_view())
        .child(file_hint_view(
            "Drag onto an audio track to add it to the timeline.",
        ));
    readonly_properties_panel_view("audio-file-properties", header, content)
}

fn empty_panel_view() -> gpui::AnyElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .px(px(16.0))
        .bg(rgb(PANEL))
        .text_color(rgb(TEXT))
        .child(
            div()
                .size(px(48.0))
                .rounded(px(12.0))
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(0x131316))
                .flex()
                .items_center()
                .justify_center()
                .child(
                    svg()
                        .path(IconName::Pointer.path())
                        .size(px(24.0))
                        .text_color(rgb(MUTED)),
                ),
        )
        .child(
            div()
                .mt(px(14.0))
                .text_size(px(14.0))
                .line_height(px(20.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child("Nothing selected"),
        )
        .child(
            div()
                .mt(px(5.0))
                .text_center()
                .text_size(px(12.0))
                .line_height(px(17.0))
                .text_color(rgb(MUTED))
                .child("Select a clip on the timeline or a file in the Explorer."),
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

fn timing_card_view(
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

fn slider_view(fraction: f32, reference_fraction: f32) -> gpui::Div {
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

fn trimmed_number(value: f64) -> String {
    let formatted = format!("{value:.2}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

fn value_field_view(label: &'static str, value: String, unit: &'static str) -> gpui::Div {
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
    let header = properties_header_view(
        file_badge_view(&format, 0x355b78, 0x8fb9dd),
        name,
        "Video file".to_owned(),
    );
    let content = div()
        .child(properties_heading_view("FILE"))
        .child(file_details_view(path, panel.size_bytes))
        .child(properties_divider_view())
        .child(properties_heading_view("LOCATION"))
        .child(file_location_view(path))
        .child(properties_divider_view())
        .child(file_hint_view(
            "Drag onto a video track to add it to the timeline.",
        ));
    readonly_properties_panel_view("video-file-properties", header, content)
}

/// Format and size rows shared by the file panels.
fn file_details_view(path: &Path, size_bytes: Option<u64>) -> gpui::Div {
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

fn file_location_view(path: &Path) -> gpui::Div {
    div()
        .mt(px(8.0))
        .font_family("monospace")
        .text_size(px(12.0))
        .line_height(px(18.0))
        .text_color(rgb(0xb4b4bc))
        .whitespace_normal()
        .child(path.display().to_string().replace('/', "/\u{200b}")) // 零宽空格让长路径在斜杠处换行
}

fn file_hint_view(text: &'static str) -> gpui::Div {
    div()
        .text_size(px(12.0))
        .line_height(px(17.0))
        .text_color(rgb(MUTED))
        .child(text)
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
