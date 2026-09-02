use std::{fs::read_to_string, path::PathBuf};

use gpui::{
    Context, IntoElement, ParentElement, Render, SharedString, Styled, Window, div, px, rgb,
};
use ulid::Ulid;

use crate::editor::{
    ACCENT, MediaAsset, MediaKind, TimelineTime,
    explorer::{FileTreeEntry, FileTreeEntryKind, is_srt_path},
    media_probe::probe_asset,
};

#[derive(Clone, Debug)]
pub(super) struct ExplorerDropPreview {
    pub(super) kind: MediaKind,
    pub(super) absolute_path: PathBuf,
    pub(super) name: String,
    pub(super) track_id: Ulid,
    pub(super) raw_start: TimelineTime,
    pub(super) start: TimelineTime,
    pub(super) duration: TimelineTime,
    pub(super) invalid_reason: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) enum AssetBeingDragged {
    V1(AssetBeingDraggedV1),
    Srt(DraggedSRT),
    None,
}

impl AssetBeingDragged {
    pub fn from_file_entry(entry: &FileTreeEntry) -> Self {
        let x = match entry.kind {
            FileTreeEntryKind::Video | FileTreeEntryKind::Image | FileTreeEntryKind::Audio => {
                let metadata = match probe_asset(&entry.absolute_path) {
                    Ok(metadata) => metadata,
                    Err(_) => return Self::None,
                };
                Self::V1(AssetBeingDraggedV1 {
                    absolute_path: entry.absolute_path.clone(),
                    metadata,
                })
            }
            FileTreeEntryKind::Directory { .. } | FileTreeEntryKind::Timeline => Self::None,
            FileTreeEntryKind::Other => {
                if !is_srt_path(&entry.absolute_path) {
                    return Self::None;
                }
                Self::Srt(DraggedSRT {
                    absolute_path: entry.absolute_path.clone(),
                    text: read_to_string(&entry.absolute_path).unwrap_or_default(),
                })
            }
        };
        return x;
    }
}

#[derive(Clone, Debug)]
pub(super) struct AssetBeingDraggedV1 {
    pub(super) absolute_path: PathBuf,
    pub(super) metadata: MediaAsset,
}

impl AssetBeingDraggedV1 {
    pub fn name(&self) -> SharedString {
        self.absolute_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.absolute_path.display().to_string())
            .into()
    }
}

#[derive(Clone, Debug)]
pub struct DraggedSRT {
    pub absolute_path: PathBuf,
    pub text: String,
}

impl DraggedSRT {
    pub fn name(&self) -> SharedString {
        self.absolute_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.absolute_path.display().to_string())
            .into()
    }
}

impl Render for AssetBeingDragged {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let (kind, name) = match self {
            Self::V1(asset) => (asset.metadata.kind.label(), asset.name()),
            Self::Srt(asset) => ("SRT", asset.name()),
            Self::None => return gpui::Empty.into_any_element(),
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
                    .child(kind),
            )
            .child(div().min_w_0().text_sm().text_ellipsis().child(name))
            .into_any_element()
    }
}
