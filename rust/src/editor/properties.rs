use super::properties_text::text_clip_panel;
use super::properties_transform::{
    properties_section_label, properties_tab, video_transform_panel,
};
use super::timeline_clip::AudioClip;
use super::*;
use std::path::Path;

#[derive(Clone)]
pub(super) struct PropertiesPanelResizeDrag;

struct PropertiesPanelResizeDragView;

impl Render for PropertiesPanelResizeDragView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().size(px(1.0)).opacity(0.0)
    }
}

#[allow(dead_code)] // Used by the properties panel v2 implementation once it replaces v1.
pub(super) enum PropertiesPanelViewable<'a> {
    VideoClip(&'a VideoClip),
    AudioClip(&'a AudioClip),
    TextClip(&'a TextClip),
    VideoFile(&'a Path),
    AudioFile(&'a Path),
    TimelineFile(&'a Path),
    None,
}

pub fn current_properties_panel_viewable(editor: &Editor) -> PropertiesPanelViewable<'_> {
    match &editor.preview.target {
        PreviewTarget::Timeline(_) => {
            let Some(timeline) = editor.timeline.as_ref() else {
                return PropertiesPanelViewable::None;
            };
            if timeline.interaction.selected_clip_ids.len() == 1 {
                let clip_id = *timeline
                    .interaction
                    .selected_clip_ids
                    .iter()
                    .next()
                    .expect("one selected clip ID was counted");
                let Some(clip) = timeline.data.clip(clip_id) else {
                    return PropertiesPanelViewable::None;
                };
                return match clip {
                    Clip::Video(clip) => PropertiesPanelViewable::VideoClip(clip),
                    Clip::Audio(clip) => PropertiesPanelViewable::AudioClip(clip),
                    Clip::Text(clip) => PropertiesPanelViewable::TextClip(clip),
                };
            }
            PropertiesPanelViewable::TimelineFile(&timeline.path)
        }
        PreviewTarget::VideoFile(path, _) => PropertiesPanelViewable::VideoFile(path),
        PreviewTarget::AudioFile(path, _) => PropertiesPanelViewable::AudioFile(path),
        PreviewTarget::ImageFile(_) => PropertiesPanelViewable::None,
        PreviewTarget::None => {
            let Some(path) = editor.explorer.selected_file.as_deref() else {
                return PropertiesPanelViewable::None;
            };
            if explorer::is_video_path(path) {
                PropertiesPanelViewable::VideoFile(path)
            } else if explorer::is_audio_path(path) {
                PropertiesPanelViewable::AudioFile(path)
            } else if timeline_document::is_timeline_path(path) {
                PropertiesPanelViewable::TimelineFile(path)
            } else {
                PropertiesPanelViewable::None
            }
        }
    }
}

#[allow(dead_code)] // Used once properties panel v2 replaces v1.
pub(super) fn properties_panel_v2(data: PropertiesPanelViewable<'_>) -> gpui::AnyElement {
    match data {
        PropertiesPanelViewable::TextClip(clip) => {
            let property_field =
                |label: &'static str, value: String, unit: &'static str, color: Option<u32>| {
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
                                .when_some(color, |field, color| {
                                    field.child(
                                        div()
                                            .size(px(18.0))
                                            .flex_shrink_0()
                                            .rounded_sm()
                                            .border_1()
                                            .border_color(rgb(BORDER))
                                            .bg(gpui::rgba(color)),
                                    )
                                })
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

            div()
                .id("text-clip-properties-v2")
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
                        .child(properties_tab("Text", true)),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .px_5()
                        .py_5()
                        .child(properties_section_label("CONTENT"))
                        .child(property_field(
                            "Text",
                            clip.properties.text.clone(),
                            "",
                            None,
                        ))
                        .child(property_field(
                            "Length",
                            format!("{:.3}", clip.length.as_secs_f64()),
                            "s",
                            None,
                        ))
                        .child(properties_section_label("STYLE"))
                        .child(property_field(
                            "Font",
                            clip.properties.font.clone(),
                            "",
                            None,
                        ))
                        .child(property_field(
                            "Font size",
                            format!("{}", clip.properties.font_size),
                            "px",
                            None,
                        ))
                        .child(property_field(
                            "Color",
                            format!("#{:08X}", clip.properties.color),
                            "",
                            Some(clip.properties.color),
                        ))
                        .child(properties_section_label("POSITION"))
                        .child(property_field(
                            "Position X",
                            format!("{}", clip.properties.position_x),
                            "",
                            None,
                        ))
                        .child(property_field(
                            "Position Y",
                            format!("{}", clip.properties.position_y),
                            "",
                            None,
                        )),
                )
                .into_any_element()
        }
        PropertiesPanelViewable::VideoClip(_)
        | PropertiesPanelViewable::AudioClip(_)
        | PropertiesPanelViewable::VideoFile(_)
        | PropertiesPanelViewable::AudioFile(_)
        | PropertiesPanelViewable::TimelineFile(_) => panic!("not implemented"),
        PropertiesPanelViewable::None => div()
            .p_4()
            .text_color(rgb(MUTED))
            .child("No properties available")
            .into_any_element(),
    }
}

pub(super) fn properties_panel(editor: &Editor, cx: &mut Context<Editor>) -> gpui::AnyElement {
    let content = match &editor.preview.target {
        PreviewTarget::None => div()
            .p_4()
            .text_color(rgb(MUTED))
            .child("No preview available")
            .into_any_element(),
        PreviewTarget::Timeline(_) => {
            let Some(timeline) = editor.timeline.as_ref() else {
                return div()
                    .p_4()
                    .text_color(rgb(MUTED))
                    .child("No timeline selected")
                    .into_any_element();
            };
            let selected_clip_ids = &timeline.interaction.selected_clip_ids;
            let selection = match selected_clip_ids.len() {
                0 => TimelineClipSelection::None,
                1 => TimelineClipSelection::Single(
                    *selected_clip_ids
                        .iter()
                        .next()
                        .expect("one selected clip ID was counted"),
                ),
                _ => TimelineClipSelection::Multiple(selected_clip_ids),
            };
            timeline_properties(&timeline.data, selection, &editor.properties)
        }
        file_target @ (PreviewTarget::VideoFile(path, _)
        | PreviewTarget::AudioFile(path, _)
        | PreviewTarget::ImageFile(path)) => {
            let file_asset = editor
                .timeline
                .as_ref()
                .and_then(|timeline| timeline.data.asset_for_path(path));
            match file_target {
                PreviewTarget::VideoFile(path, video) => {
                    video_file_properties(path, file_asset, video)
                }
                PreviewTarget::AudioFile(path, audio) => {
                    audio_file_properties(path, file_asset, audio)
                }
                PreviewTarget::ImageFile(path) => image_file_properties(path, file_asset),
                PreviewTarget::None | PreviewTarget::Timeline(_) => {
                    unreachable!("non-file target handled separately")
                }
            }
        }
    };

    div()
        .id("editor-properties-panel")
        .relative()
        .w(px(editor.properties.width))
        .h_full()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .border_l_1()
        .border_color(if editor.properties.resizing {
            rgb(ACCENT)
        } else {
            rgb(BORDER)
        })
        .group_hover("properties-panel-resize", |style| {
            style.border_color(rgb(ACCENT))
        })
        .bg(rgb(PANEL))
        .child(
            div()
                .id("editor-properties-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .child(content),
        )
        .child(
            div()
                .id("properties-panel-resize-handle")
                .absolute()
                .top_0()
                .left(px(-3.0))
                .w(px(6.0))
                .h_full()
                .group("properties-panel-resize")
                .cursor(CursorStyle::ResizeLeftRight)
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(Editor::begin_properties_panel_resize),
                )
                .on_drag(PropertiesPanelResizeDrag, |_, _, _, cx| {
                    cx.new(|_| PropertiesPanelResizeDragView)
                }),
        )
        .into_any_element()
}

enum TimelineClipSelection<'a> {
    None,
    Single(Ulid),
    Multiple(&'a HashSet<Ulid>),
}

fn timeline_properties(
    timeline: &TimelineSerialization,
    selection: TimelineClipSelection<'_>,
    panel: &PropertiesPanelState,
) -> gpui::AnyElement {
    let selected_clip_id = match selection {
        TimelineClipSelection::None => None,
        TimelineClipSelection::Single(clip_id) => Some(clip_id),
        TimelineClipSelection::Multiple(clip_ids) => {
            return div()
                .id("timeline-multi-properties")
                .flex()
                .flex_col()
                .gap_4()
                .child(properties_title(
                    format!("{} clips selected", clip_ids.len()),
                    "Timeline selection",
                ))
                .into_any_element();
        }
    };

    let selected = selected_clip_id.and_then(|id| {
        let index = timeline.clip_index(id)?;
        let clip = &timeline.clips[index];
        let asset = clip.media().and_then(|clip| timeline.asset(clip.asset_id));
        let track = timeline.track(clip.track_id())?;
        Some((clip, asset, track))
    });
    let editable = selected_clip_id
        .is_some_and(|clip_id| timeline.clip(clip_id).is_some() && !timeline.clip_locked(clip_id));

    div()
        .id("timeline-properties")
        .when_some(selected, |this, (clip, asset, track)| {
            let title = asset.map(|asset| asset.name.clone()).unwrap_or_else(|| {
                clip.text().map_or_else(
                    || "Missing media".to_string(),
                    |clip| clip.properties.text.clone(),
                )
            });
            let has_video_transform = track.kind == TrackKind::Video
                && asset.is_some_and(|asset| asset.kind != MediaKind::Audio);

            this.flex()
                .flex_col()
                .when(has_video_transform, |this| {
                    this.child(video_transform_panel(panel, editable))
                })
                .when(!has_video_transform && clip.media().is_some(), |this| {
                    this.gap_4()
                        .child(properties_title(title, "Timeline clip"))
                        .child(properties_value(
                            "Timeline start",
                            format_time(timeline.seconds(clip.timeline_start()), false),
                        ))
                        .child(properties_value(
                            "Source in",
                            format_time(timeline.source_start_seconds(clip), false),
                        ))
                        .child(properties_value(
                            "Source out",
                            format_time(
                                timeline
                                    .source_position_at(
                                        clip,
                                        clip.timeline_end(timeline.settings.frame_rate),
                                    )
                                    .as_secs_f64(),
                                false,
                            ),
                        ))
                        .child(properties_value(
                            "Clip duration",
                            format_time(
                                timeline.seconds(clip.frame_length(timeline.settings.frame_rate)),
                                false,
                            ),
                        ))
                        .child(properties_value("Track", track.name.clone()))
                        .when_some(asset, |this, asset| {
                            this.child(properties_value("Source", asset_description(asset)))
                        })
                })
                .when_some(clip.text(), |this, text_clip| {
                    this.child(text_clip_panel(panel, editable, text_clip.properties.color))
                })
        })
        .when(selected.is_none(), |this| {
            this.text_sm()
                .text_color(rgb(MUTED))
                .child("Select a timeline clip to view its properties.")
        })
        .into_any_element()
}

fn video_file_properties(
    path: &Path,
    asset: Option<&MediaAsset>,
    video: &VideoBackend,
) -> gpui::AnyElement {
    let duration = video.duration().as_secs_f64();
    let (width, height) = video.frame_size();
    let framerate = video.framerate();

    file_properties(path, "Video")
        .when_some(asset, |this, asset| {
            this.child(properties_value("Codec", asset.codec.clone()))
                .child(properties_value(
                    "Audio",
                    if asset.has_audio { "Yes" } else { "No" }.to_string(),
                ))
        })
        .child(properties_value(
            "Resolution",
            format!("{width} × {height}"),
        ))
        .when_some(framerate, |this, framerate| {
            this.child(properties_value(
                "Frame rate",
                format!("{framerate:.2} fps"),
            ))
        })
        .child(properties_value("Duration", format_time(duration, false)))
        .into_any_element()
}

fn audio_file_properties(
    path: &Path,
    asset: Option<&MediaAsset>,
    audio: &AudioBackend,
) -> gpui::AnyElement {
    let duration = asset
        .map(|asset| asset.duration)
        .unwrap_or_else(|| audio.duration().as_secs_f64());

    file_properties(path, "Audio")
        .when_some(asset, |this, asset| {
            this.child(properties_value("Codec", asset.codec.clone()))
        })
        .child(properties_value("Duration", format_time(duration, false)))
        .into_any_element()
}

fn image_file_properties(path: &Path, asset: Option<&MediaAsset>) -> gpui::AnyElement {
    file_properties(path, "Image")
        .when_some(asset, |this, asset| {
            this.child(properties_value("Codec", asset.codec.clone()))
                .child(properties_value(
                    "Resolution",
                    format!("{} × {}", asset.width, asset.height),
                ))
        })
        .into_any_element()
}

fn set_properties_panel_width_from_x(panel: &mut PropertiesPanelState, x: f32, window: &Window) {
    let viewport_width: f32 = window.viewport_size().width.into();
    let editor_width = (viewport_width - crate::gpui_inspector::docked_width(window)).max(0.0);
    let available_max = (editor_width - MEDIA_PANEL_WIDTH - MIN_PREVIEW_WIDTH)
        .clamp(MIN_PROPERTIES_PANEL_WIDTH, MAX_PROPERTIES_PANEL_WIDTH);
    panel.width = (editor_width - x).clamp(MIN_PROPERTIES_PANEL_WIDTH, available_max);
}

impl Editor {
    pub(super) fn begin_properties_panel_resize(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.properties.resizing = true;
        set_properties_panel_width_from_x(&mut self.properties, event.position.x.into(), window);
        cx.notify();
    }

    pub(super) fn resize_properties_panel_drag(
        &mut self,
        event: &DragMoveEvent<PropertiesPanelResizeDrag>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.properties.resizing {
            set_properties_panel_width_from_x(
                &mut self.properties,
                event.event.position.x.into(),
                window,
            );
            cx.notify();
        }
    }

    pub(super) fn finish_properties_panel_resize(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.properties.resizing && event.button == MouseButton::Left {
            set_properties_panel_width_from_x(
                &mut self.properties,
                event.position.x.into(),
                window,
            );
            self.properties.resizing = false;
            cx.notify();
        }
    }
}

fn properties_title(title: String, subtitle: &'static str) -> gpui::Div {
    div()
        .min_w_0()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_base()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_ellipsis()
                .child(title),
        )
        .child(div().text_xs().text_color(rgb(MUTED)).child(subtitle))
}

fn file_properties(path: &Path, kind: &'static str) -> gpui::Div {
    let title = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());

    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(properties_title(title, "Timeline file"))
        .child(properties_value("Type", kind.to_string()))
        .child(properties_value("Path", path.display().to_string()))
}

fn properties_value(label: &str, value: String) -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_xs()
                .text_color(rgb(MUTED))
                .child(label.to_string()),
        )
        .child(div().text_sm().child(value))
}

fn asset_description(asset: &MediaAsset) -> String {
    match asset.kind {
        MediaKind::Image => format!("{} image · {}×{}", asset.codec, asset.width, asset.height),
        MediaKind::Audio => format!("{} audio", asset.codec),
        MediaKind::Video => format!(
            "{} · {}×{} · {:.2} fps",
            asset.codec, asset.width, asset.height, asset.framerate
        ),
    }
}
