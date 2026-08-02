use super::*;
use std::path::Path;

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
            .w(px(PROPERTIES_PANEL_WIDTH))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_l_1()
            .border_color(rgb(BORDER))
            .bg(rgb(PANEL))
            .child(
                div()
                    .h(px(52.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .px_4()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("Properties"),
            )
            .child(
                div()
                    .id("editor-properties-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .child(content),
            )
            .into_any_element()
    }

    fn timeline_properties(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let selected = self.selected_clip_id.and_then(|id| {
            let index = self.project.clip_index(id)?;
            let clip = &self.project.clips[index];
            let asset = clip
                .asset_id
                .and_then(|asset_id| self.project.asset(asset_id));
            let track = self.project.track(clip.track_id)?;
            Some((clip, asset, track))
        });

        div()
            .id("timeline-properties")
            .when_some(selected, |this, (clip, asset, track)| {
                let title = asset
                    .map(|asset| asset.name.clone())
                    .unwrap_or_else(|| "Missing media".to_string());
                this.flex()
                    .flex_col()
                    .gap_4()
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
                    .child(
                        div()
                            .mt_2()
                            .flex()
                            .gap_2()
                            .child(panel_button("Nudge left").on_click(cx.listener(
                                |editor, _, _, cx| {
                                    editor.move_selected(-1);
                                    cx.notify();
                                },
                            )))
                            .child(panel_button("Nudge right").on_click(cx.listener(
                                |editor, _, _, cx| {
                                    editor.move_selected(1);
                                    cx.notify();
                                },
                            ))),
                    )
                    .child(panel_button("Split at playhead").on_click(cx.listener(
                        |editor, _, _, cx| {
                            editor.split_selected();
                            cx.notify();
                        },
                    )))
                    .child(panel_button("Duplicate clip").on_click(cx.listener(
                        |editor, _, _, cx| {
                            editor.duplicate_selected();
                            cx.notify();
                        },
                    )))
                    .child(panel_button("Delete clip").text_color(rgb(ERROR)).on_click(
                        cx.listener(|editor, _, _, cx| {
                            editor.delete_selected();
                            cx.notify();
                        }),
                    ))
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

fn panel_button(label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(label)
        .h_9()
        .px_3()
        .flex_1()
        .flex()
        .items_center()
        .justify_center()
        .rounded_lg()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(SURFACE))
        .cursor(CursorStyle::PointingHand)
        .text_sm()
        .hover(|style| style.bg(rgb(SURFACE_HOVER)))
        .child(label.to_string())
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
