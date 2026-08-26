use super::properties_text::TextClipPropertiesView;
use super::properties_transform::{properties_section_label, properties_tab};
use super::timeline_clip::AudioClip;
use super::*;
use std::path::Path;

#[allow(dead_code)] // Used by the properties panel v2 implementation once it replaces v1.
pub(super) enum PropertiesPanelViewable<'a> {
    VideoClip(&'a VideoClip),
    AudioClip(&'a AudioClip),
    TextClip(&'a TextClip),
    VideoFile(&'a Path),
    AudioFile(&'a Path),
    ImageFile(&'a Path),
    TimelineFile(&'a TimelineRuntimeState),
    None,
}

pub fn current_properties_panel_viewable(editor: &Editor) -> PropertiesPanelViewable<'_> {
    if let Some(timeline) = editor.timeline.as_ref()
        && let Some(clip_id) = timeline.interaction.selected_clip_id
    {
        let Some(clip) = timeline.data.clip(clip_id) else {
            return PropertiesPanelViewable::None;
        };
        return match clip {
            Clip::Video(clip) => PropertiesPanelViewable::VideoClip(clip),
            Clip::Audio(clip) => PropertiesPanelViewable::AudioClip(clip),
            Clip::Text(clip) => PropertiesPanelViewable::TextClip(clip),
        };
    }

    if let Some(path) = editor.explorer.selected_file.as_deref() {
        if explorer::is_video_path(path) {
            return PropertiesPanelViewable::VideoFile(path);
        }
        if explorer::is_audio_path(path) {
            return PropertiesPanelViewable::AudioFile(path);
        }
        if explorer::is_image_path(path) {
            return PropertiesPanelViewable::ImageFile(path);
        }
        if timeline_document::is_timeline_path(path) {
            let timeline = editor
                .timeline
                .as_ref()
                .filter(|timeline| timeline.path == path)
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

pub(super) fn properties_panel(
    data: PropertiesPanelViewable<'_>,
    event_bus: Entity<EventBus>,
) -> gpui::AnyElement {
    match data {
        PropertiesPanelViewable::TextClip(clip) => {
            TextClipPropertiesView::new(clip.clone(), event_bus).into_any_element()
        }
        PropertiesPanelViewable::VideoClip(clip) => video_clip(clip),
        PropertiesPanelViewable::AudioClip(clip) => audio_clip(clip),
        PropertiesPanelViewable::VideoFile(file) => video_file(file),
        PropertiesPanelViewable::TimelineFile(timeline) => timeline_file(timeline),
        PropertiesPanelViewable::AudioFile(_) | PropertiesPanelViewable::ImageFile(_) => {
            panic!("not implemented")
        }
        PropertiesPanelViewable::None => div()
            .p_4()
            .text_color(rgb(MUTED))
            .child("No properties available")
            .into_any_element(),
    }
}

fn timeline_file(timeline: &TimelineRuntimeState) -> gpui::AnyElement {
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
    let name = timeline
        .path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| timeline.path.display().to_string());
    let duration = timeline
        .data
        .settings
        .frame_rate
        .seconds(timeline.data.content_duration());
    let playhead = timeline.data.settings.frame_rate.seconds(timeline.playhead);

    div()
        .id("timeline-file-properties-v2")
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
                .flex()
                .flex_col()
                .gap_3()
                .px_5()
                .py_5()
                .child(properties_section_label("FILE"))
                .child(property_field("Name", name, ""))
                .child(property_field(
                    "Path",
                    timeline.path.display().to_string(),
                    "",
                ))
                .child(properties_section_label("TIMELINE"))
                .child(property_field("Duration", format_time(duration, false), ""))
                .child(property_field("Playhead", format_time(playhead, false), ""))
                .child(property_field(
                    "Frame rate",
                    timeline.data.settings.frame_rate.label(),
                    "",
                ))
                .child(property_field(
                    "Resolution",
                    format!(
                        "{} × {}",
                        timeline.data.settings.width, timeline.data.settings.height
                    ),
                    "px",
                ))
                .child(property_field(
                    "Audio rate",
                    timeline.data.settings.audio_sample_rate.to_string(),
                    "Hz",
                ))
                .child(property_field(
                    "Tracks",
                    timeline.data.tracks.len().to_string(),
                    "",
                ))
                .child(property_field(
                    "Clips",
                    timeline.data.clips.len().to_string(),
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
                    clip.timeline_start.frames().to_string(),
                    "frames",
                ))
                .child(property_field(
                    "Source in",
                    clip.source_in.frames().to_string(),
                    "frames",
                ))
                .child(property_field(
                    "Source out",
                    clip.source_out.frames().to_string(),
                    "frames",
                ))
                .child(property_field(
                    "Duration",
                    duration.frames().to_string(),
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

fn video_file(path: &Path) -> gpui::AnyElement {
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
        .id("video-file-properties-v2")
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
                .child(properties_tab("Video", true)),
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

fn video_clip(clip: &VideoClip) -> gpui::AnyElement {
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
        .id("video-clip-properties-v2")
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
                .child(properties_tab("Video", true)),
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
                    clip.timeline_start.frames().to_string(),
                    "frames",
                ))
                .child(property_field(
                    "Source in",
                    clip.source_in.frames().to_string(),
                    "frames",
                ))
                .child(property_field(
                    "Source out",
                    clip.source_out.frames().to_string(),
                    "frames",
                ))
                .child(property_field(
                    "Duration",
                    duration.frames().to_string(),
                    "frames",
                ))
                .child(properties_section_label("TRANSFORM"))
                .child(property_field(
                    "Position X",
                    format!("{:.2}", clip.video_properties.position_x),
                    "px",
                ))
                .child(property_field(
                    "Position Y",
                    format!("{:.2}", clip.video_properties.position_y),
                    "px",
                ))
                .child(property_field(
                    "Scale",
                    format!("{:.2}", clip.video_properties.scale * 100.0),
                    "%",
                )),
        )
        .into_any_element()
}
