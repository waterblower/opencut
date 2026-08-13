use super::*;
use crate::playback_view::{CONTROL_HEIGHT, PlaybackViewProps, playback_view};
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
            .child(if let Some(video_handle) = &self.preview.video {
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
            .preview
            .video
            .as_ref()
            .map_or((Duration::ZERO, Duration::ZERO, true), |video| {
                (video.position(), video.duration(), video.paused())
            });
        let reported_progress = if duration.is_zero() {
            0.0
        } else {
            (position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
        };
        let progress = self.preview.scrub_fraction.unwrap_or(reported_progress);
        let position = self
            .preview
            .scrub_fraction
            .map_or(position, |fraction| duration.mul_f64(fraction as f64));
        let has_media = self.preview.video.is_some();

        playback_view(
            PlaybackViewProps {
                origin_x,
                origin_y,
                width,
                height,
                has_media,
                can_play: has_media,
                paused,
                scrubbing: self.preview.is_scrubbing,
                progress,
                position,
                duration,
                volume: self.preview.volume,
                muted: self.preview.volume <= f64::EPSILON,
                volume_open: self.preview.volume_open,
                content,
                extra_control: None,
                fullscreen_control: div()
                    .id("editor-video-fullscreen")
                    .cursor(CursorStyle::PointingHand)
                    .rounded_md()
                    .hover(|style| style.bg(rgb(SURFACE_HOVER)))
                    .px_3()
                    .py_2()
                    .text_lg()
                    .child("⛶")
                    .on_click(cx.listener(Self::playback_toggle_fullscreen))
                    .into_any_element(),
            },
            cx,
        )
    }
}
