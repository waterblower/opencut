use crate::clip_placement::validate_clip_placement;
use crate::edit_action::{EditAction, edit_timeline};
use crate::editor::Editor;
use crate::explorer_file_entry::{FileTreeEntry, FileTreeEntryKind, search_tree, visible_tree};
use crate::model::MediaAsset;
use crate::theme::{ACCENT, BORDER, MUTED, PANEL, SURFACE, TEXT};
use crate::timeline::TimelineFrameIndex;
use crate::timeline_clip::{AudioClipProperties, Clip, VideoClip, VideoClipProperties};
use crate::timeline_document::relative_asset_path;
use crate::track::TrackKind;
use crate::ui_components::TextInput;
use anyhow::{Context as _, Result, anyhow, bail};
use futures::StreamExt;
use futures::channel::mpsc;
use gpui::prelude::FluentBuilder;
use gpui::{
    AppContext as _, Context, CursorStyle, Entity, FocusHandle, InteractiveElement, IntoElement,
    MouseButton, ParentElement, ScrollHandle, StatefulInteractiveElement, Styled, Task, div, px,
    rgb,
};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use ulid::Ulid;

pub(super) struct RenameDialogState {
    pub(crate) relative_path: PathBuf,
    pub(crate) input: Entity<TextInput>,
}

pub(super) struct NewTimelineDialogState {
    pub(crate) relative_directory: PathBuf,
    pub(crate) input: Entity<TextInput>,
}

pub(crate) struct ExplorerState {
    pub(crate) file_tree: Vec<FileTreeEntry>,
    pub(crate) expanded_directories: HashSet<PathBuf>,
    pub(crate) root_expanded: bool,
    pub(crate) filter: Entity<TextInput>,
    pub(crate) search_query: Option<String>,
    pub(crate) search_results: Vec<FileTreeEntry>,
    pub(crate) search_pending: bool,
    pub(crate) scroll: ScrollHandle,
    pub(crate) selected_file: Option<PathBuf>,
    pub(crate) rename_dialog: Option<RenameDialogState>,
    pub(crate) new_timeline_dialog: Option<NewTimelineDialogState>,
    pub(crate) trash_timeline_dialog_open: bool, // 确认把当前打开的时间线移到废纸篓。
    _file_tree_watch: Task<()>,                  // drop 时停止监听项目目录。
}

#[derive(Deserialize, Serialize)]
struct FileExplorerSettings {
    #[serde(default)]
    expanded_directories: Vec<PathBuf>,
    #[serde(default = "root_expanded_by_default")]
    root_expanded: bool,
}

impl ExplorerState {
    /// Loads the saved folder expansion, scans the visible tree and starts watching
    /// `project_root` for changes. `selected_file` is relative to `project_root`.
    pub(super) fn new(
        project_root: &Path,
        selected_file: Option<PathBuf>,
        return_focus: FocusHandle,
        cx: &mut Context<Editor>,
    ) -> Self {
        let filter = cx
            .new(|cx| TextInput::new_search("explorer-filter", "Filter files…", return_focus, cx));
        cx.observe(&filter, |editor, _, cx| {
            editor.schedule_explorer_search(cx);
            cx.notify();
        })
        .detach();
        let expansion = load_explorer_expansion(project_root);
        let file_tree =
            visible_tree(project_root, &expansion.expanded_directories).unwrap_or_default();
        Self {
            file_tree,
            expanded_directories: expansion.expanded_directories,
            root_expanded: expansion.root_expanded,
            filter,
            search_query: None,
            search_results: Vec::new(),
            search_pending: false,
            scroll: ScrollHandle::new(),
            selected_file,
            rename_dialog: None,
            new_timeline_dialog: None,
            trash_timeline_dialog_open: false,
            _file_tree_watch: watch_file_tree(project_root.to_path_buf(), cx),
        }
    }

    pub(super) fn refresh_file_tree(&mut self, project_root: &Path) -> Result<()> {
        self.file_tree = visible_tree(project_root, &self.expanded_directories)?;
        Ok(())
    }

    pub(crate) fn toggle_directory(
        &mut self,
        project_root: &Path,
        relative_path: PathBuf,
    ) -> Result<()> {
        if !self.expanded_directories.remove(&relative_path) {
            self.expanded_directories.insert(relative_path);
        }
        self.refresh_file_tree(project_root)
    }
}

impl Editor {
    pub(super) fn save_explorer_expansion(&self) -> Result<()> {
        save_explorer_expansion(
            &self.project_root,
            &self.explorer.expanded_directories,
            self.explorer.root_expanded,
        )
    }
}

impl Editor {
    pub(super) fn schedule_explorer_search(&mut self, cx: &mut Context<Self>) {
        let query = self.explorer.filter.read(cx).text().trim().to_string();
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

        let project_root = self.project_root.clone();
        cx.spawn(async move |editor, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;

            let still_requested = editor
                .update(cx, |editor, _| {
                    editor.project_root == project_root
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
                    if editor.project_root != project_root
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
                            .child(format!("In {location} · saved as .timeline (JSON)")),
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
                                        eprintln!("{error:?}");
                                    }
                                    cx.notify();
                                },
                            ))),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn trash_timeline_dialog_view(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let timeline_name = match self.timeline.as_ref() {
            Some(timeline) => timeline
                .path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            None => String::new(),
        };

        div()
            .id("trash-timeline-dialog-overlay")
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
                    editor.explorer.trash_timeline_dialog_open = false;
                    cx.notify();
                }),
            )
            .child(
                div()
                    .id("trash-timeline-dialog")
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
                            .child(format!("Move {timeline_name} to Trash?")),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .gap_2()
                            .child(rename_dialog_button("Cancel", false).on_click(cx.listener(
                                |editor, _, _, cx| {
                                    editor.explorer.trash_timeline_dialog_open = false;
                                    cx.notify();
                                },
                            )))
                            .child(rename_dialog_button("Move to Trash", true).on_click(
                                cx.listener(|editor, _, _, cx| {
                                    if let Err(error) = editor.trash_active_timeline(cx) {
                                        eprintln!("{error:?}");
                                    }
                                    cx.notify();
                                }),
                            )),
                    ),
            )
            .into_any_element()
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

pub(crate) fn renamed_relative_path(old_path: &std::path::Path, new_name: &str) -> Option<PathBuf> {
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

pub(crate) fn remap_relative_path(
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
pub(crate) fn move_path_to_trash(path: &std::path::Path) -> Result<()> {
    use objc2_foundation::{NSFileManager, NSString, NSURL};

    let path = path
        .to_str()
        .ok_or_else(|| anyhow!("the path is not valid UTF-8"))?;
    let url = NSURL::fileURLWithPath(&NSString::from_str(path));
    NSFileManager::defaultManager()
        .trashItemAtURL_resultingItemURL_error(&url, None)
        .map_err(|error| anyhow!("{error}"))
}

#[cfg(not(target_os = "macos"))]
fn move_path_to_trash(_path: &std::path::Path) -> Result<()> {
    Err(anyhow!(
        "moving files to the system Trash is not supported on this platform"
    ))
}

pub(crate) fn explorer_file_badge(entry: &FileTreeEntry) -> gpui::Div {
    let extension = entry
        .absolute_path
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
) -> Result<()> {
    let path = file_explorer_settings_path(project_root);
    let Some(directory) = path.parent() else {
        bail!("file explorer settings path had no parent directory");
    };
    fs::create_dir_all(directory)
        .map_err(|error| anyhow!("could not create {}: {error}", directory.display()))?;
    let mut expanded_directories = expanded_directories.iter().cloned().collect::<Vec<_>>();
    expanded_directories.sort();
    let json = serde_json::to_string_pretty(&FileExplorerSettings {
        expanded_directories,
        root_expanded,
    })
    .map_err(|error| anyhow!("could not serialize file explorer settings: {error}"))?;
    fs::write(&path, format!("{json}\n"))
        .map_err(|error| anyhow!("could not write {}: {error}", path.display()))
}

fn file_explorer_settings_path(project_root: &Path) -> PathBuf {
    project_root.join(".opencut/file-explorer.json")
}

fn root_expanded_by_default() -> bool {
    true
}

struct ExplorerExpansion {
    expanded_directories: HashSet<PathBuf>,
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

fn load_explorer_expansion(project_root: &Path) -> ExplorerExpansion {
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

/// Rescans the visible file tree whenever something under `project_root` changes.
/// The watch lasts as long as the returned task.
fn watch_file_tree(project_root: PathBuf, cx: &mut Context<Editor>) -> Task<()> {
    // 一次改动常带来一串事件；等这段时间让它们到齐，再只重扫一次。
    const EVENT_BATCH_DELAY: Duration = Duration::from_millis(100);
    let (sender, mut receiver) = mpsc::unbounded::<notify::Result<notify::Event>>();
    cx.spawn(async move |editor, cx| {
        let watcher = (|| -> notify::Result<RecommendedWatcher> {
            let mut watcher = notify::recommended_watcher(move |event| {
                // 发送失败只说明 task 已结束、接收端已释放，此时不再需要事件。
                let _ = sender.unbounded_send(event);
            })?;
            watcher.watch(&project_root, RecursiveMode::Recursive)?;
            Ok(watcher)
        })();
        let _watcher = match watcher {
            Ok(watcher) => watcher, // 必须活到 task 结束，否则监听立即停止。
            Err(error) => {
                log::error!("Could not watch {}: {error:?}", project_root.display());
                return;
            }
        };
        while let Some(first_event) = receiver.next().await {
            cx.background_executor().timer(EVENT_BATCH_DELAY).await;
            let mut events = vec![first_event];
            while let Ok(event) = receiver.try_recv() {
                events.push(event);
            }
            for event in &events {
                match event {
                    Ok(_) => {
                        // do nothing
                    }
                    Err(error) => {
                        log::error!("File watch error in {}: {error:?}", project_root.display());
                    }
                }
            }
            // 读取文件只产生 Access 事件，不改变文件树；出错时无法判断，按已改变处理。
            let tree_may_have_changed = events.iter().any(|event| match event {
                Ok(event) => !matches!(event.kind, EventKind::Access(_)),
                Err(_) => true,
            });
            if !tree_may_have_changed {
                continue;
            }
            let result = editor.update(cx, |editor, cx| -> Result<()> {
                editor.explorer.refresh_file_tree(&editor.project_root)?;
                cx.notify();
                Ok(())
            });
            match result {
                Ok(Ok(())) => {
                    // do nothing
                }
                Ok(Err(error)) => log::error!("{error:?}"),
                Err(error) => {
                    log::debug!("editor released: {error:?}");
                    break;
                }
            }
        }
    })
}

#[cfg(test)]
#[path = "tests/explorer.test.rs"]
mod tests;
