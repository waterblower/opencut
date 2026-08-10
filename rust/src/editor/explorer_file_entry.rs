use super::*;
use std::{collections::HashSet, fs, path::Path};

#[derive(Clone, PartialEq)]
pub(in crate::editor) struct FileTreeEntry {
    pub relative_path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub kind: FileTreeEntryKind,
    pub size_bytes: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::editor) enum FileTreeEntryKind {
    Directory { expanded: bool },
    Video,
    Image,
    Audio,
    Timeline,
    Other,
}

pub(in crate::editor) fn visible_tree(
    project_root: &Path,
    expanded_directories: &HashSet<PathBuf>,
) -> Result<Vec<FileTreeEntry>, String> {
    let mut entries = Vec::new();
    read_directory(
        project_root,
        Path::new(""),
        0,
        expanded_directories,
        &mut entries,
    )?;
    Ok(entries)
}

/// Searches the complete project tree, independently of which folders are expanded.
/// Matching ancestor directories are included so results retain their hierarchy.
pub(in crate::editor) fn search_tree(
    project_root: &Path,
    query: &str,
) -> Result<Vec<FileTreeEntry>, String> {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    search_directory(project_root, Path::new(""), 0, &query, true)
}

fn search_directory(
    project_root: &Path,
    relative_directory: &Path,
    depth: usize,
    query: &str,
    is_root: bool,
) -> Result<Vec<FileTreeEntry>, String> {
    let directory = project_root.join(relative_directory);
    let children = match directory_children(&directory) {
        Ok(children) => children,
        Err(error) if !is_root => {
            eprintln!("{error}");
            return Ok(Vec::new());
        }
        Err(error) => return Err(error),
    };
    let mut matches = Vec::new();

    for (name, is_directory, size_bytes) in children {
        let relative_path = relative_directory.join(&name);
        if is_directory {
            let descendants =
                search_directory(project_root, &relative_path, depth + 1, query, false)?;
            let directory_matches = relative_path
                .to_string_lossy()
                .to_lowercase()
                .contains(query);
            if directory_matches || !descendants.is_empty() {
                matches.push(file_tree_entry(
                    relative_path,
                    name,
                    depth,
                    true,
                    None,
                    !descendants.is_empty(),
                ));
                matches.extend(descendants);
            }
        } else if relative_path
            .to_string_lossy()
            .to_lowercase()
            .contains(query)
        {
            matches.push(file_tree_entry(
                relative_path,
                name,
                depth,
                false,
                size_bytes,
                false,
            ));
        }
    }

    Ok(matches)
}

fn read_directory(
    project_root: &Path,
    relative_directory: &Path,
    depth: usize,
    expanded_directories: &HashSet<PathBuf>,
    entries: &mut Vec<FileTreeEntry>,
) -> Result<(), String> {
    let directory = project_root.join(relative_directory);
    let children = directory_children(&directory)?;

    for (name, is_directory, size_bytes) in children {
        let relative_path = relative_directory.join(&name);
        let expanded = is_directory && expanded_directories.contains(&relative_path);
        entries.push(file_tree_entry(
            relative_path.clone(),
            name,
            depth,
            is_directory,
            size_bytes,
            expanded,
        ));
        if expanded {
            read_directory(
                project_root,
                &relative_path,
                depth + 1,
                expanded_directories,
                entries,
            )?;
        }
    }
    Ok(())
}

fn directory_children(directory: &Path) -> Result<Vec<(String, bool, Option<u64>)>, String> {
    let mut children = fs::read_dir(directory)
        .map_err(|error| format!("could not read {}: {error}", directory.display()))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            let size_bytes = (!file_type.is_dir())
                .then(|| entry.metadata().ok().map(|metadata| metadata.len()))
                .flatten();
            let name = entry.file_name().to_string_lossy().into_owned();
            if matches!(name.as_str(), ".DS_Store" | ".git" | ".opencut") {
                return None;
            }
            Some((name, file_type.is_dir(), size_bytes))
        })
        .collect::<Vec<_>>();
    children.sort_by(|(left_name, left_dir, _), (right_name, right_dir, _)| {
        right_dir
            .cmp(left_dir)
            .then_with(|| left_name.to_lowercase().cmp(&right_name.to_lowercase()))
    });
    Ok(children)
}

fn file_tree_entry(
    relative_path: PathBuf,
    name: String,
    depth: usize,
    is_directory: bool,
    size_bytes: Option<u64>,
    expanded: bool,
) -> FileTreeEntry {
    let kind = if is_directory {
        FileTreeEntryKind::Directory { expanded }
    } else if super::super::timeline_document::is_timeline_path(&relative_path) {
        FileTreeEntryKind::Timeline
    } else if is_video_path(&relative_path) {
        FileTreeEntryKind::Video
    } else if is_image_path(&relative_path) {
        FileTreeEntryKind::Image
    } else if is_audio_path(&relative_path) {
        FileTreeEntryKind::Audio
    } else {
        FileTreeEntryKind::Other
    };
    FileTreeEntry {
        relative_path,
        name,
        depth,
        kind,
        size_bytes,
    }
}

pub(in crate::editor) fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png"
            )
        })
}

pub(in crate::editor) fn is_video_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "mp4" | "mov" | "m4v" | "mkv" | "webm" | "avi"
            )
        })
}

pub(in crate::editor) fn is_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "aac" | "flac" | "m4a" | "mp3" | "ogg" | "wav"
            )
        })
}

impl Editor {
    pub(super) fn explorer_file_entry(
        &self,
        index: usize,
        entry: &FileTreeEntry,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let path = entry.relative_path.clone();
        let selection_path = path.clone();
        let action_path = path.clone();
        let context_path = path.clone();
        let selected = self.explorer.selected_file.as_ref() == Some(&path);
        let media_kind = match entry.kind {
            FileTreeEntryKind::Video => Some(MediaKind::Video),
            FileTreeEntryKind::Image => Some(MediaKind::Image),
            FileTreeEntryKind::Audio => Some(MediaKind::Audio),
            FileTreeEntryKind::Directory { .. }
            | FileTreeEntryKind::Timeline
            | FileTreeEntryKind::Other => None,
        };
        let media_drag = media_kind.map(|kind| ExplorerMediaDrag {
            relative_path: path.clone(),
            name: entry.name.clone(),
            kind,
        });
        let metadata = file_entry_metadata(entry, self.timeline.as_ref());

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
            .when(
                matches!(entry.kind, FileTreeEntryKind::Directory { .. }),
                |this| {
                    let selection_path = selection_path.clone();
                    this.on_click(cx.listener(move |editor, _, _, cx| {
                        editor.toggle_directory(selection_path.clone());
                        cx.notify();
                    }))
                },
            )
            .when(entry.kind == FileTreeEntryKind::Timeline, |this| {
                let selection_path = selection_path.clone();
                this.on_click(cx.listener(move |editor, _, _, cx| {
                    editor.open_timeline(selection_path.clone(), cx);
                    cx.notify();
                }))
            })
            .when(
                !matches!(
                    entry.kind,
                    FileTreeEntryKind::Directory { .. } | FileTreeEntryKind::Timeline
                ),
                |this| {
                    let selection_path = selection_path.clone();
                    this.on_click(cx.listener(move |editor, _, _, cx| {
                        editor.select_file(selection_path.clone(), cx);
                        cx.notify();
                    }))
                },
            )
            .when(
                matches!(entry.kind, FileTreeEntryKind::Directory { .. }),
                |this| {
                    let context_path = context_path.clone();
                    this.on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                            editor.show_file_context_menu(context_path.clone(), true, event, cx);
                        }),
                    )
                },
            )
            .when(
                !matches!(entry.kind, FileTreeEntryKind::Directory { .. }),
                |this| {
                    let context_path = context_path.clone();
                    this.on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |editor, event: &MouseDownEvent, _, cx| {
                            editor.show_file_context_menu(context_path.clone(), false, event, cx);
                        }),
                    )
                },
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
                    .w(px(
                        if matches!(entry.kind, FileTreeEntryKind::Directory { .. }) {
                            14.0
                        } else {
                            38.0
                        },
                    ))
                    .h(px(20.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .when_some(
                        match entry.kind {
                            FileTreeEntryKind::Directory { expanded } => Some(expanded),
                            _ => None,
                        },
                        |this, expanded| {
                            this.text_color(rgb(MUTED))
                                .child(if expanded { "▾" } else { "▸" })
                        },
                    )
                    .when(
                        !matches!(entry.kind, FileTreeEntryKind::Directory { .. }),
                        |this| this.child(explorer_file_badge(entry)),
                    ),
            )
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .text_sm()
                    .font_family("monospace")
                    .text_ellipsis()
                    .text_color(rgb(if entry.kind != FileTreeEntryKind::Other {
                        TEXT
                    } else {
                        MUTED
                    }))
                    .child(entry.name.clone()),
            )
            .when(media_kind.is_some() && selected, |this| {
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
    }
}

fn file_entry_metadata(
    entry: &FileTreeEntry,
    active_timeline: Option<&TimelineState>,
) -> Option<String> {
    match entry.kind {
        FileTreeEntryKind::Directory { .. } => None,
        FileTreeEntryKind::Timeline
            if active_timeline.is_some_and(|timeline| timeline.path == entry.relative_path) =>
        {
            Some("ACTIVE".to_string())
        }
        FileTreeEntryKind::Video
        | FileTreeEntryKind::Image
        | FileTreeEntryKind::Audio
        | FileTreeEntryKind::Timeline
        | FileTreeEntryKind::Other => entry.size_bytes.map(format_file_size),
    }
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
#[path = "explorer_file_entry.test.rs"]
mod tests;
