use crate::editor::Editor;
use crate::explorer_file_entry::{FileTreeEntry, FileTreeEntryKind, is_srt_path};
use crate::model::{DEFAULT_IMAGE_CLIP_DURATION, MediaAsset, MediaKind};
use crate::theme::ACCENT;
use crate::timeline::TimelineFrameIndex;
use ::transcribe::SRT;
use anyhow::Result;
use engine::media_asset::probe_media_asset;
use gpui::{
    Context, CursorStyle, IntoElement, ParentElement, Render, SharedString, Styled, WeakEntity,
    Window, canvas, div, px, rgb,
};
use std::fs::read_to_string;
use std::path::PathBuf;
use ulid::Ulid;

#[derive(Clone, Debug)]
struct ExplorerDropPreview {
    kind: MediaKind,
    absolute_path: PathBuf,
    name: String,
    track_id: Ulid,
    raw_start: TimelineFrameIndex,
    start: TimelineFrameIndex,
    duration: TimelineFrameIndex,
    invalid_reason: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) enum AssetBeingDragged {
    Media(DraggedMedia),
    Srt(DraggedSRT),
    None,
}

impl AssetBeingDragged {
    pub(crate) fn from_file_entry(entry: &FileTreeEntry) -> Result<Self> {
        let x = match entry.kind {
            FileTreeEntryKind::Video | FileTreeEntryKind::Image | FileTreeEntryKind::Audio => {
                let metadata =
                    probe_media_asset(&entry.absolute_path, DEFAULT_IMAGE_CLIP_DURATION)?;
                Self::Media(DraggedMedia {
                    absolute_path: entry.absolute_path.clone(),
                    metadata,
                })
            }
            FileTreeEntryKind::Directory { .. } | FileTreeEntryKind::Timeline => Self::None,
            FileTreeEntryKind::Other => {
                if !is_srt_path(&entry.absolute_path) {
                    return Ok(Self::None);
                }
                let text = read_to_string(&entry.absolute_path)?;
                let srt = SRT::from_string(&text)?;
                Self::Srt(DraggedSRT {
                    absolute_path: entry.absolute_path.clone(),
                    srt,
                })
            }
        };
        Ok(x)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DraggedMedia {
    pub(super) absolute_path: PathBuf,
    pub(super) metadata: MediaAsset,
}

impl DraggedMedia {
    pub(crate) fn name(&self) -> SharedString {
        self.absolute_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.absolute_path.display().to_string())
            .into()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DraggedSRT {
    absolute_path: PathBuf,
    pub(crate) srt: SRT,
}

impl DraggedSRT {
    fn name(&self) -> SharedString {
        self.absolute_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.absolute_path.display().to_string())
            .into()
    }
}

pub(super) struct ExplorerDragView {
    pub(super) asset: AssetBeingDragged,
    pub(super) editor: WeakEntity<Editor>,
}

impl Render for ExplorerDragView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (kind, name) = match &self.asset {
            AssetBeingDragged::Media(asset) => (asset.metadata.kind.label(), asset.name()),
            AssetBeingDragged::Srt(asset) => ("SRT", asset.name()),
            AssetBeingDragged::None => return gpui::Empty.into_any_element(),
        };

        let placement_cursor = (|| {
            let editor = self.editor.upgrade()?;
            let timeline = editor.read(cx).timeline.as_ref()?;
            let preview = timeline.preview_drop_asset.as_ref()?;
            Some(
                if preview.validate_placement(&timeline.editing_state).is_err() {
                    CursorStyle::OperationNotAllowed
                } else {
                    CursorStyle::ClosedHand
                },
            )
        })();
        if let Some(cursor) = placement_cursor {
            // 轨道内只显示共用的落点预览，避免素材浮层挡住错误原因。
            return canvas(
                |_, _, _| {},
                move |_, (), window, _| {
                    window.set_window_cursor_style(cursor);
                },
            )
            .size(px(1.0))
            .into_any_element();
        }

        div()
            .relative()
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
            // GPUI 会沿用拖拽起点的光标；在拖拽浮层最后绘制时覆盖它。
            .child(
                canvas(
                    |_, _, _| {},
                    move |_, (), window, _| {
                        window.set_window_cursor_style(CursorStyle::ClosedHand);
                    },
                )
                .absolute()
                .inset_0()
                .size_full(),
            )
            .into_any_element()
    }
}
