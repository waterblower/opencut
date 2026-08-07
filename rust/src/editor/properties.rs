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

impl Editor {
    pub(super) fn properties_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let content = match &self.preview_target {
            PreviewTarget::Timeline => self.timeline_properties(cx),
            PreviewTarget::VideoFile(path) => self.video_file_properties(path),
            PreviewTarget::AudioFile(path) => self.audio_file_properties(path),
            PreviewTarget::ImageFile(path) => self.image_file_properties(path),
        };

        div()
            .id("editor-properties-panel")
            .relative()
            .w(px(self.properties_panel_width))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(if self.is_resizing_properties_panel {
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
                        cx.listener(Self::begin_properties_panel_resize),
                    )
                    .on_drag(PropertiesPanelResizeDrag, |_, _, _, cx| {
                        cx.new(|_| PropertiesPanelResizeDragView)
                    }),
            )
            .into_any_element()
    }

    fn set_properties_panel_width_from_x(&mut self, x: f32, window: &Window) {
        let viewport_width: f32 = window.viewport_size().width.into();
        let editor_width = (viewport_width - crate::gpui_inspector::docked_width(window)).max(0.0);
        let available_max = (editor_width - MEDIA_PANEL_WIDTH - MIN_PREVIEW_WIDTH)
            .clamp(MIN_PROPERTIES_PANEL_WIDTH, MAX_PROPERTIES_PANEL_WIDTH);
        self.properties_panel_width =
            (editor_width - x).clamp(MIN_PROPERTIES_PANEL_WIDTH, available_max);
    }

    pub(super) fn begin_properties_panel_resize(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.is_resizing_properties_panel = true;
        self.set_properties_panel_width_from_x(event.position.x.into(), window);
        cx.notify();
    }

    pub(super) fn resize_properties_panel_drag(
        &mut self,
        event: &DragMoveEvent<PropertiesPanelResizeDrag>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_resizing_properties_panel {
            self.set_properties_panel_width_from_x(event.event.position.x.into(), window);
            cx.notify();
        }
    }

    pub(super) fn finish_properties_panel_resize(
        &mut self,
        event: &MouseUpEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_resizing_properties_panel && event.button == MouseButton::Left {
            self.set_properties_panel_width_from_x(event.position.x.into(), window);
            self.is_resizing_properties_panel = false;
            cx.notify();
        }
    }

    fn timeline_properties(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let selection_count = self.selected_clip_ids.len();
        if selection_count > 1 {
            return div()
                .id("timeline-multi-properties")
                .flex()
                .flex_col()
                .gap_4()
                .child(properties_title(
                    format!("{selection_count} clips selected"),
                    "Timeline selection",
                ))
                .into_any_element();
        }

        let selected = self.selected_clip_id.and_then(|id| {
            let index = self.project.clip_index(id)?;
            let clip = &self.project.clips[index];
            let asset = self.project.asset(clip.asset_id);
            let track = self.project.track(clip.track_id)?;
            Some((clip, asset, track))
        });
        let editable = self.selected_clips_editable();

        div()
            .id("timeline-properties")
            .when_some(selected, |this, (clip, asset, track)| {
                let title = asset
                    .map(|asset| asset.name.clone())
                    .unwrap_or_else(|| "Missing media".to_string());
                let has_video_transform = track.kind == TrackKind::Video
                    && asset.is_some_and(|asset| asset.kind != MediaKind::Audio);

                this.flex()
                    .flex_col()
                    .when(has_video_transform, |this| {
                        this.child(self.video_transform_panel(
                            clip.id,
                            clip.video_properties,
                            editable,
                            cx,
                        ))
                    })
                    .when(!has_video_transform, |this| {
                        this.gap_4()
                            .child(properties_title(title, "Timeline clip"))
                            .child(properties_value(
                                "Timeline start",
                                format_time(self.project.seconds(clip.timeline_start), false),
                            ))
                            .child(properties_value(
                                "Source in",
                                format_time(self.project.source_start_seconds(clip), false),
                            ))
                            .child(properties_value(
                                "Source out",
                                format_time(
                                    self.project
                                        .source_position_at(clip, clip.timeline_end())
                                        .as_secs_f64(),
                                    false,
                                ),
                            ))
                            .child(properties_value(
                                "Clip duration",
                                format_time(self.project.seconds(clip.duration()), false),
                            ))
                            .child(properties_value("Track", track.name.clone()))
                            .when_some(asset, |this, asset| {
                                this.child(properties_value("Source", asset_description(asset)))
                            })
                    })
            })
            .when(selected.is_none(), |this| {
                this.text_sm()
                    .text_color(rgb(MUTED))
                    .child("Select a timeline clip to view its properties.")
            })
            .into_any_element()
    }

    fn video_file_properties(&self, path: &Path) -> gpui::AnyElement {
        let asset = self.asset_for_path(path);
        let runtime = self.video.as_ref();
        let duration = asset
            .map(|asset| asset.duration)
            .or_else(|| runtime.map(|video| video.duration().as_secs_f64()));
        let resolution = asset
            .map(|asset| (asset.width, asset.height))
            .or_else(|| runtime.map(|video| video.size()).and_then(unsigned_size));
        let framerate = asset
            .map(|asset| asset.framerate)
            .or_else(|| runtime.map(|video| video.framerate()));

        file_properties(path, "Video")
            .when_some(asset, |this, asset| {
                this.child(properties_value("Codec", asset.codec.clone()))
                    .child(properties_value(
                        "Audio",
                        if asset.has_audio { "Yes" } else { "No" }.to_string(),
                    ))
            })
            .when_some(resolution, |this, (width, height)| {
                this.child(properties_value(
                    "Resolution",
                    format!("{width} × {height}"),
                ))
            })
            .when_some(framerate, |this, framerate| {
                this.child(properties_value(
                    "Frame rate",
                    format!("{framerate:.2} fps"),
                ))
            })
            .when_some(duration, |this, duration| {
                this.child(properties_value("Duration", format_time(duration, false)))
            })
            .into_any_element()
    }

    fn audio_file_properties(&self, path: &Path) -> gpui::AnyElement {
        let asset = self.asset_for_path(path);
        let duration = asset.map(|asset| asset.duration).or_else(|| {
            self.standalone_audio
                .as_ref()
                .map(|audio| audio.duration().as_secs_f64())
        });

        file_properties(path, "Audio")
            .when_some(asset, |this, asset| {
                this.child(properties_value("Codec", asset.codec.clone()))
            })
            .when_some(duration, |this, duration| {
                this.child(properties_value("Duration", format_time(duration, false)))
            })
            .into_any_element()
    }

    fn image_file_properties(&self, path: &Path) -> gpui::AnyElement {
        let asset = self.asset_for_path(path);
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

    fn asset_for_path(&self, path: &Path) -> Option<&MediaAsset> {
        self.project
            .assets
            .iter()
            .find(|asset| asset.path.as_path() == path)
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
        .child(properties_title(title, "Project file"))
        .child(properties_value("Type", kind.to_string()))
        .child(properties_value("Path", path.display().to_string()))
}

fn unsigned_size((width, height): (i32, i32)) -> Option<(u32, u32)> {
    (width > 0 && height > 0).then_some((width as u32, height as u32))
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
