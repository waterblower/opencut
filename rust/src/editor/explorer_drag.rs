use std::path::{Path, PathBuf};

use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div, px, rgb};
use ulid::Ulid;

use crate::editor::{
    ACCENT, Editor, MediaKind, PendingExplorerDrop, TimelineTime, explorer::explorer_asset_for_path,
};

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
pub(super) struct ExplorerMediaDrag {
    pub(super) kind: MediaKind,
    pub(super) name: String,
    pub(super) relative_path: PathBuf,
}

impl Render for ExplorerMediaDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
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
                    .child(self.kind.label()),
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

pub(super) fn drop_dragged_explorer_media(
    drag: &ExplorerMediaDrag,
    editor: &mut Editor,
    cx: &mut Context<Editor>,
) {
    match drag {
        ExplorerMediaDrag {
            kind: MediaKind::Video,
            name,
            relative_path,
        } => {
            // Video metadata determines both its duration and whether it has an
            // audio stream, so use the standard probed-media placement flow.
            drop_dragged_timeline_media(name, relative_path, editor, cx);
        }
        ExplorerMediaDrag {
            kind: MediaKind::Image,
            name,
            relative_path,
        } => {
            // Images use the same placement flow, with their default duration
            // supplied by the media probe and placement logic.
            drop_dragged_timeline_media(name, relative_path, editor, cx);
        }
        ExplorerMediaDrag {
            kind: MediaKind::Audio,
            name,
            relative_path,
        } => {
            // Audio is placed through the standard flow; track validation later
            // ensures that the selected destination accepts audio media.
            drop_dragged_timeline_media(name, relative_path, editor, cx);
        }
        ExplorerMediaDrag {
            kind: MediaKind::Srt,
            ..
        } => {
            // Subtitle timeline placement is not supported yet.
        }
    }
}

fn drop_dragged_timeline_media(
    name: &str,
    relative_path: &Path,
    editor: &mut Editor,
    cx: &mut Context<Editor>,
) {
    // Consume the preview so a completed drop cannot be reused. Only accept it
    // when it still belongs to this drag and its destination track still exists.
    let Some(preview) = editor.explorer.drop_preview.take().filter(|preview| {
        preview.relative_path == relative_path
            && editor
                .timeline
                .as_ref()
                .is_some_and(|timeline| timeline.data.track(preview.track_id).is_some())
    }) else {
        // A stale or missing preview cancels the drop, including any snap guide
        // left behind while dragging.
        if let Some(timeline) = editor.timeline.as_mut() {
            timeline.interaction.snap_guide = None;
        }
        return;
    };

    // The drag interaction has ended, so the snapping indicator is no longer
    // relevant even when the drop continues successfully.
    if let Some(timeline) = editor.timeline.as_mut() {
        timeline.interaction.snap_guide = None;
    }

    // Preview validation may reject a location before any timeline mutation.
    if let Some(reason) = preview.invalid_reason {
        eprintln!("Cannot add {name}: {reason}.");
        editor.status = None;
        cx.notify();
        return;
    }

    // Reborrow the timeline immutably to look up any metadata already known for
    // the dragged path; keep the guard defensive if this invariant changes.
    let Some(timeline) = editor.timeline.as_ref() else {
        return;
    };

    // Place known media immediately using the unsnapped pointer time; the
    // placement routine applies the current snapping and duration rules.
    if let Some(asset) = explorer_asset_for_path(
        &timeline.data.assets,
        &editor.explorer.drag_assets,
        relative_path,
    ) {
        editor.place_explorer_asset(
            relative_path.to_path_buf(),
            preview.track_id,
            preview.raw_start,
            asset,
            cx,
        );
    } else {
        // Unknown media needs metadata inspection first. Preserve the intended
        // destination so the asynchronous probe can finish the placement.
        editor.explorer.pending_drop = Some(PendingExplorerDrop {
            relative_path: relative_path.to_path_buf(),
            track_id: preview.track_id,
            raw_start: preview.raw_start,
        });
        editor.status = Some(format!("Inspecting {name} before placing it…"));
        editor.request_explorer_drag_probe(relative_path.to_path_buf(), cx);
    }

    // Refresh the editor to clear drag UI and show either the placed asset or
    // the pending inspection status.
    cx.notify();
}
