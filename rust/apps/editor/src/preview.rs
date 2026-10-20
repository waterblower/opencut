use crate::asset::IconName;
use crate::editor::Editor;
use crate::preview_image::preview_image_file;
use crate::theme::MUTED;
use crate::timeline::{TimelinePlayerExt, TimelineRuntimeState};
use anyhow::{Context as _, Result};
use gpui::prelude::*;
use gpui::{
    BorderStyle, Bounds, DragMoveEvent, Empty, Entity, FontWeight, MouseButton, Pixels,
    ScrollHandle, canvas, div, point, px, quad, rgb, size, svg,
};
use player_ui::audio_player::AudioPlayer;
use player_ui::timeline_player::TimelinePlayer;
use player_ui::video_player::VideoPlayer;
use std::path::{Path, PathBuf};

pub(crate) enum PreviewTarget {
    None,
    Error {
        path: PathBuf, // 本次选择失败的文件，相对于项目根目录。
        error: String,
    },
    Timeline {
        _task: gpui::Task<()>, // 按字段声明顺序释放：先取消播放任务，再释放播放器。
        _subscription: gpui::Subscription, // 预览替换时取消观察；播放器通知只触发编辑器重绘。
        path: PathBuf,
        player: Entity<TimelinePlayer>,
    },
    VideoFile {
        _task: gpui::Task<()>, // 按字段声明顺序释放：先取消播放任务，再释放播放器。
        path: PathBuf,
        player: Entity<VideoPlayer>,
    },
    AudioFile {
        _task: gpui::Task<()>, // 按字段声明顺序释放：先取消播放任务，再释放播放器。
        path: PathBuf,
        player: Entity<AudioPlayer>,
    },
    ImageFile(PathBuf),
}

/// Builds a paused player snapshot at the timeline's playhead and observes it for redraws.
/// Rebuild the preview when edits invalidate the snapshot's time base, such as frame rate changes.
pub(crate) fn timeline_preview_target<T: 'static>(
    timeline: &TimelineRuntimeState,
    project_root: &Path,
    cx: &mut Context<T>,
) -> Result<PreviewTarget> {
    let path = timeline.path.strip_prefix(project_root)?.to_path_buf();
    let timeline_player =
        TimelinePlayer::from_runtime_state(timeline).context("creating timeline player")?;
    let player = cx.new(move |_| timeline_player);
    let task = player.update(cx, |player, cx| player.start(cx));
    Ok(PreviewTarget::Timeline {
        _task: task,
        _subscription: cx.observe(&player, |_, _, cx| cx.notify()),
        path,
        player,
    })
}

impl Editor {
    pub(super) fn preview_player(
        &self,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        match &self.preview.target {
            PreviewTarget::Error { path, error } => div()
                .id("preview-error")
                .w(px(width))
                .h(px(height))
                .p(px(32.0))
                .flex()
                .flex_col()
                .gap(px(32.0))
                .bg(rgb(0x080808))
                .overflow_hidden()
                .child(
                    div()
                        .flex()
                        .flex_shrink_0()
                        .items_start()
                        .gap(px(16.0))
                        .child(
                            div()
                                .size(px(40.0))
                                .flex_shrink_0()
                                .rounded(px(10.0))
                                .bg(rgb(0x2b171a))
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(
                                    svg()
                                        .path(IconName::AlertTriangle.path())
                                        .size(px(24.0))
                                        .text_color(rgb(0xed8585)),
                                ),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .whitespace_normal()
                                .child(
                                    div()
                                        .text_size(px(18.0))
                                        .line_height(px(24.0))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(rgb(0xf2f2f4))
                                        .child("Unable to preview file"),
                                )
                                .child(
                                    div()
                                        .mt(px(4.0))
                                        .font_family("monospace")
                                        .text_size(px(12.0))
                                        .line_height(px(18.0))
                                        .text_color(rgb(0xb4b4bc))
                                        .child(
                                            path.display().to_string().replace('/', "/\u{200b}"),
                                        ),
                                ),
                        ),
                )
                .child(
                    div()
                        .relative()
                        .flex_1()
                        .min_h_0()
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(rgb(0x463033))
                        .bg(rgb(0x151113))
                        .overflow_hidden()
                        .child(
                            div()
                                .id("preview-error-message")
                                .size_full()
                                .p(px(16.0))
                                .pr(px(28.0))
                                .overflow_y_scroll()
                                .track_scroll(&self.preview.error_scroll)
                                .font_family("monospace")
                                .text_size(px(13.0))
                                .line_height(px(24.0))
                                .text_color(rgb(0xe6b3b3))
                                .whitespace_normal()
                                .children(error.split('\n').map(|line| {
                                    div().min_h(px(24.0)).child(line.replace('/', "/\u{200b}"))
                                })),
                        )
                        .child(
                            div()
                                .id("preview-error-scrollbar")
                                .absolute()
                                .top(px(4.0))
                                .bottom(px(4.0))
                                .right(px(4.0))
                                .w(px(8.0))
                                .cursor_pointer()
                                .on_mouse_down(MouseButton::Left, {
                                    let scroll = self.preview.error_scroll.clone();
                                    move |event, _, cx| {
                                        scroll_error_to_pointer(&scroll, event.position.y);
                                        cx.stop_propagation();
                                        cx.refresh_windows();
                                    }
                                })
                                .on_drag(self.preview.error_scroll.clone(), |_, _, _, cx| {
                                    cx.new(|_| Empty)
                                })
                                .on_drag_move::<ScrollHandle>({
                                    let scroll = self.preview.error_scroll.clone();
                                    move |event: &DragMoveEvent<ScrollHandle>, _, cx| {
                                        scroll_error_to_pointer(&scroll, event.event.position.y);
                                        cx.refresh_windows();
                                    }
                                })
                                .child(
                                    canvas(|_, _, _| (), {
                                        let scroll = self.preview.error_scroll.clone();
                                        move |bounds, _, window, _| {
                                            let track_height = f32::from(bounds.size.height);
                                            if track_height <= 0.0 {
                                                return;
                                            }
                                            let thumb_height =
                                                error_scroll_thumb_height(&scroll, track_height);
                                            let maximum = f32::from(scroll.max_offset().y);
                                            let fraction = if maximum > 0.0 {
                                                (-f32::from(scroll.offset().y) / maximum)
                                                    .clamp(0.0, 1.0)
                                            } else {
                                                0.0
                                            };
                                            window.paint_quad(quad(
                                                bounds,
                                                px(4.0),
                                                rgb(0x2b2023),
                                                px(0.0),
                                                rgb(0x2b2023),
                                                BorderStyle::Solid,
                                            ));
                                            let thumb = Bounds::new(
                                                point(
                                                    bounds.left(),
                                                    bounds.top()
                                                        + px(fraction
                                                            * (track_height - thumb_height)),
                                                ),
                                                size(bounds.size.width, px(thumb_height)),
                                            );
                                            window.paint_quad(quad(
                                                thumb,
                                                px(4.0),
                                                rgb(0x88676b),
                                                px(0.0),
                                                rgb(0x88676b),
                                                BorderStyle::Solid,
                                            ));
                                        }
                                    })
                                    .size_full(),
                                ),
                        ),
                )
                .into_any_element(),
            PreviewTarget::None => div()
                .w(px(width))
                .h(px(height))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_1()
                .child(
                    div()
                        .text_color(rgb(0xb4b4bc))
                        .child("No preview available"),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .text_color(rgb(MUTED))
                        .child("Select an image, audio, video, or timeline file."),
                )
                .into_any_element(),
            PreviewTarget::Timeline { player, .. } => {
                let picture = self.preview_timeline_picture(player, cx);
                let player_view =
                    player.update(cx, |player, cx| player.render_with_picture(picture, cx));
                div()
                    .relative()
                    .w(px(width))
                    .h(px(height))
                    .child(player_view)
                    .child(self.preview_text_overlay(player, cx))
                    .into_any_element()
            }
            PreviewTarget::VideoFile { player, .. } => div()
                .w(px(width))
                .h(px(height))
                .child(player.clone())
                .into_any_element(),
            PreviewTarget::AudioFile { player, .. } => div()
                .w(px(width))
                .h(px(height))
                .child(player.clone())
                .into_any_element(),
            PreviewTarget::ImageFile(path) => {
                preview_image_file(self.project_root.join(path), width, height)
            }
        }
    }
}

pub(crate) struct PreviewState {
    pub(crate) target: PreviewTarget,
    pub(crate) fullscreen: bool,
    pub(crate) error_scroll: ScrollHandle,
}

fn error_scroll_thumb_height(scroll: &ScrollHandle, track_height: f32) -> f32 {
    let viewport = f32::from(scroll.bounds().size.height);
    let content = viewport + f32::from(scroll.max_offset().y);
    if content <= 0.0 {
        return track_height;
    }
    (track_height * viewport / content)
        .max(24.0)
        .min(track_height)
}

fn scroll_error_to_pointer(scroll: &ScrollHandle, pointer_y: Pixels) {
    let bounds = scroll.bounds();
    let track_height = (f32::from(bounds.size.height) - 8.0).max(0.0);
    let thumb_height = error_scroll_thumb_height(scroll, track_height);
    let travel = track_height - thumb_height;
    if travel <= 0.0 {
        return;
    }
    let position = f32::from(pointer_y - bounds.top()) - 4.0 - thumb_height / 2.0;
    let fraction = (position / travel).clamp(0.0, 1.0);
    scroll.set_offset(point(scroll.offset().x, -scroll.max_offset().y * fraction));
}
