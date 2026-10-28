use crate::asset::IconName;
use crate::event_bus::{AppEvent, EventBus};
use crate::theme::{ACCENT, BORDER, ERROR, MUTED, PANEL, SURFACE, TEXT};
use crate::timeline::TimelineRuntimeState;
use gpui::prelude::*;
use gpui::{CursorStyle, Entity, MouseButton, div, px, rgb, svg};
use ulid::Ulid;

/// The missing media dialog's view model: a snapshot of the open timeline's missing files,
/// rebuilt by event handlers.
pub(crate) struct MissingMediaDialog {
    pub(crate) files: Vec<MissingMediaFile>,
}

pub(crate) struct MissingMediaFile {
    pub(crate) asset_id: Ulid,
    pub(crate) name: String,
    pub(crate) clip_count: usize,
}

/// One entry per offline asset that clips use, sorted by file name.
pub(crate) fn missing_media_dialog(timeline: &TimelineRuntimeState) -> MissingMediaDialog {
    let mut files = timeline
        .editing_state
        .assets
        .iter()
        .filter(|asset| timeline.offline_asset_ids.contains(&asset.id))
        .map(|asset| MissingMediaFile {
            asset_id: asset.id,
            name: match asset.path.file_name() {
                Some(name) => name.to_string_lossy().into_owned(),
                None => asset.path.display().to_string(),
            },
            clip_count: timeline
                .editing_state
                .clips
                .iter()
                .filter(|clip| clip.asset_id() == Some(asset.id))
                .count(),
        })
        .filter(|file| file.clip_count > 0)
        .collect::<Vec<_>>();
    files.sort_by(|left, right| left.name.cmp(&right.name));
    MissingMediaDialog { files }
}

pub(crate) fn missing_media_dialog_view(
    dialog: &MissingMediaDialog,
    event_bus: Entity<EventBus>,
) -> gpui::AnyElement {
    let backdrop_event_bus = event_bus.clone();
    let title = match dialog.files.len() {
        1 => "1 media file is missing".to_string(),
        count => format!("{count} media files are missing"),
    };
    div()
        .id("missing-media-dialog-overlay")
        .absolute()
        .inset_0()
        .occlude()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::rgba(0x00000088))
        .on_mouse_down(MouseButton::Left, move |_, _, cx| {
            backdrop_event_bus.update(cx, |_, cx| cx.emit(AppEvent::MissingMediaDialogClosed));
        })
        .child(
            div()
                .id("missing-media-dialog")
                .w(px(560.0))
                .p_5()
                .flex()
                .flex_col()
                .rounded_xl()
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(PANEL))
                .shadow_lg()
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .text_lg()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .mt_3()
                        .text_sm()
                        .text_color(rgb(MUTED))
                        .child("Clips stay on the timeline, shown as offline."),
                )
                .child(
                    div()
                        .mt_5()
                        .px_3()
                        .flex()
                        .flex_col()
                        .rounded_lg()
                        .border_1()
                        .border_color(rgb(BORDER))
                        .bg(rgb(SURFACE))
                        .children(dialog.files.iter().enumerate().map(|(index, file)| {
                            let asset_id = file.asset_id;
                            div()
                                .py_4()
                                .flex()
                                .items_center()
                                .gap_3()
                                .when(index > 0, |row| row.border_t_1().border_color(rgb(BORDER)))
                                .child(
                                    svg()
                                        .path(IconName::AlertTriangle.path())
                                        .size(px(14.0))
                                        .text_color(rgb(ERROR)),
                                )
                                .child(
                                    div()
                                        .min_w_0()
                                        .flex_1()
                                        .child(
                                            div()
                                                .font_family("monospace")
                                                .text_xs()
                                                .text_color(rgb(TEXT))
                                                .child(file.name.clone()),
                                        )
                                        .child(
                                            div().mt_1().text_xs().text_color(rgb(MUTED)).child(
                                                match file.clip_count {
                                                    1 => "1 clip".to_string(),
                                                    count => format!("{count} clips"),
                                                },
                                            ),
                                        ),
                                )
                                .child(
                                    div()
                                        .id(gpui::SharedString::from(format!("relink-{asset_id}")))
                                        .h_9()
                                        .px_3()
                                        .flex()
                                        .items_center()
                                        .rounded_lg()
                                        .border_1()
                                        .border_color(rgb(ACCENT))
                                        .bg(rgb(ACCENT))
                                        .text_sm()
                                        .text_color(rgb(0x17120a))
                                        .cursor(CursorStyle::PointingHand)
                                        .hover(|style| style.opacity(0.85))
                                        .child("Relink media")
                                        .on_click({
                                            let event_bus = event_bus.clone();
                                            move |_, _, cx| {
                                                event_bus.update(cx, |_, cx| {
                                                    cx.emit(AppEvent::RelinkMediaFileChosen {
                                                        asset_id,
                                                    })
                                                });
                                            }
                                        }),
                                )
                        })),
                )
                .child(
                    div()
                        .mt_5()
                        .pt_4()
                        .flex()
                        .justify_end()
                        .border_t_1()
                        .border_color(rgb(BORDER))
                        .child(
                            div()
                                .id("continue-with-offline-media")
                                .h_10()
                                .px_4()
                                .flex()
                                .items_center()
                                .rounded_lg()
                                .border_1()
                                .border_color(rgb(BORDER))
                                .bg(rgb(SURFACE))
                                .text_sm()
                                .text_color(rgb(TEXT))
                                .cursor(CursorStyle::PointingHand)
                                .hover(|style| style.opacity(0.85))
                                .child("Continue with offline media")
                                .on_click(move |_, _, cx| {
                                    event_bus.update(cx, |_, cx| {
                                        cx.emit(AppEvent::MissingMediaDialogClosed)
                                    });
                                }),
                        ),
                ),
        )
        .into_any_element()
}
