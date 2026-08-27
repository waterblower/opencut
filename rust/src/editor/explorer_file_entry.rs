use crate::editor::explorer_drag::ExplorerMediaDrag;

use super::*;
use std::{collections::HashSet, fs, path::Path};
use url::Url;

#[derive(Clone, PartialEq)]
pub struct FileTreeEntry {
    pub relative_path: PathBuf,
    pub name: String,
    pub depth: usize,
    pub kind: FileTreeEntryKind,
    pub size_bytes: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileTreeEntryKind {
    Directory { expanded: bool },
    Video,
    Image,
    Audio,
    Timeline,
    Other,
}

pub fn visible_tree(
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
pub fn search_tree(project_root: &Path, query: &str) -> Result<Vec<FileTreeEntry>, String> {
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

pub fn is_image_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "jpg" | "jpeg" | "png" | "gif" | "webp" | "bmp" | "ico"
            )
        })
}

pub fn is_video_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "mp4" | "mov" | "m4v" | "mkv" | "webm" | "avi"
            )
        })
}

pub fn is_audio_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "aac" | "flac" | "m4a" | "mp3" | "ogg" | "wav"
            )
        })
}

pub fn is_srt_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension.to_ascii_lowercase().as_str(), "srt"))
}

impl Editor {
    pub(super) fn explorer_file_entry(
        &self,
        index: usize,
        entry: &FileTreeEntry,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let path = entry.relative_path.clone();

        let selected = self.explorer.selected_file.as_ref() == Some(&path);
        let active_timeline = matches!(entry.kind, FileTreeEntryKind::Timeline)
            && self
                .timeline
                .as_ref()
                .is_some_and(|timeline| timeline.path == path);
        let media_kind = match entry.kind {
            FileTreeEntryKind::Video => Some(MediaKind::Video),
            FileTreeEntryKind::Image => Some(MediaKind::Image),
            FileTreeEntryKind::Audio => Some(MediaKind::Audio),
            FileTreeEntryKind::Other if is_srt_path(&path) => Some(MediaKind::Srt),
            FileTreeEntryKind::Directory { .. }
            | FileTreeEntryKind::Timeline
            | FileTreeEntryKind::Other => None,
        };
        let media_drag = media_kind.map(|kind| ExplorerMediaDrag {
            relative_path: path.clone(),
            name: entry.name.clone(),
            kind,
        });
        let metadata = file_entry_metadata(entry, active_timeline);

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
            .bg(rgb(if active_timeline {
                0x211a0f
            } else if selected {
                0x1e1b13
            } else {
                PANEL
            }))
            .cursor(CursorStyle::PointingHand)
            .hover(|style| style.bg(rgb(SURFACE_HOVER)))
            .when_some(media_drag, |this, drag| {
                this.cursor(CursorStyle::OpenHand)
                    .on_drag(drag, |drag: &ExplorerMediaDrag, _, _, cx| {
                        cx.new(|_| drag.clone())
                    })
            })
            .on_click(cx.listener({
                let entry = entry.clone();
                move |editor, _, _, cx| {
                    match entry.kind {
                        FileTreeEntryKind::Directory { .. } => {
                            let project_root = editor.global_settings.project_root.clone();
                            if let Err(error) = editor
                                .explorer
                                .toggle_directory(&project_root, entry.relative_path.clone())
                                .and_then(|_| editor.save_explorer_expansion())
                            {
                                eprintln!("{error}");
                            }
                        }
                        FileTreeEntryKind::Timeline => editor
                            .open_timeline(entry.relative_path.clone(), cx)
                            .expect("open_timeline failed"),
                        FileTreeEntryKind::Video
                        | FileTreeEntryKind::Image
                        | FileTreeEntryKind::Audio
                        | FileTreeEntryKind::Other => {
                            editor.select_file(entry.relative_path.clone(), cx);
                        }
                    }
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener({
                    let entry = entry.clone();
                    move |editor, event: &MouseDownEvent, _, cx| {
                        editor.show_file_context_menu(
                            entry.relative_path.clone(),
                            matches!(entry.kind, FileTreeEntryKind::Directory { .. }),
                            event,
                            cx,
                        );
                    }
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
                    .when(active_timeline, |this| {
                        this.font_weight(gpui::FontWeight::SEMIBOLD)
                    })
                    .text_ellipsis()
                    .text_color(rgb(if entry.kind != FileTreeEntryKind::Other {
                        TEXT
                    } else {
                        MUTED
                    }))
                    .child(entry.name.clone()),
            )
            .when_some(metadata, |this, metadata| {
                this.child(
                    div()
                        .flex_shrink_0()
                        .font_family("monospace")
                        .text_xs()
                        .text_ellipsis()
                        .when(active_timeline, |this| {
                            this.h(px(20.0))
                                .px_2()
                                .flex()
                                .items_center()
                                .rounded_sm()
                                .border_1()
                                .border_color(rgb(0x8a652d))
                                .bg(rgb(0x2a241b))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(ACCENT))
                        })
                        .when(!active_timeline, |this| {
                            this.max_w(px(58.0)).text_color(rgb(0x55555e))
                        })
                        .child(metadata),
                )
            })
    }

    pub(super) fn select_file(&mut self, relative_path: PathBuf, cx: &mut Context<Self>) {
        let is_image = is_image_path(&relative_path);
        let is_video = is_video_path(&relative_path);
        let is_audio = is_audio_path(&relative_path);

        self.select_only_clip(None);
        self.explorer.selected_file = Some(relative_path.clone());

        if is_image || is_video || is_audio {
            self.preview.target = match (is_video, is_audio) {
                (true, _) | (_, true) => PreviewTarget::None,
                _ => PreviewTarget::ImageFile(relative_path.clone()),
            };
            self.status = None;
            self.preview.volume_control_open = false;
            self.preview.is_scrubbing = false;
            self.preview.is_adjusting_volume = false;
            self.preview.last_scrub_seek = None;
            self.preview.timeline_drag = None;
        }

        if !is_video && !is_audio {
            return;
        }

        let project_root = self.global_settings.project_root.clone();
        let source_path = project_root.join(&relative_path);
        let Ok(url) = Url::from_file_path(&source_path) else {
            eprintln!("Could not open {}", source_path.display());
            return;
        };
        self.status = Some(format!("Loading preview for {}…", relative_path.display()));

        if is_audio {
            cx.spawn(async move |editor, cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move { AudioBackend::new(&url) })
                    .await;

                editor
                    .update(cx, |editor, cx| {
                        let still_requested =
                            matches!(
                                editor.explorer.selected_file.as_ref(),
                                Some(path) if path == &relative_path
                            ) && matches!(&editor.preview.target, PreviewTarget::None);
                        if editor.global_settings.project_root != project_root || !still_requested {
                            return;
                        }

                        match result {
                            Ok(audio) => {
                                audio.set_playing(false);
                                editor.preview.target =
                                    PreviewTarget::AudioFile(relative_path.clone(), audio);
                                editor.status = Some("Audio preview ready.".to_string());
                            }
                            Err(error) => {
                                editor.status = None;
                                eprintln!("{error}");
                            }
                        }
                        cx.notify();
                    })
                    .ok();
            })
            .detach();
            return;
        }

        let video = VideoBackend::open(&url)
            .map_err(|error| format!("Could not preview {}: {error}", source_path.display()));

        let still_requested = matches!(
            self.explorer.selected_file.as_ref(),
            Some(path) if path == &relative_path
        ) && matches!(&self.preview.target, PreviewTarget::None);
        if self.global_settings.project_root != project_root || !still_requested {
            return;
        }

        match video {
            Ok(video) => {
                self.preview.target = PreviewTarget::VideoFile(relative_path, video);
                self.status = Some("Video preview ready.".to_string());
            }
            Err(error) => {
                self.status = None;
                eprintln!("{error}");
            }
        }
    }
}

fn file_entry_metadata(entry: &FileTreeEntry, active_timeline: bool) -> Option<String> {
    match entry.kind {
        FileTreeEntryKind::Directory { .. } => None,
        FileTreeEntryKind::Timeline if active_timeline => Some("ACTIVE".to_string()),
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
