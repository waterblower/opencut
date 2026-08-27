use std::path::{Path, PathBuf};

use gpui::{Context, IntoElement, ParentElement, Render, Styled, Window, div, px, rgb};
use ulid::Ulid;

use crate::editor::{
    ACCENT, Clip, EditAction, Editor, MediaKind, TimelineTime, edit_and_rebuild_timeline,
    editing::validate_clips_placements, srt::srt_to_text_clips,
};

#[derive(Clone, Debug)]
pub(super) struct ExplorerDropPreview {
    pub(super) kind: MediaKind,
    pub(super) relative_path: PathBuf,
    pub(super) name: String,
    pub(super) track_id: Ulid,
    pub(super) raw_start: TimelineTime,
    pub(super) start: TimelineTime,
    pub(super) duration: TimelineTime,
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
            name,
            relative_path,
        } => {
            drop_dragged_srt(name, relative_path, editor, cx);
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

    // Probe synchronously if the drag preview did not already cache metadata.
    let asset = editor
        .probe_explorer_drag_asset(relative_path)
        .expect("probe_explorer_drag_asset failed");
    editor.place_explorer_asset(
        relative_path.to_path_buf(),
        preview.track_id,
        preview.raw_start,
        asset,
        cx,
    );

    // Refresh the editor to clear the drag UI and show the placed asset.
    cx.notify();
}

fn drop_dragged_srt(
    name: &str,
    relative_path: &Path,
    editor: &mut Editor,
    cx: &mut Context<Editor>,
) {
    // The drag preview identifies both the text track under the pointer and the
    // timeline position where the subtitle timestamps should begin.
    let Some(preview) = editor.explorer.drop_preview.take().filter(|preview| {
        preview.relative_path == relative_path
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
        eprintln!("Cannot add {name}: {reason}.");
        editor.status = None;
        cx.notify();
        return;
    }

    let Some(timeline) = editor.timeline.as_ref() else {
        return;
    };
    let frame_rate = timeline.data.settings.frame_rate;
    let project_root = editor.global_settings.project_root.clone();
    let source_path = project_root.join(relative_path);
    let parsed_clips = srt_to_text_clips(&source_path, frame_rate);
    let result = (|| {
        let mut text_clips = match parsed_clips {
            Ok(clips) => clips,
            Err(error) => return Err(error),
        };
        if text_clips.is_empty() {
            return Err("the SRT file contains no subtitle cues".to_string());
        }
        for clip in &mut text_clips {
            clip.track_id = preview.track_id;
            clip.timeline_start += preview.start;
        }
        let clip_ids = text_clips.iter().map(|clip| clip.id).collect::<Vec<_>>();
        let clip_count = text_clips.len();
        let clips = text_clips.into_iter().map(Clip::Text).collect::<Vec<_>>();

        let Some(timeline) = editor.timeline.as_mut() else {
            return Err("the destination timeline is unavailable".to_string());
        };
        if let Err(error) = validate_clips_placements(&timeline.data, &clips) {
            return Err(format!(
                "could not place subtitle clips: {}",
                error.message()
            ));
        }
        timeline.record_editing_history();
        if let Err(error) = edit_and_rebuild_timeline(
            &mut editor.preview,
            &project_root,
            timeline,
            EditAction::AddClips {
                clips,
                assets: Vec::new(),
            },
        ) {
            return Err(format!("could not place subtitle clips: {error}"));
        }
        timeline.interaction.selected_clip_id = clip_ids.first().copied();
        timeline.interaction.selected_clip_ids = clip_ids.iter().copied().collect();
        timeline.save(&project_root);
        editor.explorer.selected_file = Some(relative_path.to_path_buf());
        Ok(clip_count)
    })();

    match result {
        Ok(clip_count) => {
            editor.status = Some(format!("Added {clip_count} subtitle clips from {name}."));
        }
        Err(error) => {
            editor.status = Some(format!("Could not import {name}: {error}."));
            eprintln!("Cannot add {name}: {error}.");
        }
    }
    cx.notify();
}
