use crate::edit_action::{EditAction, edit_timeline};
use crate::editor::Editor;
use crate::preview::{PreviewTarget, timeline_preview_target};
use crate::theme::{ACCENT, BORDER, MUTED, PANEL, SURFACE, SURFACE_HOVER, TEXT};
use crate::timeline::{FRAME_RATE_PRESETS, FrameRate};
use ::timeline::TimelineEditingState;
use anyhow::{Result, bail};
use gpui::prelude::*;
use gpui::{CursorStyle, MouseButton, div, px, rgb};

pub fn settings_modal_view(
    timeline: &TimelineEditingState,
    cx: &mut Context<Editor>,
) -> gpui::AnyElement {
    let selected = timeline.settings.frame_rate;
    let options = FRAME_RATE_PRESETS
        .into_iter()
        .enumerate()
        .map(|(index, (frame_rate, label))| {
            let active = frame_rate == selected;
            div()
                .id(("timeline-frame-rate", index))
                .h(px(44.0))
                .px_3()
                .flex()
                .items_center()
                .justify_between()
                .rounded_lg()
                .border_1()
                .border_color(rgb(if active { ACCENT } else { BORDER }))
                .bg(rgb(if active { 0x2a241b } else { SURFACE }))
                .cursor(CursorStyle::PointingHand)
                .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                .child(label)
                .child(div().size_2().rounded_full().bg(rgb(if active {
                    ACCENT
                } else {
                    0x45454d
                })))
                .on_click(cx.listener(move |editor, _, _, cx| {
                    if let Err(error) = ui_callback_set_timeline_frame_rate(editor, frame_rate, cx)
                    {
                        log::error!("{error:?}");
                    }
                    cx.notify();
                }))
        })
        .collect::<Vec<_>>();

    div()
        .id("project-settings-overlay")
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .occlude()
        .bg(gpui::rgba(0x000000b3))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|editor, _, _, cx| {
                editor.settings_open = false;
                cx.notify();
            }),
        )
        .child(
            div()
                .id("project-settings-modal")
                .w(px(460.0))
                .flex()
                .flex_col()
                .rounded_xl()
                .border_1()
                .border_color(rgb(BORDER))
                .bg(rgb(PANEL))
                .shadow_lg()
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_, _, _, cx| cx.stop_propagation()),
                )
                .child(
                    div()
                        .h(px(58.0))
                        .px_5()
                        .flex()
                        .items_center()
                        .justify_between()
                        .border_b_1()
                        .border_color(rgb(BORDER))
                        .child(
                            div()
                                .text_lg()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .child("Timeline Settings"),
                        )
                        .child(
                            div()
                                .id("close-project-settings")
                                .size_8()
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded_md()
                                .cursor(CursorStyle::PointingHand)
                                .text_color(rgb(MUTED))
                                .hover(|style| {
                                    style.bg(rgb(SURFACE_HOVER)).text_color(rgb(TEXT))
                                })
                                .child("×")
                                .on_click(cx.listener(|editor, _, _, cx| {
                                    editor.settings_open = false;
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    div()
                        .p_5()
                        .flex()
                        .flex_col()
                        .gap_4()
                        .child(
                            div()
                                .text_xs()
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(MUTED))
                                .child("TIMELINE FRAME RATE"),
                        )
                        .child(div().grid().grid_cols(2).gap_2().children(options))
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(MUTED))
                                .child(
                                    "Existing edit points keep their elapsed time and snap to the nearest frame in the new rate.",
                                ),
                        ),
                ),
        )
        .into_any_element()
}

fn ui_callback_set_timeline_frame_rate(
    editor: &mut Editor,
    frame_rate: FrameRate,
    cx: &mut Context<Editor>,
) -> Result<()> {
    editor.settings_open = false;
    let Some(timeline) = editor.timeline.as_mut() else {
        bail!("Cannot change frame rate when no timeline is open");
    };
    let previous = timeline.editing_state.settings.frame_rate;
    if previous == frame_rate {
        return Ok(());
    }

    let should_save = edit_timeline(timeline, EditAction::SetFrameRate { frame_rate })?;
    let has_clips = !timeline.editing_state.clips.is_empty();
    if has_clips {
        // 播放头存的是帧号；按旧帧率换算回时间，再取新帧率下最近的帧，与片段的换算方式一致。
        timeline.set_playhead(previous.rescale_nearest(timeline.playhead(), frame_rate));
    }
    if should_save {
        timeline.save()?;
    }
    // 预览播放器持有旧帧率的快照；换成新快照，旧播放器随之释放并停止。
    if matches!(editor.preview.target, PreviewTarget::Timeline { .. }) {
        editor.preview.target = timeline_preview_target(timeline, &editor.project_root, cx)?;
    }
    Ok(())
}
