use std::{
    fs::read_to_string,
    path::{Path, PathBuf},
};

use gpui::{
    Context, DragMoveEvent, IntoElement, ParentElement, Render, SharedString, Styled, Window, div,
    px, rgb,
};
use ulid::Ulid;

use crate::editor::{
    ACCENT, Editor, MediaKind, RULER_HEIGHT, TRACK_HEIGHT, TimelineRuntimeState, TimelineTime,
    edit_and_rebuild_timeline,
    editing::validate_clips_placements,
    explorer::{FileTreeEntry, FileTreeEntryKind, is_srt_path},
    media_probe::probe_asset,
    model::DEFAULT_IMAGE_CLIP_DURATION,
    srt::srt_to_text_clips,
    validate_clip_placement, validate_text_clip_placement,
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
}

impl AssetBeingDragged {
    pub fn from_file_entry(entry: &FileTreeEntry) -> Option<Self> {
        let x = match entry.kind {
            FileTreeEntryKind::Video | FileTreeEntryKind::Image | FileTreeEntryKind::Audio => {
                Some(Self::V1(AssetBeingDraggedV1 {
                    kind: match entry.kind {
                        FileTreeEntryKind::Video => MediaKind::Video,
                        FileTreeEntryKind::Image => MediaKind::Image,
                        FileTreeEntryKind::Audio => MediaKind::Audio,
                        _ => unreachable!("the outer match only accepts media files"),
                    },
                    name: entry.name.clone(),
                    absolute_path: entry.absolute_path.clone(),
                }))
            }
            FileTreeEntryKind::Directory { .. } | FileTreeEntryKind::Timeline => None,
            FileTreeEntryKind::Other => {
                if !is_srt_path(&entry.absolute_path) {
                    return None;
                }
                Some(Self::Srt(DraggedSRT {
                    absolute_path: entry.absolute_path.clone(),
                    text: read_to_string(&entry.absolute_path).unwrap_or_default(),
                }))
            }
        };
        return x;
    }
}

#[derive(Clone, Debug)]
pub(super) struct AssetBeingDraggedV1 {
    pub(super) kind: MediaKind,
    pub(super) name: String,
    pub(super) absolute_path: PathBuf,
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
        let kind = match self {
            Self::V1(asset) => asset.kind.label(),
            Self::Srt(_) => "SRT",
        };
        let name = match self {
            Self::V1(asset) => asset.name(),
            Self::Srt(asset) => asset.name(),
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
    }
}

pub(super) fn drop_dragged_explorer_media(
    drag: &AssetBeingDragged,
    editor: &mut Editor,
    cx: &mut Context<Editor>,
) {
    todo!("on drop");
}

fn drop_dragged_srt(
    name: &str,
    absolute_path: &Path,
    editor: &mut Editor,
    cx: &mut Context<Editor>,
) {
}
