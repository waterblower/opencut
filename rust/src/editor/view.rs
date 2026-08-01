use super::*;

const MAX_RULER_TICKS: usize = 240;
const TICK_STEPS: [f64; 10] = [1.0, 2.0, 5.0, 10.0, 15.0, 30.0, 60.0, 300.0, 600.0, 1800.0];

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let viewport = window.viewport_size();
        let preview_width =
            (f32::from(viewport.width) - MEDIA_PANEL_WIDTH - INSPECTOR_WIDTH).max(320.0);
        let preview_height =
            (f32::from(viewport.height) - TOPBAR_HEIGHT - TIMELINE_HEIGHT).max(240.0);

        if window.is_fullscreen() {
            return div()
                .id("editor-fullscreen-preview")
                .track_focus(&self.focus_handle)
                .on_action(cx.listener(Self::action_toggle_playback))
                .on_action(cx.listener(Self::action_toggle_fullscreen))
                .on_action(cx.listener(Self::action_exit_fullscreen))
                .on_action(cx.listener(Self::action_toggle_inspector))
                .size_full()
                .overflow_hidden()
                .bg(rgb(0x000000))
                .text_color(rgb(TEXT))
                .child(self.preview_player(
                    0.0,
                    0.0,
                    f32::from(viewport.width),
                    f32::from(viewport.height),
                    cx,
                ));
        }
        let file_menu = self
            .file_context_menu
            .as_ref()
            .map(|menu| self.file_menu_overlay(menu, viewport, cx));

        div()
            .id("editor-root")
            .track_focus(&self.focus_handle)
            .on_action(cx.listener(Self::action_toggle_playback))
            .on_action(cx.listener(Self::action_delete_selected))
            .on_action(cx.listener(Self::action_split_clip))
            .on_action(cx.listener(Self::action_undo))
            .on_action(cx.listener(Self::action_redo))
            .on_action(cx.listener(Self::action_duplicate_selected))
            .on_action(cx.listener(Self::action_add_marker))
            .on_action(cx.listener(Self::action_toggle_fullscreen))
            .on_action(cx.listener(Self::action_exit_fullscreen))
            .on_action(cx.listener(Self::action_toggle_inspector))
            .on_action(cx.listener(Self::action_reveal_in_finder))
            .on_action(cx.listener(Self::action_open_in_default_app))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _, _, cx| {
                    if editor.preview_volume_open {
                        editor.preview_volume_open = false;
                        cx.notify();
                    }
                }),
            )
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .overflow_hidden()
            .bg(rgb(BACKGROUND))
            .text_color(rgb(TEXT))
            .child(self.topbar(cx))
            .child(
                div()
                    .id("editor-workspace")
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .id("editor-upper-workspace")
                            .min_w_0()
                            .flex_1()
                            .min_h_0()
                            .flex()
                            .child(self.media_panel(cx))
                            .child(self.preview_player(
                                MEDIA_PANEL_WIDTH,
                                TOPBAR_HEIGHT,
                                preview_width,
                                preview_height,
                                cx,
                            ))
                            .child(self.inspector(cx)),
                    )
                    .child(self.timeline(cx)),
            )
            .when_some(file_menu, |this, menu| this.child(menu))
    }
}

impl Editor {
    fn topbar(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let undo_enabled = !self.undo_stack.is_empty();
        let redo_enabled = !self.redo_stack.is_empty();
        let export_enabled = !self.project.clips.is_empty() && !self.exporting;
        let has_error = self.error.is_some();
        let message = self
            .error
            .as_deref()
            .or(self.status.as_deref())
            .unwrap_or("Ready")
            .to_string();

        div()
            .id("editor-topbar")
            .h(px(TOPBAR_HEIGHT))
            .flex_shrink_0()
            .flex()
            .items_center()
            .justify_between()
            .px_5()
            .border_b_1()
            .border_color(rgb(BORDER))
            .bg(rgb(PANEL))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(gpui::FontWeight::BOLD)
                            .child("OpenCut"),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(rgb(MUTED))
                            .child("EDITOR · AUTOSAVED"),
                    ),
            )
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .px_5()
                    .flex()
                    .justify_center()
                    .child(
                        div()
                            .text_sm()
                            .text_ellipsis()
                            .text_color(if has_error { rgb(ERROR) } else { rgb(MUTED) })
                            .child(message),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(toolbar_button("Undo", undo_enabled).on_click(cx.listener(
                        |editor, _, _, cx| {
                            editor.undo();
                            cx.notify();
                        },
                    )))
                    .child(toolbar_button("Redo", redo_enabled).on_click(cx.listener(
                        |editor, _, _, cx| {
                            editor.redo();
                            cx.notify();
                        },
                    )))
                    .child(toolbar_button("Open Folder", true).on_click(cx.listener(
                        |editor, _, _, cx| {
                            editor.open_project_folder(cx);
                        },
                    )))
                    .child(
                        toolbar_button(
                            if self.exporting {
                                "Exporting…"
                            } else {
                                "Export MP4"
                            },
                            export_enabled,
                        )
                        .bg(rgb(ACCENT))
                        .text_color(rgb(0x17120a))
                        .on_click(cx.listener(|editor, _, _, cx| {
                            editor.export(cx);
                        })),
                    ),
            )
            .into_any_element()
    }

    fn media_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let project_name = self
            .project_root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.project_root.display().to_string());
        let entries = self
            .file_tree
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                let path = entry.relative_path.clone();
                let selection_path = path.clone();
                let action_path = path.clone();
                let context_path = path.clone();
                let selected = self.selected_file.as_ref() == Some(&path);
                let is_directory = entry.is_directory;
                let is_video = entry.is_video;
                let is_image = entry.is_image;
                let is_audio = entry.is_audio;
                let is_media = is_video || is_image || is_audio;
                let thumbnail_path = is_image.then(|| self.project_root.join(&path));
                let icon = if is_directory {
                    if entry.expanded { "▾" } else { "▸" }
                } else if is_video {
                    "▶"
                } else if is_audio {
                    "♪"
                } else {
                    "·"
                };
                div()
                    .id(("project-file", index))
                    .h(px(34.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .rounded_md()
                    .pr_2()
                    .pl(px(8.0 + entry.depth as f32 * 16.0))
                    .bg(rgb(if selected { 0x25221c } else { PANEL }))
                    .cursor(CursorStyle::PointingHand)
                    .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                    .on_click(cx.listener(move |editor, _, _, cx| {
                        if is_directory {
                            editor.toggle_directory(selection_path.clone());
                        } else {
                            editor.select_file(selection_path.clone(), cx);
                        }
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                            editor.show_file_context_menu(context_path.clone(), event, cx);
                        }),
                    )
                    .child(
                        div()
                            .size(px(18.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .overflow_hidden()
                            .rounded_sm()
                            .text_color(rgb(if is_media { ACCENT } else { MUTED }))
                            .when_some(thumbnail_path, |this, path| {
                                this.child(img(path).size_full().object_fit(ObjectFit::Cover))
                            })
                            .when(!is_image, |this| this.child(icon)),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .text_sm()
                            .text_ellipsis()
                            .text_color(rgb(if is_media || is_directory {
                                TEXT
                            } else {
                                MUTED
                            }))
                            .child(entry.name.clone()),
                    )
                    .when(is_media, |this| {
                        this.child(
                            div()
                                .id(("add-project-file", index))
                                .size_6()
                                .flex_shrink_0()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .occlude()
                                .bg(rgb(0x25252a))
                                .hover(|style| style.bg(rgb(ACCENT)).text_color(rgb(0x17120a)))
                                .child("+")
                                .on_click(cx.listener(move |editor, _, _, cx| {
                                    editor.add_file_to_timeline(action_path.clone(), cx);
                                })),
                        )
                    })
            })
            .collect::<Vec<_>>();

        div()
            .id("editor-media-panel")
            .w(px(MEDIA_PANEL_WIDTH))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(rgb(BORDER))
            .bg(rgb(PANEL))
            .child(
                div()
                    .h(px(62.0))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .justify_center()
                    .gap_1()
                    .justify_between()
                    .px_4()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .w_full()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_ellipsis()
                            .child(project_name),
                    )
                    .child(
                        div()
                            .w_full()
                            .text_xs()
                            .font_family("monospace")
                            .text_color(rgb(MUTED))
                            .text_ellipsis()
                            .child(self.project_root.display().to_string()),
                    ),
            )
            .child(
                div()
                    .id("editor-media-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.timeline_vertical_scroll)
                    .flex()
                    .flex_col()
                    .gap_2()
                    .p_3()
                    .when(entries.is_empty(), |this| {
                        this.child(
                            div()
                                .p_3()
                                .text_sm()
                                .text_color(rgb(MUTED))
                                .child("This project folder is empty."),
                        )
                    })
                    .children(entries),
            )
            .into_any_element()
    }

    fn file_menu_overlay(
        &self,
        menu: &FileContextMenu,
        viewport: gpui::Size<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let width = 268.0;
        let height = 92.0;
        let left = menu
            .x
            .clamp(8.0, (f32::from(viewport.width) - width - 8.0).max(8.0));
        let top = menu
            .y
            .clamp(8.0, (f32::from(viewport.height) - height - 8.0).max(8.0));

        div()
            .id("file-context-menu-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _, _, cx| {
                    editor.dismiss_file_context_menu();
                    cx.notify();
                }),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|editor, _, _, cx| {
                    editor.dismiss_file_context_menu();
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("file-context-menu")
                    .absolute()
                    .left(px(left))
                    .top(px(top))
                    .w(px(width))
                    .p_1()
                    .flex()
                    .flex_col()
                    .rounded_lg()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(0x1b1b1e))
                    .shadow_lg()
                    .occlude()
                    .child(
                        file_menu_item("Reveal in Finder", "⌥⌘R").on_click(cx.listener(
                            |editor, _, _, cx| {
                                editor.reveal_selected_file(cx);
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        file_menu_item("Open in Default App", "⌃⇧↵").on_click(cx.listener(
                            |editor, _, _, cx| {
                                editor.open_selected_file_in_default_app(cx);
                                cx.notify();
                            },
                        )),
                    ),
            )
            .into_any_element()
    }

    fn inspector(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
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
            .id("editor-inspector")
            .w(px(INSPECTOR_WIDTH))
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
                    .child("Inspector"),
            )
            .child(
                div()
                    .id("editor-inspector-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .p_4()
                    .when_some(selected, |this, (clip, asset, track)| {
                        let title = asset
                            .map(|asset| asset.name.clone())
                            .unwrap_or_else(|| "Missing media".to_string());
                        this.flex()
                            .flex_col()
                            .gap_4()
                            .child(
                                div()
                                    .text_lg()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .child(title),
                            )
                            .child(inspector_value(
                                "Timeline start",
                                format_time(clip.timeline_start),
                            ))
                            .child(inspector_value("Source in", format_time(clip.source_in)))
                            .child(inspector_value("Source out", format_time(clip.source_out)))
                            .child(inspector_value(
                                "Clip duration",
                                format_time(clip.duration()),
                            ))
                            .child(inspector_value("Track", track.name.clone()))
                            .when_some(asset, |this, asset| {
                                this.child(inspector_value("Source", asset_description(asset)))
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
                            .child("Select a timeline clip to inspect it.")
                    }),
            )
            .into_any_element()
    }

    fn timeline(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let duration = self.project.timeline_duration().max(12.0);
        let timeline_width =
            (duration as f32 * self.pixels_per_second + TIMELINE_PADDING * 2.0).max(900.0);
        let zoom_step = if self.pixels_per_second >= 120.0 {
            1.0
        } else if self.pixels_per_second >= 60.0 {
            2.0
        } else if self.pixels_per_second >= 36.0 {
            5.0
        } else {
            10.0
        };
        // The ruler is not virtualised, so coarsen the step until a long project stays
        // within a bounded number of labels rather than one per second.
        let tick_step = TICK_STEPS
            .iter()
            .copied()
            .find(|step| *step >= zoom_step && duration / step <= MAX_RULER_TICKS as f64)
            .unwrap_or(duration / MAX_RULER_TICKS as f64);
        let tick_count = (duration / tick_step).ceil() as usize + 1;
        let ruler_ticks = (0..tick_count).map(|index| {
            let time = index as f64 * tick_step;
            div()
                .absolute()
                .left(px(TIMELINE_PADDING + time as f32 * self.pixels_per_second))
                .top_0()
                .h_full()
                .border_l_1()
                .border_color(rgb(0x333338))
                .pl_1()
                .font_family("monospace")
                .text_xs()
                .text_color(rgb(MUTED))
                .child(format_time_precise(time))
        });
        let marker_elements = self
            .project
            .markers
            .iter()
            .enumerate()
            .map(|(index, marker)| {
                let marker_time = marker.time;
                div()
                    .id(("timeline-marker", index))
                    .absolute()
                    .left(px(TIMELINE_PADDING
                        + marker.time as f32 * self.pixels_per_second
                        - 4.0))
                    .top_0()
                    .size_2()
                    .bg(rgb(ACCENT))
                    .cursor(CursorStyle::PointingHand)
                    .on_click(cx.listener(move |editor, _, _, cx| {
                        editor.load_timeline_position(marker_time, false);
                        cx.notify();
                    }))
            });

        let track_headers =
            self.project
                .tracks
                .iter()
                .enumerate()
                .map(|(index, track)| {
                    let track_id = track.id;
                    div()
                        .id(("track-header", index))
                        .h(px(TRACK_HEIGHT))
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .justify_center()
                        .gap_2()
                        .px_3()
                        .border_b_1()
                        .border_color(rgb(BORDER))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .child(track.name.clone()),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(MUTED))
                                        .child(track_kind_label(track.kind)),
                                ),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_1()
                                .child(
                                    track_button(
                                        ("track-lock", index),
                                        if track.locked { "🔒" } else { "♢" },
                                    )
                                    .on_click(cx.listener(
                                        move |editor, _, _, cx| {
                                            editor.toggle_track_lock(track_id);
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(
                                    track_button(
                                        ("track-visible", index),
                                        if track.visible { "◉" } else { "○" },
                                    )
                                    .on_click(cx.listener(
                                        move |editor, _, _, cx| {
                                            editor.toggle_track_visibility(track_id);
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(
                                    track_button(
                                        ("track-mute", index),
                                        if track.muted { "M×" } else { "M" },
                                    )
                                    .on_click(cx.listener(
                                        move |editor, _, _, cx| {
                                            editor.toggle_track_mute(track_id);
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(track_button(("track-up", index), "↑").on_click(
                                    cx.listener(move |editor, _, _, cx| {
                                        editor.move_track(track_id, -1);
                                        cx.notify();
                                    }),
                                ))
                                .child(track_button(("track-down", index), "↓").on_click(
                                    cx.listener(move |editor, _, _, cx| {
                                        editor.move_track(track_id, 1);
                                        cx.notify();
                                    }),
                                ))
                                .child(track_button(("track-delete", index), "×").on_click(
                                    cx.listener(move |editor, _, _, cx| {
                                        editor.delete_track(track_id);
                                        cx.notify();
                                    }),
                                )),
                        )
                });

        let track_rows = self
            .project
            .tracks
            .iter()
            .enumerate()
            .map(|(track_index, track)| {
                let clip_elements = self
                    .project
                    .clips_on_track(track.id)
                    .map(|clip| {
                        let clip_id = clip.id;
                        let selected = self.selected_clip_id == Some(clip_id);
                        let asset = clip.asset_id.and_then(|id| self.project.asset(id));
                        let name = asset
                            .map(|asset| asset.name.clone())
                            .unwrap_or_else(|| "Missing media".to_string());
                        let left =
                            TIMELINE_PADDING + clip.timeline_start as f32 * self.pixels_per_second;
                        let width = (clip.duration() as f32 * self.pixels_per_second).max(4.0);
                        let color = match track.kind {
                            TrackKind::Video => CLIP_BLUE,
                            TrackKind::Audio => 0x24656b,
                        };
                        let cached =
                            asset.is_some_and(|asset| self.media_cache_ready.contains(&asset.id));
                        let thumbnail = asset.and_then(|asset| match asset.kind {
                            MediaKind::Image => Some(self.project_root.join(&asset.path)),
                            MediaKind::Video => cached
                                .then(|| media_cache::thumbnail_path(&self.project_root, asset)),
                            MediaKind::Audio => None,
                        });
                        let waveform = asset.and_then(|asset| {
                            (cached && asset.has_audio)
                                .then(|| media_cache::waveform_path(&self.project_root, asset))
                        });
                        let has_audio = asset.is_some_and(|asset| asset.has_audio);
                        div()
                            .id(("timeline-clip", clip_id))
                            .absolute()
                            .left(px(left))
                            .top(px(5.0))
                            .w(px(width))
                            .h(px(TRACK_HEIGHT - 10.0))
                            .overflow_hidden()
                            .rounded_md()
                            .border_2()
                            .border_color(rgb(if selected { ACCENT } else { color + 0x101010 }))
                            .bg(rgb(color))
                            .cursor(CursorStyle::PointingHand)
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                                    editor.begin_clip_move(clip_id, event, cx);
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .when_some(thumbnail, |this, path| {
                                        this.child(
                                            img(path)
                                                .size_full()
                                                .object_fit(ObjectFit::Cover)
                                                .opacity(0.45),
                                        )
                                    }),
                            )
                            .when_some(waveform, |this, path| {
                                this.child(
                                    div()
                                        .absolute()
                                        .left_0()
                                        .right_0()
                                        .bottom_0()
                                        .h(px(24.0))
                                        .opacity(0.82)
                                        .child(img(path).size_full().object_fit(ObjectFit::Fill)),
                                )
                            })
                            .child(
                                div()
                                    .absolute()
                                    .inset_0()
                                    .p_2()
                                    .flex()
                                    .flex_col()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_ellipsis()
                                            .child(name),
                                    )
                                    .child(
                                        div()
                                            .font_family("monospace")
                                            .text_xs()
                                            .text_color(rgb(0xc8d8e8))
                                            .child(if has_audio {
                                                "Audio".to_string()
                                            } else {
                                                format!("{}s", clip.duration().round())
                                            }),
                                    ),
                            )
                            .child(trim_handle(("left-trim", clip_id), true).on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                    editor.begin_trim(
                                        clip_id,
                                        TrimEdge::Left,
                                        event.position.x.into(),
                                    );
                                    cx.notify();
                                }),
                            ))
                            .child(trim_handle(("right-trim", clip_id), false).on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                    editor.begin_trim(
                                        clip_id,
                                        TrimEdge::Right,
                                        event.position.x.into(),
                                    );
                                    cx.notify();
                                }),
                            ))
                    })
                    .collect::<Vec<_>>();
                div()
                    .id(("track-row", track_index))
                    .relative()
                    .w(px(timeline_width))
                    .h(px(TRACK_HEIGHT))
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(if track_index % 2 == 0 {
                        0x101012
                    } else {
                        0x0d0d0f
                    }))
                    .children(clip_elements)
            });
        let playhead_left = TIMELINE_PADDING + self.playhead as f32 * self.pixels_per_second;

        div()
            .id("editor-timeline")
            .h(px(TIMELINE_HEIGHT))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(rgb(BORDER))
            .bg(rgb(0x0a0a0c))
            .on_mouse_move(cx.listener(Self::update_trim))
            .on_mouse_move(cx.listener(Self::update_clip_move))
            .on_mouse_move(cx.listener(Self::update_playhead_scrub))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_trim))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_clip_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::finish_playhead_scrub))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_trim))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_clip_move))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::finish_playhead_scrub))
            .child(
                div()
                    .h(px(TIMELINE_HEADER_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .border_b_1()
                    .border_color(rgb(BORDER))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                timeline_icon_button(
                                    "timeline-play",
                                    if self.playing { "Ⅱ" } else { "▶" },
                                )
                                .on_click(cx.listener(
                                    |editor, _, _, cx| {
                                        editor.toggle_playback();
                                        cx.notify();
                                    },
                                )),
                            )
                            .child(div().w(px(108.0)).font_family("monospace").text_sm().child(
                                format!(
                                    "{} / {}",
                                    format_time(self.playhead),
                                    format_time(self.project.timeline_duration())
                                ),
                            ))
                            .child(timeline_icon_button("add-video-track", "+V").on_click(
                                cx.listener(|editor, _, _, cx| {
                                    editor.add_track(TrackKind::Video);
                                    cx.notify();
                                }),
                            ))
                            .child(timeline_icon_button("add-audio-track", "+A").on_click(
                                cx.listener(|editor, _, _, cx| {
                                    editor.add_track(TrackKind::Audio);
                                    cx.notify();
                                }),
                            ))
                            .child(
                                timeline_icon_button("add-marker", "◆").on_click(cx.listener(
                                    |editor, _, _, cx| {
                                        editor.add_marker();
                                        cx.notify();
                                    },
                                )),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(timeline_icon_button("zoom-out", "−").on_click(cx.listener(
                                |editor, _, _, cx| {
                                    editor.zoom(0.8);
                                    cx.notify();
                                },
                            )))
                            .child(
                                div()
                                    .w(px(58.0))
                                    .text_center()
                                    .font_family("monospace")
                                    .text_xs()
                                    .text_color(rgb(MUTED))
                                    .child(format!("{:.0}px/s", self.pixels_per_second)),
                            )
                            .child(timeline_icon_button("zoom-in", "+").on_click(cx.listener(
                                |editor, _, _, cx| {
                                    editor.zoom(1.25);
                                    cx.notify();
                                },
                            ))),
                    ),
            )
            .child(
                div()
                    .id("timeline-tracks-vertical-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(
                        div()
                            .h(px(
                                RULER_HEIGHT + self.project.tracks.len() as f32 * TRACK_HEIGHT,
                            ))
                            .w_full()
                            .flex()
                            .child(
                                div()
                                    .w(px(TRACK_HEADER_WIDTH))
                                    .h_full()
                                    .flex_shrink_0()
                                    .flex()
                                    .flex_col()
                                    .border_r_1()
                                    .border_color(rgb(BORDER))
                                    .child(
                                        div()
                                            .h(px(RULER_HEIGHT))
                                            .flex_shrink_0()
                                            .border_b_1()
                                            .border_color(rgb(BORDER)),
                                    )
                                    .children(track_headers),
                            )
                            .child(
                                div()
                                    .id("editor-timeline-scroll")
                                    .min_w_0()
                                    .flex_1()
                                    .h_full()
                                    .overflow_x_scroll()
                                    .track_scroll(&self.timeline_scroll)
                                    .child(
                                        div()
                                            .relative()
                                            .w(px(timeline_width))
                                            .min_h_full()
                                            .child(
                                                div()
                                                    .id("timeline-seek-ruler")
                                                    .relative()
                                                    .w_full()
                                                    .h(px(RULER_HEIGHT))
                                                    .border_b_1()
                                                    .border_color(rgb(BORDER))
                                                    .cursor(CursorStyle::PointingHand)
                                                    .children(ruler_ticks)
                                                    .children(marker_elements)
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            |editor,
                                                             event: &MouseDownEvent,
                                                             _,
                                                             cx| {
                                                                editor.begin_playhead_scrub(event);
                                                                cx.notify();
                                                            },
                                                        ),
                                                    ),
                                            )
                                            .children(track_rows)
                                            .child(
                                                div()
                                                    .absolute()
                                                    .top_0()
                                                    .bottom_0()
                                                    .left(px(playhead_left))
                                                    .w(px(2.0))
                                                    .bg(rgb(ACCENT))
                                                    .cursor(CursorStyle::ResizeLeftRight)
                                                    .on_mouse_down(
                                                        MouseButton::Left,
                                                        cx.listener(
                                                            |editor,
                                                             event: &MouseDownEvent,
                                                             _,
                                                             cx| {
                                                                editor.begin_playhead_scrub(event);
                                                                cx.stop_propagation();
                                                                cx.notify();
                                                            },
                                                        ),
                                                    )
                                                    .child(
                                                        div()
                                                            .absolute()
                                                            .top_0()
                                                            .left(px(-4.0))
                                                            .size_2()
                                                            .bg(rgb(ACCENT)),
                                                    ),
                                            ),
                                    )
                            ),
                    ),
            )
            .into_any_element()
    }
}

fn timeline_icon_button(
    id: impl Into<gpui::ElementId>,
    label: &'static str,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h_7()
        .min_w(px(28.0))
        .px_2()
        .flex()
        .items_center()
        .justify_center()
        .rounded_md()
        .bg(rgb(SURFACE))
        .cursor(CursorStyle::PointingHand)
        .text_xs()
        .hover(|style| style.bg(rgb(SURFACE_HOVER)))
        .child(label)
}

fn track_button(id: impl Into<gpui::ElementId>, label: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h_5()
        .min_w(px(24.0))
        .px_1()
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .bg(rgb(SURFACE))
        .cursor(CursorStyle::PointingHand)
        .text_xs()
        .hover(|style| style.bg(rgb(SURFACE_HOVER)))
        .child(label)
}

fn track_kind_label(kind: TrackKind) -> &'static str {
    match kind {
        TrackKind::Video => "V",
        TrackKind::Audio => "A",
    }
}

fn format_time_precise(seconds: f64) -> String {
    let minutes = (seconds / 60.0).floor() as u64;
    let seconds = seconds % 60.0;
    format!("{minutes:02}:{seconds:04.1}")
}

fn toolbar_button(label: &'static str, enabled: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(label)
        .h_9()
        .px_3()
        .flex()
        .items_center()
        .justify_center()
        .rounded_lg()
        .border_1()
        .border_color(rgb(BORDER))
        .bg(rgb(SURFACE))
        .cursor(if enabled {
            CursorStyle::PointingHand
        } else {
            CursorStyle::Arrow
        })
        .text_sm()
        .text_color(rgb(if enabled { TEXT } else { 0x55555d }))
        .when(enabled, |this| {
            this.hover(|style| style.bg(rgb(SURFACE_HOVER)))
        })
        .child(label.to_string())
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

fn file_menu_item(label: &'static str, shortcut: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(label)
        .h(px(40.0))
        .px_3()
        .flex()
        .items_center()
        .justify_between()
        .rounded_md()
        .cursor(CursorStyle::PointingHand)
        .hover(|style| style.bg(rgb(0x34343a)))
        .child(div().text_sm().child(label))
        .child(
            div()
                .font_family("monospace")
                .text_sm()
                .text_color(rgb(MUTED))
                .child(shortcut),
        )
}

fn inspector_value(label: &str, value: String) -> gpui::Div {
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

fn trim_handle(id: impl Into<gpui::ElementId>, left: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .absolute()
        .top_0()
        .bottom_0()
        .when(left, |this| this.left_0())
        .when(!left, |this| this.right_0())
        .w(px(10.0))
        .bg(rgb(ACCENT))
        .opacity(0.72)
        .cursor(CursorStyle::ResizeLeftRight)
        .occlude()
        .hover(|style| style.opacity(1.0))
}
