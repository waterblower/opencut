use crate::asset::IconName;
use crate::edit_action::{EditAction, MAX_CLIP_GAIN_DB, MIN_CLIP_GAIN_DB};
use crate::editor::Editor;
use crate::event_bus::{AppEvent, EventBus};
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
use crate::theme::{ACCENT, BORDER, ERROR, MUTED, PANEL, SURFACE, TEXT};
use crate::timeline::{FrameRate, TimelineFrameIndex};
use crate::timeline_clip::{AudioClipProperties, Clip};
use crate::timeline_document;
use crate::track::TrackKind;
use crate::ui_components::text_input_v2::{InputConstraint, TextInputV2};
use gpui::prelude::FluentBuilder;
use gpui::{
    App, AppContext, Bounds, ContentMask, ElementId, Entity, FocusHandle, Focusable, FontWeight,
    InteractiveElement, IntoElement, ParentElement, PathBuilder, RenderOnce,
    StatefulInteractiveElement, Styled, Window, canvas, div, point, px, relative, rgb, size, svg,
};
use std::path::Path;
use std::rc::Rc;
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
            Clip::Video(clip) => PropertyPanel::VideoClip(video_clip_panel(
                &timeline.editing_state,
                clip,
                editor.event_bus.clone(),
                editor.focus_handle.clone(),
            )),
            Clip::Audio(clip) => PropertyPanel::AudioClip(audio_clip_panel(
                timeline,
                clip,
                &editor.waveform_cache,
                editor.event_bus.clone(),
                editor.focus_handle.clone(),
            )),
            Clip::Text(clip) => PropertyPanel::TextClip(text_clip_panel(
                &timeline.editing_state,
                clip,
                editor.event_bus.clone(),
                editor.focus_handle.clone(),
            )),
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

pub(crate) fn clip_property_panel_header_view(
    kind: TrackKind,
    title: String,
    track_name: &str,
    locked: bool,
) -> gpui::Div {
    let (label, background, border) = match kind {
        TrackKind::Text => ("T", 0x7251a3, 0x8261b3),
        TrackKind::Video => ("V", 0x294d75, 0x395d85),
        TrackKind::Audio => ("A", 0x24656b, 0x34757b),
    };
    let badge = div()
        .size(px(32.0))
        .flex_shrink_0()
        .rounded(px(8.0))
        .border_1()
        .border_color(rgb(if locked { 0x62626e } else { border }))
        .bg(rgb(if locked { 0x33333b } else { background }))
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(12.0))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(0xffffff));
    let badge_content = if locked {
        badge.child(
            svg()
                .path(IconName::Lock.path())
                .size(px(20.0))
                .text_color(rgb(0xc9c9cf)),
        )
    } else {
        badge.child(label)
    };
    property_panel_header_view(
        badge_content,
        title,
        clip_track_subtitle_view(kind, track_name, locked),
    )
    .when(locked, |header| {
        header.bg(rgb(0x242429)).border_color(rgb(0x62626e))
    })
}

pub(crate) fn clip_property_panel_frame_view(
    content: gpui::AnyElement,
    locked: bool,
) -> gpui::AnyElement {
    div()
        .size_full()
        .min_h_0()
        .relative()
        .overflow_hidden()
        .child(content)
        .when(locked, |panel| {
            panel.child(
                canvas(
                    |_, _, _| (),
                    |bounds, (), window, _| {
                        // 装饰层不注册命中区域，保持内容滚动与分隔线拖动正常。
                        let width = f32::from(bounds.size.width);
                        let height = f32::from(bounds.size.height);
                        for (strip_width, strip_height) in [(width, 5.0), (5.0, height)] {
                            let strip_bounds =
                                Bounds::new(bounds.origin, size(px(strip_width), px(strip_height)));
                            window.with_content_mask(
                                Some(ContentMask {
                                    bounds: strip_bounds,
                                }),
                                |window| {
                                    let mut stripes = PathBuilder::stroke(px(2.0));
                                    let mut offset = -8.0;
                                    while offset < strip_width + strip_height {
                                        stripes.move_to(bounds.origin + point(px(offset), px(0.0)));
                                        stripes.line_to(
                                            bounds.origin
                                                + point(
                                                    px(offset - strip_height),
                                                    px(strip_height),
                                                ),
                                        );
                                        offset += 8.0;
                                    }
                                    if let Ok(path) = stripes.build() {
                                        window.paint_path(path, rgb(0x676773));
                                    }
                                },
                            );
                        }
                        let mut border = PathBuilder::stroke(px(1.0));
                        for (index, (x, y)) in [
                            (0.5, 0.5),
                            (width - 0.5, 0.5),
                            (width - 0.5, height - 0.5),
                            (0.5, height - 0.5),
                            (0.5, 0.5),
                        ]
                        .into_iter()
                        .enumerate()
                        {
                            let position = bounds.origin + point(px(x), px(y));
                            if index == 0 {
                                border.move_to(position);
                            } else {
                                border.line_to(position);
                            }
                        }
                        if let Ok(path) = border.build() {
                            window.paint_path(path, rgb(0x62626e));
                        }
                    },
                )
                .absolute()
                .inset_0()
                .size_full(),
            )
        })
        .into_any_element()
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
    locked: bool,
) -> gpui::Div {
    let time = format_property_panel_time(frame_rate.seconds(frame_index));
    div()
        .min_w_0()
        .min_h(px(50.0))
        .px(px(10.0))
        .py(px(8.0))
        .rounded(px(6.0))
        .bg(rgb(if locked { 0x19191d } else { 0x131316 }))
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

pub(super) fn slider_view(fraction: f32, reference_fraction: f32, locked: bool) -> gpui::Div {
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
                .bg(rgb(if locked { 0x55555e } else { ACCENT })),
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
                .ml(px(if locked { -5.0 } else { -7.0 }))
                .top(px(if locked { -3.0 } else { -5.0 }))
                .size(px(if locked { 10.0 } else { 14.0 }))
                .rounded_full()
                .bg(rgb(if locked { 0x55555e } else { TEXT }))
                .when(!locked, |knob| knob.border_2().border_color(rgb(PANEL))),
        )
}

pub(super) fn audio_properties_view(
    clip_id: Ulid,
    audio_properties: &AudioClipProperties,
    locked: bool,
    event_bus: Entity<EventBus>,
    return_focus: FocusHandle,
) -> gpui::Div {
    let unmuted = !audio_properties.muted;
    div()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(MUTED))
                        .child("Gain"),
                )
                .child(div().w(px(84.0)).flex_shrink_0().child(if locked {
                    value_field_view("", format!("{:.2}", audio_properties.gain_db), "dB", true)
                        .into_any_element()
                } else {
                    let event_bus = event_bus.clone();
                    div()
                        .h(px(30.0))
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap(px(6.0))
                        .px(px(9.0)) // 加 1px 边框后与只读态一致：内容距外缘 10px。
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(rgb(BORDER))
                        .bg(rgb(SURFACE))
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .font_family("monospace")
                                .text_size(px(13.0))
                                .text_color(rgb(TEXT))
                                .child(
                                    TextInputV2::new(
                                        format!("clip-gain-{clip_id}"),
                                        format!("{:.2}", audio_properties.gain_db),
                                        return_focus,
                                    )
                                    .constraint(InputConstraint::Number)
                                    .on_text_change(
                                        move |text, _, cx| {
                                            let Ok(gain_db) = text.parse::<f64>() else {
                                                return;
                                            };
                                            if !(MIN_CLIP_GAIN_DB..=MAX_CLIP_GAIN_DB)
                                                .contains(&gain_db)
                                            {
                                                return;
                                            }
                                            event_bus.update(cx, |_, cx| {
                                                cx.emit(AppEvent::Edit(EditAction::SetClipGain {
                                                    clip_id,
                                                    gain_db,
                                                }))
                                            })
                                        },
                                    ),
                                ),
                        )
                        .child(
                            div()
                                .flex_shrink_0()
                                .text_size(px(12.0))
                                .text_color(rgb(MUTED))
                                .child("dB"),
                        )
                        .into_any_element()
                })),
        )
        .child(
            div()
                .mt(px(14.0))
                .flex()
                .items_center()
                .justify_between()
                .opacity(if locked { 0.5 } else { 1.0 })
                .child(
                    svg()
                        .path(if unmuted {
                            IconName::Mic.path()
                        } else {
                            IconName::MicOff.path()
                        })
                        .size(px(16.0))
                        .text_color(rgb(0xc9c9cf)),
                )
                .child(
                    div()
                        .id("clip-mute-toggle")
                        .relative()
                        .w(px(34.0))
                        .h(px(20.0))
                        .rounded_full()
                        .bg(rgb(if unmuted { ACCENT } else { BORDER }))
                        .when(!locked, |toggle| {
                            toggle.cursor_pointer().on_click(move |_, _, cx| {
                                event_bus.update(cx, |_, cx| {
                                    cx.emit(AppEvent::Edit(EditAction::SetClipMuted {
                                        clip_id,
                                        muted: unmuted,
                                    }))
                                })
                            })
                        })
                        .child(
                            div()
                                .absolute()
                                .top(px(3.0))
                                .left(px(if unmuted { 17.0 } else { 3.0 }))
                                .size(px(14.0))
                                .rounded_full()
                                .bg(rgb(if unmuted { PANEL } else { MUTED })),
                        ),
                ),
        )
}

pub(super) fn value_field_view(
    label: &'static str,
    value: String,
    unit: &'static str,
    locked: bool,
) -> gpui::Div {
    div()
        .h(px(30.0))
        .min_w_0()
        .flex()
        .items_center()
        .gap(px(6.0))
        .px(px(10.0))
        .rounded(px(6.0))
        .when(!locked, |field| field.border_1().border_color(rgb(BORDER)))
        .bg(rgb(if locked { 0x19191d } else { SURFACE }))
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

fn clip_track_subtitle_view(kind: TrackKind, track_name: &str, locked: bool) -> gpui::Div {
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
        .when(locked, |subtitle| {
            subtitle.child(
                div()
                    .flex_shrink_0()
                    .text_color(rgb(0xc9c9cf))
                    .child("· Locked"),
            )
        })
}
