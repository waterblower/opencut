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
        let metadata = if is_timeline
            && self
                .timeline
                .as_ref()
                .is_some_and(|timeline| timeline.path == path)
        {
            Some("ACTIVE".to_string())
        } else {
            explorer_metadata(
                entry,
                self.timeline.as_ref().and_then(|timeline| {
                    timeline.data.assets.iter().find(|asset| asset.path == path)
                }),
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
    }
}
