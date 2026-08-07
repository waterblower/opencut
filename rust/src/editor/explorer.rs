use super::*;

#[path = "explorer_file_menu.rs"]
mod explorer_file_menu;
pub(crate) use explorer_file_menu::FileContextMenu;

pub(super) struct RenameDialogState {
    relative_path: PathBuf,
    input: Entity<ExplorerFilter>,
}

pub(super) struct NewTimelineDialogState {
    relative_directory: PathBuf,
    input: Entity<ExplorerFilter>,
}

#[derive(Clone, Debug)]
pub(super) struct ExplorerMediaDrag {
    pub(super) relative_path: PathBuf,
    pub(super) name: String,
    pub(super) kind: MediaKind,
}

#[derive(Clone, Debug)]
pub(super) struct ExplorerDropPreview {
    pub(super) relative_path: PathBuf,
    pub(super) name: String,
    pub(super) track_id: u64,
    pub(super) raw_start: TimelineTime,
    pub(super) start: TimelineTime,
    pub(super) duration: TimelineTime,
    pub(super) analyzing: bool,
    pub(super) invalid_reason: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct PendingExplorerDrop {
    relative_path: PathBuf,
    track_id: u64,
    raw_start: TimelineTime,
}

struct ExplorerDragView {
    name: String,
    kind: MediaKind,
}

impl Render for ExplorerDragView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let label = match self.kind {
            MediaKind::Video => "VIDEO",
            MediaKind::Image => "IMAGE",
            MediaKind::Audio => "AUDIO",
        };
        div()
            .max_w(px(280.0))
            .h_9()
            .px_3()
            .flex()
            .items_center()
            .gap_2()
            .rounded_md()
            .border_1()
            .border_color(rgb(ACCENT))
            .bg(rgb(0x1b1b1e))
            .shadow_lg()
            .child(
                div()
                    .font_family("monospace")
                    .text_xs()
                    .text_color(rgb(ACCENT))
                    .child(label),
            )
            .child(
                div()
                    .min_w_0()
                    .text_sm()
                    .text_ellipsis()
                    .child(self.name.clone()),
            )
    }
}

impl Editor {
    pub(super) fn media_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let project_name = self
            .project_root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.project_root.display().to_string());
        let filter_query = self.explorer_filter.read(cx).query().to_string();
        let filter = filter_query.trim().to_lowercase();
        let show_root_contents = self.explorer_root_expanded || !filter.is_empty();
        let visible_entries = if filter.is_empty() {
            &self.file_tree
        } else {
            &self.explorer_search_results
        };
        let root_context_path = PathBuf::new();
        let root_row = div()
            .id("project-root")
            .h(px(38.0))
            .flex_shrink_0()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .cursor(CursorStyle::PointingHand)
            .hover(|style| style.bg(rgb(SURFACE_HOVER)))
            .on_click(cx.listener(|editor, _, _, cx| {
                editor.explorer_root_expanded = !editor.explorer_root_expanded;
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                    editor.show_file_context_menu(root_context_path.clone(), true, event, cx);
                }),
            )
            .child(
                div()
                    .w(px(14.0))
                    .flex_shrink_0()
                    .text_color(rgb(MUTED))
                    .child(if show_root_contents { "▾" } else { "▸" }),
            )
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .font_family("monospace")
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_ellipsis()
                    .child(project_name),
            );
        let entries = visible_entries
            .iter()
            .enumerate()
            .filter(|_| show_root_contents)
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
                let is_timeline = entry.is_timeline;
                let is_media = is_video || is_image || is_audio;
                let media_drag = is_media.then(|| ExplorerMediaDrag {
                    relative_path: path.clone(),
                    name: entry.name.clone(),
                    kind: if is_audio {
                        MediaKind::Audio
                    } else if is_image {
                        MediaKind::Image
                    } else {
                        MediaKind::Video
                    },
                });
                let metadata = if is_timeline && path == self.timeline_path {
                    Some("ACTIVE".to_string())
                } else {
                    explorer_metadata(
                        entry,
                        self.project.assets.iter().find(|asset| asset.path == path),
                    )
                };
                div()
                    .id(("project-file", index))
                    .relative()
                    .h(px(38.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .gap_2()
                    .pr_2()
                    .pl(px(10.0 + (entry.depth + 1) as f32 * 16.0))
                    .bg(rgb(if selected { 0x1e1b13 } else { PANEL }))
                    .cursor(CursorStyle::PointingHand)
                    .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                    .when_some(media_drag, |this, drag| {
                        this.cursor(CursorStyle::OpenHand).on_drag(
                            drag,
                            |drag: &ExplorerMediaDrag, _, _, cx| {
                                let drag = drag.clone();
                                cx.new(|_| ExplorerDragView {
                                    name: drag.name,
                                    kind: drag.kind,
                                })
                            },
                        )
                    })
                    .on_click(cx.listener(move |editor, _, _, cx| {
                        if is_directory {
                            editor.toggle_directory(selection_path.clone());
                        } else if is_timeline {
                            editor.open_timeline(selection_path.clone(), cx);
                        } else {
                            editor.select_file(selection_path.clone(), cx);
                        }
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                            editor.show_file_context_menu(
                                context_path.clone(),
                                is_directory,
                                event,
                                cx,
                            );
                        }),
                    )
                    .when(selected, |this| {
                        this.child(
                            div()
                                .absolute()
                                .left_0()
                                .top_0()
                                .bottom_0()
                                .w(px(2.0))
                                .bg(rgb(ACCENT)),
                        )
                    })
                    .child(
                        div()
                            .w(px(if is_directory { 14.0 } else { 38.0 }))
                            .h(px(20.0))
                            .flex_shrink_0()
                            .flex()
                            .items_center()
                            .justify_center()
                            .when(is_directory, |this| {
                                this.text_color(rgb(MUTED)).child(if entry.expanded {
                                    "▾"
                                } else {
                                    "▸"
                                })
                            })
                            .when(!is_directory, |this| this.child(explorer_file_badge(entry))),
                    )
                    .child(
                        div()
                            .min_w_0()
                            .flex_1()
                            .text_sm()
                            .font_family("monospace")
                            .text_ellipsis()
                            .text_color(rgb(if is_media || is_timeline || is_directory {
                                TEXT
                            } else {
                                MUTED
                            }))
                            .child(entry.name.clone()),
                    )
                    .when(is_media && selected, |this| {
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
                                .text_color(rgb(MUTED))
                                .hover(|style| style.bg(rgb(ACCENT)).text_color(rgb(0x17120a)))
                                .child("+")
                                .on_click(cx.listener(move |editor, _, _, cx| {
                                    editor.add_file_to_timeline(action_path.clone(), cx);
                                    cx.stop_propagation();
                                })),
                        )
                    })
                    .when_some(metadata, |this, metadata| {
                        this.child(
                            div()
                                .max_w(px(58.0))
                                .flex_shrink_0()
                                .font_family("monospace")
                                .text_xs()
                                .text_ellipsis()
                                .text_color(rgb(0x55555e))
                                .child(metadata),
                        )
                    })
            })
            .collect::<Vec<_>>();

        div()
            .id("editor-media-panel")
            .w(px(MEDIA_PANEL_WIDTH))
            .h_full()
            .flex_shrink_0()
            .border_r_1()
            .border_color(rgb(BORDER))
            .bg(rgb(PANEL))
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .bg(rgb(PANEL))
                    .child(self.explorer_filter())
                    .child(
                        div()
                            .id("editor-media-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.explorer_scroll)
                            .flex()
                            .flex_col()
                            .py_2()
                            .child(root_row)
                            .when(show_root_contents && entries.is_empty(), |this| {
                                this.child(div().p_4().text_sm().text_color(rgb(MUTED)).child(
                                    if filter.is_empty() {
                                        "This project folder is empty.".to_string()
                                    } else if self.explorer_search_pending {
                                        "Searching project…".to_string()
                                    } else {
                                        format!("No files match “{filter_query}”.")
                                    },
                                ))
                            })
                            .children(entries),
                    ),
            )
            .into_any_element()
    }

    fn explorer_filter(&self) -> gpui::AnyElement {
        self.explorer_filter.clone().into_any_element()
    }

    pub(super) fn schedule_explorer_search(&mut self, cx: &mut Context<Self>) {
        let query = self.explorer_filter.read(cx).query().trim().to_string();
        if query.is_empty() {
            self.explorer_search_query = None;
            self.explorer_search_results.clear();
            self.explorer_search_pending = false;
            return;
        }
        if self.explorer_search_query.as_deref() == Some(query.as_str()) {
            return;
        }

        self.explorer_search_query = Some(query.clone());
        self.explorer_search_results.clear();
        self.explorer_search_pending = true;

        let project_root = self.project_root.clone();
        cx.spawn(async move |editor, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;

            let still_requested = editor
                .update(cx, |editor, _| {
                    editor.project_root == project_root
                        && editor.explorer_search_query.as_deref() == Some(query.as_str())
                })
                .unwrap_or(false);
            if !still_requested {
                return;
            }

            let scan_root = project_root.clone();
            let scan_query = query.clone();
            let result = cx
                .background_executor()
                .spawn(async move { workspace::search_tree(&scan_root, &scan_query) })
                .await;

            editor
                .update(cx, |editor, cx| {
                    if editor.project_root != project_root
                        || editor.explorer_search_query.as_deref() != Some(query.as_str())
                    {
                        return;
                    }
                    editor.explorer_search_pending = false;
                    match result {
                        Ok(entries) => {
                            editor.explorer_search_results = entries;
                        }
                        Err(error) => {
                            editor.explorer_search_results.clear();
                            editor.error = Some(error);
                        }
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    pub(super) fn rename_dialog(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let state = self
            .rename_dialog_state
            .as_ref()
            .expect("rename dialog rendered without state");
        let input = state.input.clone();
        let original_name = state
            .relative_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();

        div()
            .id("rename-dialog-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x00000088))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _, _, cx| {
                    editor.rename_dialog_state = None;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("rename-dialog")
                    .w(px(480.0))
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .rounded_xl()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(PANEL))
                    .shadow_lg()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(format!("Rename {original_name}")),
                    )
                    .child(input)
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(rename_dialog_button("Cancel", false).on_click(cx.listener(
                                |editor, _, _, cx| {
                                    editor.rename_dialog_state = None;
                                    cx.notify();
                                },
                            )))
                            .child(rename_dialog_button("Rename", true).on_click(cx.listener(
                                |editor, _, _, cx| {
                                    editor.finish_rename(cx);
                                    cx.notify();
                                },
                            ))),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn new_timeline_dialog(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let state = self
            .new_timeline_dialog_state
            .as_ref()
            .expect("new timeline dialog rendered without state");
        let input = state.input.clone();
        let location = if state.relative_directory.as_os_str().is_empty() {
            self.project_root
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "project root".to_string())
        } else {
            state.relative_directory.display().to_string()
        };

        div()
            .id("new-timeline-dialog-overlay")
            .absolute()
            .inset_0()
            .occlude()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::rgba(0x00000088))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|editor, _, _, cx| {
                    editor.new_timeline_dialog_state = None;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("new-timeline-dialog")
                    .w(px(480.0))
                    .p_5()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .rounded_xl()
                    .border_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(PANEL))
                    .shadow_lg()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child("New timeline"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(rgb(MUTED))
                            .text_ellipsis()
                            .child(format!("In {location} · saved as .timeline.json")),
                    )
                    .child(input)
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(rename_dialog_button("Cancel", false).on_click(cx.listener(
                                |editor, _, _, cx| {
                                    editor.new_timeline_dialog_state = None;
                                    cx.notify();
                                },
                            )))
                            .child(rename_dialog_button("Create", true).on_click(cx.listener(
                                |editor, _, _, cx| {
                                    editor.finish_create_timeline(cx);
                                    cx.notify();
                                },
                            ))),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn refresh_file_tree(&mut self) {
        self.last_tree_scan = Instant::now();
        match visible_tree(&self.project_root, &self.expanded_directories) {
            Ok(entries) => self.file_tree = entries,
            Err(error) => self.error = Some(error),
        }
    }

    fn toggle_directory(&mut self, relative_path: PathBuf) {
        if !self.expanded_directories.remove(&relative_path) {
            self.expanded_directories.insert(relative_path);
        }
        self.refresh_file_tree();
    }

    pub(super) fn update_explorer_media_drag(
        &mut self,
        event: &DragMoveEvent<ExplorerMediaDrag>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let pointer = event.event.position;
        let inside_timeline = event.bounds.contains(&pointer);
        let local_y = f32::from(pointer.y) - f32::from(event.bounds.top());
        let track_index = ((local_y - RULER_HEIGHT) / TRACK_HEIGHT).floor() as isize;
        let Some(track_id) = inside_timeline
            .then_some(track_index)
            .and_then(|index| usize::try_from(index).ok())
            .and_then(|index| self.project.tracks.get(index))
            .map(|track| track.id)
        else {
            if self.explorer_drop_preview.take().is_some() {
                self.snap_guide = None;
                cx.notify();
            }
            cx.set_active_drag_cursor_style(CursorStyle::OperationNotAllowed, window);
            return;
        };

        let drag = event.drag(cx).clone();
        let local_x = f32::from(pointer.x) - f32::from(event.bounds.left());
        let raw_start = self
            .project
            .nearest_time(((local_x - TIMELINE_PADDING) / self.pixels_per_second).max(0.0) as f64);
        self.refresh_explorer_drop_preview(&drag, track_id, raw_start);
        let invalid = self
            .explorer_drop_preview
            .as_ref()
            .is_none_or(|preview| preview.invalid_reason.is_some());
        cx.set_active_drag_cursor_style(
            if invalid {
                CursorStyle::OperationNotAllowed
            } else {
                CursorStyle::DragCopy
            },
            window,
        );
        if !self.explorer_drag_probe_jobs.contains(&drag.relative_path)
            && self.explorer_asset_for_path(&drag.relative_path).is_none()
        {
            self.request_explorer_drag_probe(drag.relative_path.clone(), cx);
        }
        cx.notify();
    }

    fn refresh_explorer_drop_preview(
        &mut self,
        drag: &ExplorerMediaDrag,
        track_id: u64,
        raw_start: TimelineTime,
    ) {
        let asset = self.explorer_asset_for_path(&drag.relative_path).cloned();
        let analyzing = asset.is_none();
        let duration = asset
            .as_ref()
            .map(|asset| self.project.ceil_time(asset.duration))
            .unwrap_or_else(|| self.project.ceil_time(DEFAULT_IMAGE_CLIP_DURATION));
        let (start, snap_guide) =
            self.snap_clip_start_ignoring(raw_start, duration, &HashSet::new());
        let kind = asset.as_ref().map_or(drag.kind, |asset| asset.kind);
        let invalid_reason = validate_clip_placement(
            &self.project,
            track_id,
            kind,
            duration,
            start,
            &HashSet::new(),
        )
        .err()
        .map(|rejection| rejection.message().to_string());
        self.snap_guide = snap_guide;
        self.explorer_drop_preview = Some(ExplorerDropPreview {
            relative_path: drag.relative_path.clone(),
            name: drag.name.clone(),
            track_id,
            raw_start,
            start,
            duration,
            analyzing,
            invalid_reason,
        });
    }

    pub(super) fn drop_explorer_media(&mut self, drag: &ExplorerMediaDrag, cx: &mut Context<Self>) {
        let Some(preview) = self.explorer_drop_preview.take().filter(|preview| {
            preview.relative_path == drag.relative_path
                && self.project.track(preview.track_id).is_some()
        }) else {
            self.snap_guide = None;
            return;
        };
        self.snap_guide = None;

        if let Some(reason) = preview.invalid_reason {
            self.error = Some(format!("Cannot add {}: {reason}.", drag.name));
            self.status = None;
            cx.notify();
            return;
        }

        if let Some(asset) = self.explorer_asset_for_path(&drag.relative_path).cloned() {
            self.place_explorer_asset(
                drag.relative_path.clone(),
                preview.track_id,
                preview.raw_start,
                asset,
            );
        } else {
            self.pending_explorer_drop = Some(PendingExplorerDrop {
                relative_path: drag.relative_path.clone(),
                track_id: preview.track_id,
                raw_start: preview.raw_start,
            });
            self.status = Some(format!("Inspecting {} before placing it…", drag.name));
            self.error = None;
            self.request_explorer_drag_probe(drag.relative_path.clone(), cx);
        }
        cx.notify();
    }

    fn explorer_asset_for_path(&self, relative_path: &std::path::Path) -> Option<&MediaAsset> {
        self.project
            .assets
            .iter()
            .find(|asset| asset.path == relative_path)
            .or_else(|| self.explorer_drag_assets.get(relative_path))
    }

    fn request_explorer_drag_probe(&mut self, relative_path: PathBuf, cx: &mut Context<Self>) {
        if self.explorer_asset_for_path(&relative_path).is_some()
            || !self.explorer_drag_probe_jobs.insert(relative_path.clone())
        {
            return;
        }

        let project_root = self.project_root.clone();
        let source_path = project_root.join(&relative_path);
        cx.spawn(async move |editor, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { probe_asset(&source_path, 0) })
                .await;

            editor
                .update(cx, |editor, cx| {
                    if editor.project_root != project_root {
                        return;
                    }
                    editor.explorer_drag_probe_jobs.remove(&relative_path);
                    match result {
                        Ok(mut asset) => {
                            asset.path = relative_path.clone();
                            editor
                                .explorer_drag_assets
                                .insert(relative_path.clone(), asset.clone());

                            if let Some(preview) = editor
                                .explorer_drop_preview
                                .as_ref()
                                .filter(|preview| preview.relative_path == relative_path)
                                .cloned()
                            {
                                let drag = ExplorerMediaDrag {
                                    relative_path: relative_path.clone(),
                                    name: asset.name.clone(),
                                    kind: asset.kind,
                                };
                                editor.refresh_explorer_drop_preview(
                                    &drag,
                                    preview.track_id,
                                    preview.raw_start,
                                );
                            }

                            let pending_matches = editor
                                .pending_explorer_drop
                                .as_ref()
                                .is_some_and(|pending| pending.relative_path == relative_path);
                            if pending_matches
                                && let Some(pending) = editor.pending_explorer_drop.take()
                            {
                                editor.place_explorer_asset(
                                    relative_path.clone(),
                                    pending.track_id,
                                    pending.raw_start,
                                    asset,
                                );
                            }
                        }
                        Err(error) => {
                            if let Some(preview) = editor
                                .explorer_drop_preview
                                .as_mut()
                                .filter(|preview| preview.relative_path == relative_path)
                            {
                                preview.analyzing = false;
                                preview.invalid_reason = Some(error.clone());
                            }
                            if editor
                                .pending_explorer_drop
                                .as_ref()
                                .is_some_and(|pending| pending.relative_path == relative_path)
                            {
                                editor.pending_explorer_drop = None;
                                editor.status = None;
                                editor.error = Some(error);
                            }
                        }
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }

    fn place_explorer_asset(
        &mut self,
        relative_path: PathBuf,
        track_id: u64,
        raw_start: TimelineTime,
        mut asset: MediaAsset,
    ) {
        let duration = self.project.ceil_time(asset.duration);
        let (start, _) = self.snap_clip_start_ignoring(raw_start, duration, &HashSet::new());
        if let Err(rejection) = validate_clip_placement(
            &self.project,
            track_id,
            asset.kind,
            duration,
            start,
            &HashSet::new(),
        ) {
            let reason = rejection.message();
            self.status = None;
            self.error = Some(format!("Cannot add {}: {reason}.", asset.name));
            return;
        }

        self.checkpoint();
        let asset_id = if let Some(asset_id) = self
            .project
            .assets
            .iter()
            .find(|existing| existing.path == relative_path)
            .map(|existing| existing.id)
        {
            asset_id
        } else {
            asset.id = self.take_id();
            asset.path = relative_path.clone();
            let asset_id = asset.id;
            self.project.assets.push(asset);
            asset_id
        };
        let clip_id = self.take_id();
        self.project.clips.push(TimelineClip {
            id: clip_id,
            track_id,
            asset_id,
            timeline_start: start,
            source_in: TimelineTime::ZERO,
            source_out: duration,
            video_properties: VideoClipProperties::default(),
            audio_properties: AudioClipProperties::default(),
        });
        self.preview_target = PreviewTarget::Timeline;
        self.load_timeline_position(self.playhead, false);
        self.selected_file = Some(relative_path);
        self.selected_asset_id = Some(asset_id);
        self.select_only_clip(Some(clip_id));
        self.save_project();
        self.status = Some("Added media at the selected timeline position.".to_string());
        self.error = None;
    }

    fn add_file_to_timeline(&mut self, relative_path: PathBuf, cx: &mut Context<Self>) {
        if let Some(asset_id) = self
            .project
            .assets
            .iter()
            .find(|asset| asset.path == relative_path)
            .map(|asset| asset.id)
        {
            self.append_asset_clip(asset_id);
            cx.notify();
            return;
        }

        let project_root = self.project_root.clone();
        let absolute_path = project_root.join(&relative_path);
        self.status = Some(format!("Inspecting {}…", relative_path.display()));
        cx.spawn(async move |editor, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { probe_asset(&absolute_path, 0) })
                .await;
            editor
                .update(cx, |editor, cx| {
                    if editor.project_root != project_root {
                        return;
                    }
                    match result {
                        Ok(mut asset) => {
                            if let Some(asset_id) = editor
                                .project
                                .assets
                                .iter()
                                .find(|asset| asset.path == relative_path)
                                .map(|asset| asset.id)
                            {
                                editor.append_asset_clip(asset_id);
                                editor.status = Some("Added media to timeline.".to_string());
                                cx.notify();
                                return;
                            }
                            let track_id = match editor.find_append_track_for_asset(&asset) {
                                Ok(track_id) => track_id,
                                Err(error) => {
                                    editor.status = None;
                                    editor.error = Some(error);
                                    cx.notify();
                                    return;
                                }
                            };
                            editor.checkpoint();
                            let duration = asset.duration;
                            asset.id = editor.take_id();
                            asset.path = relative_path.clone();
                            let asset_id = asset.id;
                            editor.project.assets.push(asset);
                            editor
                                .append_asset_clip_without_checkpoint(asset_id, track_id, duration);
                            editor.selected_file = Some(relative_path);
                            editor.save_project();
                            editor.status = Some("Added media to timeline.".to_string());
                            editor.error = None;
                        }
                        Err(error) => editor.error = Some(error),
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }
}

fn rename_dialog_button(label: &'static str, primary: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(label)
        .h_10()
        .px_4()
        .flex()
        .items_center()
        .justify_center()
        .rounded_lg()
        .border_1()
        .border_color(rgb(if primary { ACCENT } else { BORDER }))
        .bg(rgb(if primary { ACCENT } else { SURFACE }))
        .text_color(rgb(if primary { 0x17120a } else { TEXT }))
        .cursor(CursorStyle::PointingHand)
        .hover(|style| style.opacity(0.85))
        .child(label)
}

fn renamed_relative_path(old_path: &std::path::Path, new_name: &str) -> Option<PathBuf> {
    let mut components = std::path::Path::new(new_name).components();
    let component = components.next()?;
    if components.next().is_some() || !matches!(component, std::path::Component::Normal(_)) {
        return None;
    }
    Some(
        old_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new(""))
            .join(new_name),
    )
}

fn remap_relative_path(
    path: &std::path::Path,
    old_path: &std::path::Path,
    new_path: &std::path::Path,
) -> Option<PathBuf> {
    if path == old_path {
        return Some(new_path.to_path_buf());
    }
    path.strip_prefix(old_path)
        .ok()
        .filter(|suffix| !suffix.as_os_str().is_empty())
        .map(|suffix| new_path.join(suffix))
}

#[cfg(test)]
mod rename_tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn renamed_path_stays_in_the_same_directory() {
        assert_eq!(
            renamed_relative_path(Path::new("media/old.mp4"), "new.mp4"),
            Some(PathBuf::from("media/new.mp4"))
        );
        assert_eq!(
            renamed_relative_path(Path::new("old.mp4"), "../new.mp4"),
            None
        );
    }

    #[test]
    fn directory_rename_remaps_descendants() {
        assert_eq!(
            remap_relative_path(
                Path::new("old/nested/clip.mp4"),
                Path::new("old"),
                Path::new("new")
            ),
            Some(PathBuf::from("new/nested/clip.mp4"))
        );
    }
}

#[cfg(target_os = "macos")]
fn move_path_to_trash(path: &std::path::Path) -> Result<(), String> {
    use objc2_foundation::{NSFileManager, NSString, NSURL};

    let path = path
        .to_str()
        .ok_or_else(|| "the path is not valid UTF-8".to_string())?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(path));
    NSFileManager::defaultManager()
        .trashItemAtURL_resultingItemURL_error(&url, None)
        .map_err(|error| error.to_string())
}

#[cfg(not(target_os = "macos"))]
fn move_path_to_trash(_path: &std::path::Path) -> Result<(), String> {
    Err("moving files to the system Trash is not supported on this platform".to_string())
}

fn explorer_file_badge(entry: &FileTreeEntry) -> gpui::Div {
    let extension = entry
        .relative_path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            extension
                .chars()
                .take(4)
                .collect::<String>()
                .to_ascii_uppercase()
        })
        .filter(|extension| !extension.is_empty())
        .unwrap_or_else(|| "FILE".to_string());
    let (extension, text, border) = if entry.is_timeline {
        ("TL".to_string(), 0xf0b75e, 0x8a652d)
    } else if entry.is_video {
        (extension, 0x8fb9dd, 0x355b78)
    } else if entry.is_audio {
        (extension, 0x7fd0ae, 0x32725a)
    } else if entry.is_image {
        (extension, 0xc3a9e8, 0x665184)
    } else {
        (extension, 0x8b8b94, 0x46464e)
    };

    div()
        .w_full()
        .h(px(18.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .border_1()
        .border_color(rgb(border))
        .font_family("monospace")
        .text_xs()
        .text_color(rgb(text))
        .child(extension)
}

fn explorer_metadata(entry: &FileTreeEntry, asset: Option<&MediaAsset>) -> Option<String> {
    if let Some(asset) = asset
        && asset.kind != MediaKind::Image
        && asset.duration.is_finite()
        && asset.duration > 0.0
    {
        return Some(format_time(asset.duration, true));
    }
    entry.size_bytes.map(format_file_size)
}

fn format_file_size(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.1} GB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.1} MB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.0} KB", bytes / KIB)
    } else {
        format!("{bytes:.0} B")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn asset(kind: MediaKind, has_audio: bool) -> MediaAsset {
        MediaAsset {
            id: 10,
            kind,
            path: PathBuf::from("media.mp4"),
            name: "Media".to_string(),
            duration: 10.0,
            width: 1920,
            height: 1080,
            framerate: 30.0,
            frame_rate_numerator: 30,
            frame_rate_denominator: 1,
            codec: "test".to_string(),
            has_audio,
        }
    }

    #[test]
    fn explorer_drop_rejects_incompatible_tracks() {
        let project = Project::with_test_tracks();
        let audio = asset(MediaKind::Audio, true);
        let audio_rejection = validate_clip_placement(
            &project,
            1,
            audio.kind,
            TimelineTime::from_frames(30),
            TimelineTime::ZERO,
            &HashSet::new(),
        )
        .unwrap_err();
        assert_eq!(
            audio_rejection.message(),
            "Media is incompatible with the destination track"
        );

        let silent_video = asset(MediaKind::Video, false);
        let video_rejection = validate_clip_placement(
            &project,
            2,
            silent_video.kind,
            TimelineTime::from_frames(30),
            TimelineTime::ZERO,
            &HashSet::new(),
        )
        .unwrap_err();
        assert_eq!(
            video_rejection.message(),
            "Media is incompatible with the destination track"
        );
    }

    #[test]
    fn explorer_drop_detects_collisions_but_allows_adjacent_clips() {
        let mut project = Project::with_test_tracks();
        project.clips.push(TimelineClip {
            id: 20,
            track_id: 2,
            asset_id: 10,
            timeline_start: TimelineTime::from_frames(30),
            source_in: TimelineTime::ZERO,
            source_out: TimelineTime::from_frames(30),
            video_properties: VideoClipProperties::default(),
            audio_properties: AudioClipProperties::default(),
        });
        let audio = asset(MediaKind::Audio, true);

        assert_eq!(
            validate_clip_placement(
                &project,
                2,
                audio.kind,
                TimelineTime::from_frames(30),
                TimelineTime::from_frames(15),
                &HashSet::new(),
            ),
            Err(ClipPlacementRejection::ExistingClipOverlap)
        );
        assert_eq!(
            validate_clip_placement(
                &project,
                2,
                audio.kind,
                TimelineTime::from_frames(30),
                TimelineTime::ZERO,
                &HashSet::new(),
            ),
            Ok(())
        );
    }
}
