use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use gpui::{
    Context, CursorStyle, DragMoveEvent, IntoElement, ParentElement, Render, Styled, Window, div,
    px, rgb,
};
use ulid::Ulid;

use crate::editor::{
    ACCENT, Clip, EditAction, Editor, MediaKind, RULER_HEIGHT, TIMELINE_PADDING, TRACK_HEIGHT,
    TimelineTime, edit_and_rebuild_timeline, editing::validate_clips_placements,
    media_probe::probe_asset, model::DEFAULT_IMAGE_CLIP_DURATION, srt::srt_to_text_clips,
    validate_clip_placement, validate_text_clip_placement,
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

pub(super) fn update_file_drag(
    editor: &mut Editor,
    event: &DragMoveEvent<ExplorerMediaDrag>,
    window: &mut Window,
    cx: &mut Context<Editor>,
) {
    let pointer = event.event.position;
    let inside_timeline = event.bounds.contains(&pointer);
    let local_y = f32::from(pointer.y) - f32::from(event.bounds.top());
    let track_index = ((local_y - RULER_HEIGHT) / TRACK_HEIGHT).floor() as isize;
    let Some(track_id) = inside_timeline
        .then_some(track_index)
        .and_then(|index| usize::try_from(index).ok())
        .and_then(|index| editor.timeline.as_ref()?.data.tracks.get(index))
        .map(|track| track.id)
    else {
        if editor.explorer.drop_preview.take().is_some() {
            if let Some(timeline) = editor.timeline.as_mut() {
                timeline.interaction.snap_guide = None;
            }
            cx.notify();
        }
        cx.set_active_drag_cursor_style(CursorStyle::OperationNotAllowed, window);
        return;
    };

    let drag = event.drag(cx).clone();
    let local_x = f32::from(pointer.x) - f32::from(event.bounds.left());
    let Some(timeline) = editor.timeline.as_ref() else {
        return;
    };
    let raw_start = timeline.data.nearest_time(
        ((local_x - TIMELINE_PADDING) / timeline.data.view.pixels_per_second).max(0.0) as f64,
    );
    refresh_explorer_drop_preview(editor, &drag, track_id, raw_start);
    let invalid = editor
        .explorer
        .drop_preview
        .as_ref()
        .is_none_or(|preview| preview.invalid_reason.is_some());
    cx.set_active_drag_cursor_style(
        if invalid {
            CursorStyle::OperationNotAllowed
        } else {
            CursorStyle::DragCopy
        },
        window,
    );
    cx.notify();
}

fn refresh_explorer_drop_preview(
    editor: &mut Editor,
    drag: &ExplorerMediaDrag,
    track_id: Ulid,
    raw_start: TimelineTime,
) {
    let probed_asset = if drag.kind == MediaKind::Srt {
        None
    } else {
        let source_path = editor
            .global_settings
            .project_root
            .join(&drag.relative_path);
        Some(probe_asset(&source_path))
    };
    let Some(timeline) = editor.timeline.as_ref() else {
        return;
    };
    let (duration, start, snap_guide, invalid_reason) = match drag.kind {
        MediaKind::Srt => {
            let duration = TimelineTime::ONE_FRAME;
            let (start, snap_guide) =
                timeline.snap_clip_start_ignoring(raw_start, duration, &HashSet::new());
            let invalid_reason = validate_text_clip_placement(
                &timeline.data,
                track_id,
                duration,
                start,
                &HashSet::new(),
            )
            .err()
            .map(|rejection| rejection.to_string());
            (duration, start, snap_guide, invalid_reason)
        }
        MediaKind::Video | MediaKind::Image | MediaKind::Audio => {
            let asset = probed_asset.expect("non-SRT drags always probe media metadata");
            let duration = match &asset {
                Ok(asset) => timeline.data.ceil_time(asset.duration),
                Err(_) => timeline.data.ceil_time(DEFAULT_IMAGE_CLIP_DURATION),
            };
            let (start, snap_guide) =
                timeline.snap_clip_start_ignoring(raw_start, duration, &HashSet::new());
            let invalid_reason = match asset {
                Ok(asset) => validate_clip_placement(
                    &timeline.data,
                    track_id,
                    asset.kind,
                    duration,
                    start,
                    &HashSet::new(),
                )
                .err()
                .map(|rejection| rejection.to_string()),
                Err(error) => Some(error.to_string()),
            };
            (duration, start, snap_guide, invalid_reason)
        }
    };
    if let Some(timeline) = editor.timeline.as_mut() {
        timeline.interaction.snap_guide = snap_guide;
    }
    editor.explorer.drop_preview = Some(ExplorerDropPreview {
        kind: drag.kind,
        relative_path: drag.relative_path.clone(),
        name: drag.name.clone(),
        track_id,
        raw_start,
        start,
        duration,
        invalid_reason,
    });
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
    let source_path = editor.global_settings.project_root.join(relative_path);
    let asset = match probe_asset(&source_path) {
        Ok(asset) => asset,
        Err(error) => {
            editor.status = Some(format!("Could not add {name}: {error}."));
            eprintln!("Cannot add {name}: {error:?}");
            cx.notify();
            return;
        }
    };
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
    let result: anyhow::Result<usize> = (|| {
        let mut text_clips = match parsed_clips {
            Ok(clips) => clips,
            Err(error) => return Err(error),
        };
        if text_clips.is_empty() {
            anyhow::bail!("the SRT file contains no subtitle cues");
        }
        for clip in &mut text_clips {
            clip.track_id = preview.track_id;
            clip.timeline_start += preview.start;
        }
        let clip_ids = text_clips.iter().map(|clip| clip.id).collect::<Vec<_>>();
        let clip_count = text_clips.len();
        let clips = text_clips.into_iter().map(Clip::Text).collect::<Vec<_>>();

        let Some(timeline) = editor.timeline.as_mut() else {
            anyhow::bail!("the destination timeline is unavailable");
        };
        if let Err(error) = validate_clips_placements(&timeline.data, &clips) {
            return Err(error.context("could not place subtitle clips"));
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
            return Err(error.context("could not place subtitle clips"));
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
            eprintln!("Cannot add {name}: {error:?}");
        }
    }
    cx.notify();
}
