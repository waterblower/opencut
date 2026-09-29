use super::*;
use crate::editor::preview_timeline::timeline_preview;
use player_ui::{audio_player::AudioPlayer, video_player::VideoPlayer};
use preview_image::preview_image_file;

pub enum PreviewTarget {
    None,
    Timeline,
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

impl PreviewTarget {
    pub(super) fn is_timeline(&self) -> bool {
        matches!(self, Self::Timeline)
    }
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
                .child(
                    self.status
                        .clone()
                        .unwrap_or_else(|| "No preview available".into()),
                )
                .into_any_element(),
            PreviewTarget::Timeline => {
                let Some(timeline) = self.timeline.as_ref() else {
                    return div().w(px(width)).h(px(height)).into_any_element();
                };
                div()
                    .w(px(width))
                    .h(px(height))
                    .child(timeline_preview(&timeline.backend))
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
