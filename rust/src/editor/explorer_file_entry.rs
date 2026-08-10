use super::*;

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
        let directory_expanded = match entry.kind {
            FileTreeEntryKind::Directory { expanded } => Some(expanded),
            _ => None,
        };
        let is_directory = directory_expanded.is_some();
        let is_timeline = entry.kind == FileTreeEntryKind::Timeline;
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
                    editor.show_file_context_menu(context_path.clone(), is_directory, event, cx);
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
                    .when_some(directory_expanded, |this, expanded| {
                        this.text_color(rgb(MUTED))
                            .child(if expanded { "▾" } else { "▸" })
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
