use super::*;
use crate::playback_view::CONTROL_HEIGHT;
use crate::video::video;

impl Editor {
    pub(super) fn preview_video_file(
        &self,
        origin_x: f32,
        origin_y: f32,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let surface_height = (height - CONTROL_HEIGHT).max(1.0);
        let content = div()
            .id("editor-video-file-preview-content")
            .relative()
            .w(px(width))
            .h(px(surface_height))
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .bg(rgb(0x000000))
            .child(if let Some(video_handle) = &self.video {
                video(video_handle.clone())
                    .id("editor-video-file-preview")
                    .size(px(width), px(surface_height))
                    .into_any_element()
            } else {
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(MUTED))
                    .child("Loading video preview…")
                    .into_any_element()
            })
            .into_any_element();
        let (position, duration, paused) = self
            .video
            .as_ref()
            .map_or((Duration::ZERO, Duration::ZERO, true), |video| {
                (video.position(), video.duration(), video.paused())
            });
        self.playable_preview(
            origin_x,
            origin_y,
            width,
            height,
            self.video.is_some(),
            paused,
            position,
            duration,
            content,
            cx,
        )
    }
}
