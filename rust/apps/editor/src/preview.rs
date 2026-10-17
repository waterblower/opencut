use crate::asset::IconName;
use crate::editor::Editor;
use crate::preview_image::preview_image_file;
use crate::theme::MUTED;
use crate::timeline::{TimelinePlayerExt, TimelineRuntimeState};
use anyhow::{Context as _, Result};
use gpui::prelude::*;
use gpui::{Entity, FontWeight, div, px, rgb, svg};
use player_ui::audio_player::AudioPlayer;
use player_ui::timeline_player::TimelinePlayer;
use player_ui::video_player::VideoPlayer;
use std::path::{Path, PathBuf};

pub enum PreviewTarget {
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
pub fn timeline_preview_target<T: 'static>(
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
                .overflow_y_scroll()
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
                        .id("preview-error-message")
                        .h(px(192.0))
                        .flex_shrink_0()
                        .p(px(16.0))
                        .rounded(px(6.0))
                        .border_1()
                        .border_color(rgb(0x463033))
                        .bg(rgb(0x151113))
                        .overflow_y_scroll()
                        .font_family("monospace")
                        .text_size(px(13.0))
                        .line_height(px(24.0))
                        .text_color(rgb(0xe6b3b3))
                        .whitespace_normal()
                        .children(error.split('\n').map(|line| {
                            div().min_h(px(24.0)).child(line.replace('/', "/\u{200b}"))
                        })),
                )
                .into_any_element(),
            PreviewTarget::None => div()
                .w(px(width))
                .h(px(height))
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(MUTED))
                .child("No preview available")
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
}
