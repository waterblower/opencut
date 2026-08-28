use crate::{
    editor::{
        ACCENT, BORDER, MUTED, OpenInDefaultApp, PANEL, RevealInFinder,
        SURFACE, SURFACE_HOVER, TEXT,
        clip_placement::validate_clip_placement,
        context_menu::{ContextMenu, FileContextMenu},
        editing::{EditAction, edit_and_rebuild_timeline},
        editor::Editor,
        explorer_filter::ExplorerFilter,
        model::{MediaAsset, MediaKind},
        preview::PreviewTarget,
        preview_audio::AudioBackend,
        project_settings::save_project_local_settings,
        timeline::TimelineTime,
        timeline_clip::{AudioClipProperties, Clip, VideoClip, VideoClipProperties},
        timeline_document,
        track::TrackKind,
    },
    video::VideoBackend,
};
use gpui::{
    AppContext as _, Context, CursorStyle, Entity, InteractiveElement, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, StatefulInteractiveElement, Styled, Window, div, px, rgb,
};
use gpui::{ScrollHandle, prelude::FluentBuilder};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use ulid::Ulid;

#[path = "explorer_file_entry.rs"]
mod explorer_file_entry;
#[path = "explorer_file_menu.rs"]
mod explorer_file_menu;
pub(super) use explorer_file_entry::{
    FileTreeEntry, FileTreeEntryKind, is_audio_path, is_image_path, is_srt_path, is_video_path,
    search_tree, visible_tree,
};

pub(super) struct RenameDialogState {
    relative_path: PathBuf,
    input: Entity<ExplorerFilter>,
}

pub(super) struct NewTimelineDialogState {
    relative_directory: PathBuf,
    input: Entity<ExplorerFilter>,
}

pub(super) struct ExplorerExpansion {
    pub(super) expanded_directories: HashSet<PathBuf>,
    pub(super) root_expanded: bool,
}

pub struct ExplorerState {
    pub file_tree: Vec<FileTreeEntry>,
    pub expanded_directories: HashSet<PathBuf>,
    pub root_expanded: bool,
    pub filter: Entity<ExplorerFilter>,
    pub search_query: Option<String>,
    pub search_results: Vec<FileTreeEntry>,
    pub search_pending: bool,
    pub scroll: ScrollHandle,
    pub selected_file: Option<PathBuf>,
    pub rename_dialog: Option<RenameDialogState>,
    pub new_timeline_dialog: Option<NewTimelineDialogState>,
    pub last_tree_scan: Instant,
}

pub(super) fn load_explorer_expansion(project_root: &Path) -> ExplorerExpansion {
    let Ok(contents) = fs::read_to_string(file_explorer_settings_path(project_root)) else {
        return ExplorerExpansion::default();
    };
    let Ok(settings) = serde_json::from_str::<FileExplorerSettings>(&contents) else {
        return ExplorerExpansion::default();
    };
    ExplorerExpansion {
        expanded_directories: settings
            .expanded_directories
            .into_iter()
            .filter(|path| {
                !path.as_os_str().is_empty()
                    && path.is_relative()
                    && path
                        .components()
                        .all(|component| matches!(component, std::path::Component::Normal(_)))
            })
            .collect(),
        root_expanded: settings.root_expanded,
    }
}

#[derive(Deserialize, Serialize)]
struct FileExplorerSettings {
    #[serde(default)]
    expanded_directories: Vec<PathBuf>,
    #[serde(default = "root_expanded_by_default")]
    root_expanded: bool,
}

impl Default for ExplorerExpansion {
    fn default() -> Self {
        Self {
            expanded_directories: HashSet::new(),
            root_expanded: true,
        }
    }
}

impl ExplorerState {
    pub(super) fn refresh_file_tree(&mut self, project_root: &Path) -> anyhow::Result<()> {
        self.last_tree_scan = Instant::now();
        self.file_tree = visible_tree(project_root, &self.expanded_directories)?;
        Ok(())
    }

    fn toggle_directory(
        &mut self,
        project_root: &Path,
        relative_path: PathBuf,
    ) -> anyhow::Result<()> {
        if !self.expanded_directories.remove(&relative_path) {
            self.expanded_directories.insert(relative_path);
        }
        self.refresh_file_tree(project_root)
    }
}

impl Editor {
    pub(super) fn save_explorer_expansion(&self) -> anyhow::Result<()> {
        save_explorer_expansion(
            &self.global_settings.project_root,
            &self.explorer.expanded_directories,
            self.explorer.root_expanded,
        )
    }
}

fn explorer_filter(filter: &Entity<ExplorerFilter>) -> gpui::AnyElement {
    filter.clone().into_any_element()
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
                if let Err(error) = editor.save_explorer_expansion() {
                    eprintln!("Could not save explorer expansion: {error}");
                }
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
            .w_full()
            .h_full()
            .bg(rgb(PANEL))
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .bg(rgb(PANEL))
                    .child(explorer_filter(&self.explorer.filter))
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

    pub(super) fn place_explorer_asset(
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
        let track_kind = timeline
            .data
            .tracks
            .iter()
            .find(|track| track.id == track_id)
            .map(|track| track.kind);
        let (start, _) = timeline.snap_clip_start_ignoring(raw_start, duration, &HashSet::new());
        if let Err(rejection) = validate_clip_placement(
            &timeline.data,
            track_id,
            asset.kind,
            duration,
            start,
            &HashSet::new(),
        ) {
            self.status = None;
            eprintln!("Cannot add {}: {rejection}.", asset.name);
            return;
        }

        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        timeline.record_editing_history();
        let (asset_id, assets) = if let Some(asset_id) = timeline
            .data
            .assets
            .iter()
            .find(|existing| existing.path == relative_path)
            .map(|existing| existing.id)
        {
            (asset_id, Vec::new())
        } else {
            asset.id = Ulid::generate();
            asset.path = relative_path.clone();
            let asset_id = asset.id;
            (asset_id, vec![asset])
        };
        let clip_id = Ulid::generate();
        let media_clip = VideoClip {
            id: clip_id,
            track_id,
            asset_id,
            timeline_start: start,
            source_in: TimelineTime::ZERO,
            source_out: duration,
            video_properties: VideoClipProperties::default(),
            audio_properties: AudioClipProperties::default(),
        };
        let media_clip = match track_kind {
            Some(TrackKind::Video) => Clip::Video(media_clip),
            Some(TrackKind::Audio) => Clip::Audio(media_clip),
            _ => return,
        };

        edit_and_rebuild_timeline(
            &mut self.preview,
            &self.global_settings.project_root,
            timeline,
            EditAction::AddClips {
                clips: vec![media_clip],
                assets,
            },
        )
        .unwrap();

        self.explorer.selected_file = Some(relative_path);
        self.select_only_clip(Some(clip_id));
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        timeline.save(&self.global_settings.project_root);

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
fn move_path_to_trash(path: &std::path::Path) -> anyhow::Result<()> {
    use objc2_foundation::{NSFileManager, NSString, NSURL};

    let path = path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("the path is not valid UTF-8"))?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(path));
    NSFileManager::defaultManager()
        .trashItemAtURL_resultingItemURL_error(&url, None)
        .map_err(|error| anyhow::anyhow!("{error}"))
}

#[cfg(not(target_os = "macos"))]
fn move_path_to_trash(_path: &std::path::Path) -> anyhow::Result<()> {
    Err(anyhow::anyhow!(
        "moving files to the system Trash is not supported on this platform"
    ))
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

fn save_explorer_expansion(
    project_root: &Path,
    expanded_directories: &HashSet<PathBuf>,
    root_expanded: bool,
) -> anyhow::Result<()> {
    let path = file_explorer_settings_path(project_root);
    let Some(directory) = path.parent() else {
        anyhow::bail!("file explorer settings path had no parent directory");
    };
    fs::create_dir_all(directory)
        .map_err(|error| anyhow::anyhow!("could not create {}: {error}", directory.display()))?;
    let mut expanded_directories = expanded_directories.iter().cloned().collect::<Vec<_>>();
    expanded_directories.sort();
    let json = serde_json::to_string_pretty(&FileExplorerSettings {
        expanded_directories,
        root_expanded,
    })
    .map_err(|error| anyhow::anyhow!("could not serialize file explorer settings: {error}"))?;
    fs::write(&path, format!("{json}\n"))
        .map_err(|error| anyhow::anyhow!("could not write {}: {error}", path.display()))
}

fn file_explorer_settings_path(project_root: &Path) -> PathBuf {
    project_root.join(".opencut/file-explorer.json")
}

fn root_expanded_by_default() -> bool {
    true
}

#[cfg(test)]
#[path = "explorer.test.rs"]
mod tests;
