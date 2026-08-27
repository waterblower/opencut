use std::path::PathBuf;

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
    editor: &mut Editor,
    drag: &ExplorerMediaDrag,
    cx: &mut Context<Editor>,
) {
    if drag.kind == MediaKind::Srt {
        return;
    }
    let Some(preview) = editor.explorer.drop_preview.take().filter(|preview| {
        preview.relative_path == drag.relative_path
            && editor
                .timeline
                .as_ref()
                .is_some_and(|timeline| timeline.data.track(preview.track_id).is_some())
    }) else {
        if let Some(timeline) = editor.timeline.as_mut() {
            timeline.interaction.snap_guide = None;
        }
        return;
    };
    if let Some(timeline) = editor.timeline.as_mut() {
        timeline.interaction.snap_guide = None;
    }

    if let Some(reason) = preview.invalid_reason {
        eprintln!("Cannot add {}: {reason}.", drag.name);
        editor.status = None;
        cx.notify();
        return;
    }

    let Some(timeline) = editor.timeline.as_ref() else {
        return;
    };
    if let Some(asset) = explorer_asset_for_path(
        &timeline.data.assets,
        &editor.explorer.drag_assets,
        &drag.relative_path,
    ) {
        editor.place_explorer_asset(
            drag.relative_path.clone(),
            preview.track_id,
            preview.raw_start,
            asset,
            cx,
        );
    } else {
        editor.explorer.pending_drop = Some(PendingExplorerDrop {
            relative_path: drag.relative_path.clone(),
            track_id: preview.track_id,
            raw_start: preview.raw_start,
        });
        editor.status = Some(format!("Inspecting {} before placing it…", drag.name));
        editor.request_explorer_drag_probe(drag.relative_path.clone(), cx);
    }
    cx.notify();
}
