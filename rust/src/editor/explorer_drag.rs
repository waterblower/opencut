use std::path::PathBuf;

use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div, px, rgb};
use ulid::Ulid;

use crate::editor::{ACCENT, MediaKind, TimelineTime};

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
