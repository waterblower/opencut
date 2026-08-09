use super::*;
use crate::playback_view::CONTROL_HEIGHT;
use crate::video::video;

impl Editor {
    pub(super) fn preview_timeline(
        &self,
        origin_x: f32,
        origin_y: f32,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let surface_height = (height - CONTROL_HEIGHT).max(1.0);
        let media = if let Some(video_handle) = self.preview.video.as_ref() {
            video(video_handle.clone())
                .id("editor-timeline-video")
                .size(px(width), px(surface_height))
                .into_any_element()
        } else {
            div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_color(rgb(MUTED))
                .child("Choose a video from the project folder to begin")
                .into_any_element()
        };
        let content = div()
            .id("editor-timeline-preview-content")
            .relative()
            .w(px(width))
            .h(px(surface_height))
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .bg(rgb(0x000000))
            .child(media)
            .into_any_element();
        let reported_position = self.project.duration(self.timeline.playhead);
        let duration = self.project.duration(self.project.timeline_duration());
        self.playable_preview(
            origin_x,
            origin_y,
            width,
            height,
            !self.project.clips.is_empty(),
            !self.preview.playing,
            reported_position,
            duration,
            content,
            cx,
        )
    }
}
