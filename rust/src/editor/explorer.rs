use super::*;
use std::path::Path;

#[path = "explorer_file_menu.rs"]
mod explorer_file_menu;
pub(crate) use explorer_file_menu::FileContextMenu;
#[path = "explorer_file_entry.rs"]
mod explorer_file_entry;
pub(super) use explorer_file_entry::{
    FileTreeEntry, FileTreeEntryKind, is_audio_path, is_image_path, is_video_path, search_tree,
    visible_tree,
};

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
    pub(super) track_id: Ulid,
    pub(super) raw_start: TimelineTime,
    pub(super) start: TimelineTime,
    pub(super) duration: TimelineTime,
    pub(super) analyzing: bool,
    pub(super) invalid_reason: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct PendingExplorerDrop {
    relative_path: PathBuf,
    track_id: Ulid,
    raw_start: TimelineTime,
}

struct ExplorerDragView {
    name: String,
    kind: MediaKind,
}

impl ExplorerState {
    pub(super) fn refresh_file_tree(&mut self, project_root: &Path) -> Result<(), String> {
        self.last_tree_scan = Instant::now();
        self.file_tree = visible_tree(project_root, &self.expanded_directories)?;
        Ok(())
    }

    fn toggle_directory(
        &mut self,
        project_root: &Path,
        relative_path: PathBuf,
    ) -> Result<(), String> {
        if !self.expanded_directories.remove(&relative_path) {
            self.expanded_directories.insert(relative_path);
        }
        self.refresh_file_tree(project_root)
    }
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
    pub(super) fn explorer_panel(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let project_name = self
            .global_settings
            .project_root
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.global_settings.project_root.display().to_string());
        let filter_query = self.explorer.filter.read(cx).query().to_string();
        let filter = filter_query.trim().to_lowercase();
        let show_root_contents = self.explorer.root_expanded || !filter.is_empty();
        let visible_entries = if filter.is_empty() {
            &self.explorer.file_tree
        } else {
            &self.explorer.search_results
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
                editor.explorer.root_expanded = !editor.explorer.root_expanded;
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
            .map(|(index, entry)| self.explorer_file_entry(index, entry, cx))
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
                            .track_scroll(&self.explorer.scroll)
                            .flex()
                            .flex_col()
                            .py_2()
                            .child(root_row)
                            .when(show_root_contents && entries.is_empty(), |this| {
                                this.child(div().p_4().text_sm().text_color(rgb(MUTED)).child(
                                    if filter.is_empty() {
                                        "This project folder is empty.".to_string()
                                    } else if self.explorer.search_pending {
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
        self.explorer.filter.clone().into_any_element()
    }

    pub(super) fn schedule_explorer_search(&mut self, cx: &mut Context<Self>) {
        let query = self.explorer.filter.read(cx).query().trim().to_string();
        if query.is_empty() {
            self.explorer.search_query = None;
            self.explorer.search_results.clear();
            self.explorer.search_pending = false;
            return;
        }
        if self.explorer.search_query.as_deref() == Some(query.as_str()) {
            return;
        }

        self.explorer.search_query = Some(query.clone());
        self.explorer.search_results.clear();
        self.explorer.search_pending = true;

        let project_root = self.global_settings.project_root.clone();
        cx.spawn(async move |editor, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;

            let still_requested = editor
                .update(cx, |editor, _| {
                    editor.global_settings.project_root == project_root
                        && editor.explorer.search_query.as_deref() == Some(query.as_str())
                })
                .unwrap_or(false);
            if !still_requested {
                return;
            }

            let scan_root = project_root.clone();
            let scan_query = query.clone();
            let result = cx
                .background_executor()
                .spawn(async move { search_tree(&scan_root, &scan_query) })
                .await;

            editor
                .update(cx, |editor, cx| {
                    if editor.global_settings.project_root != project_root
                        || editor.explorer.search_query.as_deref() != Some(query.as_str())
                    {
                        return;
                    }
                    editor.explorer.search_pending = false;
                    match result {
                        Ok(entries) => {
                            editor.explorer.search_results = entries;
                        }
                        Err(error) => {
                            editor.explorer.search_results.clear();
                            eprintln!("{error}");
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
            .explorer
            .rename_dialog
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
                    editor.explorer.rename_dialog = None;
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
                                    editor.explorer.rename_dialog = None;
                                    cx.notify();
                                },
                            )))
                            .child(rename_dialog_button("Rename", true).on_click(cx.listener(
                                |editor, _, _, cx| {
                                    if let Err(error) = editor.finish_rename(cx) {
                                        eprintln!("{error}");
                                    }
                                    cx.notify();
                                },
                            ))),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn new_timeline_dialog(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let state = self
            .explorer
            .new_timeline_dialog
            .as_ref()
            .expect("new timeline dialog rendered without state");
        let input = state.input.clone();
        let location = if state.relative_directory.as_os_str().is_empty() {
            self.global_settings
                .project_root
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
                    editor.explorer.new_timeline_dialog = None;
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
                                    editor.explorer.new_timeline_dialog = None;
                                    cx.notify();
                                },
                            )))
                            .child(rename_dialog_button("Create", true).on_click(cx.listener(
                                |editor, _, _, cx| {
                                    if let Err(error) = editor.finish_create_timeline(cx) {
                                        eprintln!("{error}");
                                    }
                                    cx.notify();
                                },
                            ))),
                    ),
            )
            .into_any_element()
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
            .and_then(|index| self.timeline.as_ref()?.data.tracks.get(index))
            .map(|track| track.id)
        else {
            if self.explorer.drop_preview.take().is_some() {
                if let Some(timeline) = self.timeline.as_mut() {
                    timeline.interaction.snap_guide = None;
                }
                cx.notify();
            }
            cx.set_active_drag_cursor_style(CursorStyle::OperationNotAllowed, window);
            return;
        };

        let drag = event.drag(cx).clone();
        let local_x = f32::from(pointer.x) - f32::from(event.bounds.left());
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let raw_start = timeline.data.nearest_time(
            ((local_x - TIMELINE_PADDING) / timeline.data.view.pixels_per_second).max(0.0) as f64,
        );
        self.refresh_explorer_drop_preview(&drag, track_id, raw_start);
        let invalid = self
            .explorer
            .drop_preview
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
        if !self.explorer.drag_probe_jobs.contains(&drag.relative_path)
            && self.explorer_asset_for_path(&drag.relative_path).is_none()
        {
            self.request_explorer_drag_probe(drag.relative_path.clone(), cx);
        }
        cx.notify();
    }

    fn refresh_explorer_drop_preview(
        &mut self,
        drag: &ExplorerMediaDrag,
        track_id: Ulid,
        raw_start: TimelineTime,
    ) {
        let asset = self.explorer_asset_for_path(&drag.relative_path).cloned();
        let analyzing = asset.is_none();
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let duration = asset
            .as_ref()
            .map(|asset| timeline.data.ceil_time(asset.duration))
            .unwrap_or_else(|| timeline.data.ceil_time(DEFAULT_IMAGE_CLIP_DURATION));
        let (start, snap_guide) =
            timeline.snap_clip_start_ignoring(raw_start, duration, &HashSet::new());
        let kind = asset.as_ref().map_or(drag.kind, |asset| asset.kind);
        let invalid_reason = validate_clip_placement(
            &timeline.data,
            track_id,
            kind,
            duration,
            start,
            &HashSet::new(),
        )
        .err()
        .map(|rejection| rejection.message().to_string());
        if let Some(timeline) = self.timeline.as_mut() {
            timeline.interaction.snap_guide = snap_guide;
        }
        self.explorer.drop_preview = Some(ExplorerDropPreview {
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
        let Some(preview) = self.explorer.drop_preview.take().filter(|preview| {
            preview.relative_path == drag.relative_path
                && self
                    .timeline
                    .as_ref()
                    .is_some_and(|timeline| timeline.data.track(preview.track_id).is_some())
        }) else {
            if let Some(timeline) = self.timeline.as_mut() {
                timeline.interaction.snap_guide = None;
            }
            return;
        };
        if let Some(timeline) = self.timeline.as_mut() {
            timeline.interaction.snap_guide = None;
        }

        if let Some(reason) = preview.invalid_reason {
            eprintln!("Cannot add {}: {reason}.", drag.name);
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
                cx,
            );
        } else {
            self.explorer.pending_drop = Some(PendingExplorerDrop {
                relative_path: drag.relative_path.clone(),
                track_id: preview.track_id,
                raw_start: preview.raw_start,
            });
            self.status = Some(format!("Inspecting {} before placing it…", drag.name));
            self.request_explorer_drag_probe(drag.relative_path.clone(), cx);
        }
        cx.notify();
    }

    fn explorer_asset_for_path(&self, relative_path: &std::path::Path) -> Option<&MediaAsset> {
        self.timeline
            .as_ref()
            .and_then(|timeline| timeline.data.asset_for_path(relative_path))
            .or_else(|| self.explorer.drag_assets.get(relative_path))
    }

    fn request_explorer_drag_probe(&mut self, relative_path: PathBuf, cx: &mut Context<Self>) {
        if self.explorer_asset_for_path(&relative_path).is_some()
            || !self.explorer.drag_probe_jobs.insert(relative_path.clone())
        {
            return;
        }

        let project_root = self.global_settings.project_root.clone();
        let source_path = project_root.join(&relative_path);
        cx.spawn(async move |editor, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { probe_asset(&source_path, Ulid::from(0)) })
                .await;

            editor
                .update(cx, |editor, cx| {
                    if editor.global_settings.project_root != project_root {
                        return;
                    }
                    editor.explorer.drag_probe_jobs.remove(&relative_path);
                    match result {
                        Ok(mut asset) => {
                            asset.path = relative_path.clone();
                            editor
                                .explorer
                                .drag_assets
                                .insert(relative_path.clone(), asset.clone());

                            if let Some(preview) = editor
                                .explorer
                                .drop_preview
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
                                .explorer
                                .pending_drop
                                .as_ref()
                                .is_some_and(|pending| pending.relative_path == relative_path);
                            if pending_matches
                                && let Some(pending) = editor.explorer.pending_drop.take()
                            {
                                editor.place_explorer_asset(
                                    relative_path.clone(),
                                    pending.track_id,
                                    pending.raw_start,
                                    asset,
                                    cx,
                                );
                            }
                        }
                        Err(error) => {
                            if let Some(preview) = editor
                                .explorer
                                .drop_preview
                                .as_mut()
                                .filter(|preview| preview.relative_path == relative_path)
                            {
                                preview.analyzing = false;
                                preview.invalid_reason = Some(error.clone());
                            }
                            if editor
                                .explorer
                                .pending_drop
                                .as_ref()
                                .is_some_and(|pending| pending.relative_path == relative_path)
                            {
                                editor.explorer.pending_drop = None;
                                editor.status = None;
                                eprintln!("{error}");
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
        track_id: Ulid,
        raw_start: TimelineTime,
        mut asset: MediaAsset,
        cx: &mut Context<Self>,
    ) {
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let duration = timeline.data.ceil_time(asset.duration);
        let (start, _) = timeline.snap_clip_start_ignoring(raw_start, duration, &HashSet::new());
        if let Err(rejection) = validate_clip_placement(
            &timeline.data,
            track_id,
            asset.kind,
            duration,
            start,
            &HashSet::new(),
        ) {
            let reason = rejection.message();
            self.status = None;
            eprintln!("Cannot add {}: {reason}.", asset.name);
            return;
        }

        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.record_editing_history();
        self.preview.timeline_needs_rebuild = true;
        let asset_id = if let Some(asset_id) = timeline
            .data
            .assets
            .iter()
            .find(|existing| existing.path == relative_path)
            .map(|existing| existing.id)
        {
            asset_id
        } else {
            asset.id = Ulid::generate();
            asset.path = relative_path.clone();
            let asset_id = asset.id;
            timeline.data.assets.push(asset);
            asset_id
        };
        let clip_id = Ulid::generate();
        timeline.data.clips.push(TimelineClip {
            id: clip_id,
            track_id,
            asset_id,
            timeline_start: start,
            source_in: TimelineTime::ZERO,
            source_out: duration,
            video_properties: VideoClipProperties::default(),
            audio_properties: AudioClipProperties::default(),
        });
        let playhead = timeline.playhead;
        self.preview.target = PreviewTarget::Timeline;
        self.load_timeline_position_with_options(playhead, false, true);
        self.explorer.selected_file = Some(relative_path);
        self.select_only_clip(Some(clip_id));
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        timeline.save(&self.global_settings.project_root);
        self.rebuild_timeline_preview_if_needed();
        self.schedule_active_timeline_waveforms(cx);
        self.status = Some("Added media at the selected timeline position.".to_string());
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
    let (extension, text, border) = match entry.kind {
        FileTreeEntryKind::Timeline => ("TL".to_string(), 0xf0b75e, 0x8a652d),
        FileTreeEntryKind::Video => (extension, 0x8fb9dd, 0x355b78),
        FileTreeEntryKind::Audio => (extension, 0x7fd0ae, 0x32725a),
        FileTreeEntryKind::Image => (extension, 0xc3a9e8, 0x665184),
        FileTreeEntryKind::Directory { .. } | FileTreeEntryKind::Other => {
            (extension, 0x8b8b94, 0x46464e)
        }
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

#[cfg(test)]
#[path = "explorer.test.rs"]
mod tests;
