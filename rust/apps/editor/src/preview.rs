use crate::editor::Editor;
use crate::preview_image::preview_image_file;
use crate::theme::MUTED;
use gpui::prelude::*;
use gpui::{Entity, div, px, rgb};
use player_ui::audio_player::AudioPlayer;
use player_ui::timeline_player::TimelinePlayer;
use player_ui::video_player::VideoPlayer;
use std::path::PathBuf;

pub enum PreviewTarget {
    None,
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

impl Editor {
    pub(super) fn preview_player(&self, width: f32, height: f32) -> gpui::AnyElement {
        match &self.preview.target {
            PreviewTarget::None => div()
                .w(px(width))
                .h(px(height))
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(MUTED))
                .child("No preview available")
                .into_any_element(),
            PreviewTarget::Timeline { player, .. } => div()
                .w(px(width))
                .h(px(height))
                .child(player.clone())
                .into_any_element(),
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
