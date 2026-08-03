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

#[derive(Clone, Copy)]
enum VideoTransformProperty {
    PositionX,
    PositionY,
    Scale,
    Rotation,
    Opacity,
    CropLeft,
    CropRight,
    CropTop,
    CropBottom,
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
            let asset = clip
                .asset_id
                .and_then(|asset_id| self.project.asset(asset_id));
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
                                format_time(self.project.seconds(clip.timeline_start)),
                            ))
                            .child(properties_value(
                                "Source in",
                                format_time(self.project.source_start_seconds(clip)),
                            ))
                            .child(properties_value(
                                "Source out",
                                format_time(
                                    self.project
                                        .source_position_at(clip, clip.timeline_end())
                                        .as_secs_f64(),
                                ),
                            ))
                            .child(properties_value(
                                "Clip duration",
                                format_time(self.project.seconds(clip.duration())),
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

    fn video_transform_panel(
        &self,
        clip_id: u64,
        properties: VideoClipProperties,
        editable: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        div()
            .id("video-transform-properties")
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
                    .child(properties_tab("Transform", true)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .px_5()
                    .py_5()
                    .child(properties_section_label("POSITION & SCALE"))
                    .child(self.video_transform_field(
                        clip_id,
                        VideoTransformProperty::PositionX,
                        "Position X",
                        format!("{:.1}", properties.position_x),
                        "px",
                        1.0,
                        "transform-position-x",
                        editable,
                        cx,
                    ))
                    .child(self.video_transform_field(
                        clip_id,
                        VideoTransformProperty::PositionY,
                        "Position Y",
                        format!("{:.1}", properties.position_y),
                        "px",
                        1.0,
                        "transform-position-y",
                        editable,
                        cx,
                    ))
                    .child(self.video_transform_field(
                        clip_id,
                        VideoTransformProperty::Scale,
                        "Scale",
                        format!("{:.1}", properties.scale * 100.0),
                        "%",
                        0.01,
                        "transform-scale",
                        editable,
                        cx,
                    ))
                    .child(self.video_transform_field(
                        clip_id,
                        VideoTransformProperty::Rotation,
                        "Rotation",
                        format!("{:.1}", properties.rotation_degrees),
                        "°",
                        1.0,
                        "transform-rotation",
                        editable,
                        cx,
                    ))
                    .child(self.video_opacity_control(clip_id, properties.opacity, editable, cx))
                    .child(
                        div()
                            .mt_2()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(properties_section_label("CROP"))
                            .child(crop_reset_button(editable).when(editable, |button| {
                                button.on_click(cx.listener(move |editor, _, _, cx| {
                                    editor.reset_video_crop(clip_id);
                                    cx.notify();
                                }))
                            })),
                    )
                    .child(
                        div()
                            .grid()
                            .grid_cols(2)
                            .gap_3()
                            .child(self.video_crop_field(
                                clip_id,
                                VideoTransformProperty::CropLeft,
                                "Left",
                                properties.crop_left,
                                "transform-crop-left",
                                editable,
                                cx,
                            ))
                            .child(self.video_crop_field(
                                clip_id,
                                VideoTransformProperty::CropRight,
                                "Right",
                                properties.crop_right,
                                "transform-crop-right",
                                editable,
                                cx,
                            ))
                            .child(self.video_crop_field(
                                clip_id,
                                VideoTransformProperty::CropTop,
                                "Top",
                                properties.crop_top,
                                "transform-crop-top",
                                editable,
                                cx,
                            ))
                            .child(self.video_crop_field(
                                clip_id,
                                VideoTransformProperty::CropBottom,
                                "Bottom",
                                properties.crop_bottom,
                                "transform-crop-bottom",
                                editable,
                                cx,
                            )),
                    ),
            )
            .into_any_element()
    }

    #[allow(clippy::too_many_arguments)]
    fn video_transform_field(
        &self,
        clip_id: u64,
        property: VideoTransformProperty,
        label: &'static str,
        value: String,
        unit: &'static str,
        step: f64,
        field_id: &'static str,
        editable: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let decrement_property = property;
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
                    .id(field_id)
                    .h(px(48.0))
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(SURFACE))
                    .cursor(if editable {
                        CursorStyle::ResizeLeftRight
                    } else {
                        CursorStyle::Arrow
                    })
                    .child(
                        div()
                            .min_w_0()
                            .font_family("monospace")
                            .text_base()
                            .text_color(rgb(if editable { TEXT } else { MUTED }))
                            .child(value),
                    )
                    .child(div().text_sm().text_color(rgb(MUTED)).child(unit))
                    .when(editable, |field| {
                        field
                            .hover(|style| style.border_color(rgb(0x4a4a52)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |editor, _, _, cx| {
                                    editor.adjust_video_transform(clip_id, property, step);
                                    cx.notify();
                                }),
                            )
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |editor, _, _, cx| {
                                    editor.adjust_video_transform(
                                        clip_id,
                                        decrement_property,
                                        -step,
                                    );
                                    cx.notify();
                                }),
                            )
                    }),
            )
            .into_any_element()
    }

    fn video_crop_field(
        &self,
        clip_id: u64,
        property: VideoTransformProperty,
        label: &'static str,
        value: f64,
        field_id: &'static str,
        editable: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let decrement_property = property;
        div()
            .min_w_0()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .w(px(58.0))
                    .flex_shrink_0()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child(label),
            )
            .child(
                div()
                    .id(field_id)
                    .h(px(46.0))
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .rounded_md()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(SURFACE))
                    .cursor(if editable {
                        CursorStyle::ResizeLeftRight
                    } else {
                        CursorStyle::Arrow
                    })
                    .child(
                        div()
                            .font_family("monospace")
                            .text_sm()
                            .text_color(rgb(if editable { TEXT } else { MUTED }))
                            .child(format!("{:.1}%", value.clamp(0.0, 0.99) * 100.0)),
                    )
                    .when(editable, |field| {
                        field
                            .hover(|style| style.border_color(rgb(0x4a4a52)))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |editor, _, _, cx| {
                                    editor.adjust_video_transform(clip_id, property, 0.01);
                                    cx.notify();
                                }),
                            )
                            .on_mouse_down(
                                MouseButton::Right,
                                cx.listener(move |editor, _, _, cx| {
                                    editor.adjust_video_transform(
                                        clip_id,
                                        decrement_property,
                                        -0.01,
                                    );
                                    cx.notify();
                                }),
                            )
                    }),
            )
            .into_any_element()
    }

    fn video_opacity_control(
        &self,
        clip_id: u64,
        opacity: f64,
        editable: bool,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let opacity = opacity.clamp(0.0, 1.0);
        div()
            .h(px(42.0))
            .flex()
            .items_center()
            .gap_4()
            .child(
                div()
                    .w(px(112.0))
                    .flex_shrink_0()
                    .text_sm()
                    .text_color(rgb(MUTED))
                    .child("Opacity"),
            )
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        div()
                            .id("transform-opacity-slider")
                            .relative()
                            .h(px(24.0))
                            .min_w_0()
                            .flex_1()
                            .flex()
                            .items_center()
                            .cursor(if editable {
                                CursorStyle::PointingHand
                            } else {
                                CursorStyle::Arrow
                            })
                            .child(
                                div()
                                    .absolute()
                                    .left_0()
                                    .right_0()
                                    .h(px(4.0))
                                    .rounded_full()
                                    .bg(rgb(0x45454d)),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .left_0()
                                    .w(gpui::relative(opacity as f32))
                                    .h(px(4.0))
                                    .rounded_full()
                                    .bg(rgb(ACCENT)),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .left(gpui::relative(opacity as f32))
                                    .ml(px(-8.0))
                                    .size(px(16.0))
                                    .rounded_full()
                                    .bg(rgb(0xf7f7f8)),
                            )
                            .when(editable, |slider| {
                                slider
                                    .on_mouse_down(
                                        MouseButton::Left,
                                        cx.listener(move |editor, _, _, cx| {
                                            editor.adjust_video_transform(
                                                clip_id,
                                                VideoTransformProperty::Opacity,
                                                0.05,
                                            );
                                            cx.notify();
                                        }),
                                    )
                                    .on_mouse_down(
                                        MouseButton::Right,
                                        cx.listener(move |editor, _, _, cx| {
                                            editor.adjust_video_transform(
                                                clip_id,
                                                VideoTransformProperty::Opacity,
                                                -0.05,
                                            );
                                            cx.notify();
                                        }),
                                    )
                            }),
                    )
                    .child(
                        div()
                            .w(px(38.0))
                            .font_family("monospace")
                            .text_base()
                            .text_color(rgb(if editable { TEXT } else { MUTED }))
                            .child(format!("{:.0}", opacity * 100.0)),
                    ),
            )
            .into_any_element()
    }

    fn adjust_video_transform(
        &mut self,
        clip_id: u64,
        property: VideoTransformProperty,
        delta: f64,
    ) {
        if self.clip_locked(clip_id) {
            return;
        }
        let Some(index) = self.project.clip_index(clip_id) else {
            return;
        };
        let mut properties = self.project.clips[index].video_properties;
        match property {
            VideoTransformProperty::PositionX => properties.position_x += delta,
            VideoTransformProperty::PositionY => properties.position_y += delta,
            VideoTransformProperty::Scale => {
                properties.scale = (properties.scale + delta).clamp(0.0, 100.0)
            }
            VideoTransformProperty::Rotation => properties.rotation_degrees += delta,
            VideoTransformProperty::Opacity => {
                properties.opacity = (properties.opacity + delta).clamp(0.0, 1.0)
            }
            VideoTransformProperty::CropLeft => {
                properties.crop_left = (properties.crop_left + delta)
                    .clamp(0.0, 0.99 - properties.crop_right.clamp(0.0, 0.99))
            }
            VideoTransformProperty::CropRight => {
                properties.crop_right = (properties.crop_right + delta)
                    .clamp(0.0, 0.99 - properties.crop_left.clamp(0.0, 0.99))
            }
            VideoTransformProperty::CropTop => {
                properties.crop_top = (properties.crop_top + delta)
                    .clamp(0.0, 0.99 - properties.crop_bottom.clamp(0.0, 0.99))
            }
            VideoTransformProperty::CropBottom => {
                properties.crop_bottom = (properties.crop_bottom + delta)
                    .clamp(0.0, 0.99 - properties.crop_top.clamp(0.0, 0.99))
            }
        }
        if properties == self.project.clips[index].video_properties {
            return;
        }
        self.checkpoint();
        self.project.clips[index].video_properties = properties;
        self.preview_refresh_ticks = 2;
        self.save_project();
    }

    fn reset_video_crop(&mut self, clip_id: u64) {
        if self.clip_locked(clip_id) {
            return;
        }
        let Some(index) = self.project.clip_index(clip_id) else {
            return;
        };
        let mut properties = self.project.clips[index].video_properties;
        if properties.crop_left == 0.0
            && properties.crop_right == 0.0
            && properties.crop_top == 0.0
            && properties.crop_bottom == 0.0
        {
            return;
        }
        properties.crop_left = 0.0;
        properties.crop_right = 0.0;
        properties.crop_top = 0.0;
        properties.crop_bottom = 0.0;
        self.checkpoint();
        self.project.clips[index].video_properties = properties;
        self.preview_refresh_ticks = 2;
        self.save_project();
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
                this.child(properties_value("Duration", format_time(duration)))
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
                this.child(properties_value("Duration", format_time(duration)))
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

fn properties_tab(label: &'static str, active: bool) -> gpui::Div {
    div()
        .h_full()
        .flex()
        .items_center()
        .border_b_2()
        .border_color(if active { rgb(ACCENT) } else { rgb(0x00000000) })
        .text_sm()
        .text_color(rgb(if active { TEXT } else { MUTED }))
        .child(label)
}

fn properties_section_label(label: &'static str) -> gpui::Div {
    div()
        .text_xs()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(rgb(MUTED))
        .child(label)
}

fn crop_reset_button(enabled: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id("reset-video-crop")
        .h_8()
        .px_2()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .cursor(if enabled {
            CursorStyle::PointingHand
        } else {
            CursorStyle::Arrow
        })
        .text_sm()
        .text_color(rgb(if enabled { ACCENT } else { MUTED }))
        .when(enabled, |this| {
            this.hover(|style| style.bg(rgb(SURFACE_HOVER)))
        })
        .child("Reset")
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
    (width > 0 && height > 0).then(|| (width as u32, height as u32))
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
