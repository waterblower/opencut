use preview_timeline::preview_timeline_view;

use super::*;
use opencut_player::video2::{AudioBackend, VideoBackend};
use preview_image::preview_image_file;

pub enum PreviewTarget {
    None,
    Timeline,
    VideoFile(PathBuf, VideoBackend),
    AudioFile(PathBuf, AudioBackend),
    ImageFile(PathBuf),
}

pub fn load_timeline_position_with_options(
    preview: &mut PreviewState,
    timeline: &mut TimelineRuntimeState,
    position: TimelineTime,
) {
    preview.target = PreviewTarget::Timeline;
    let duration = timeline.data.content_duration();
    let position = position.clamp(TimelineTime::ZERO, duration);
    preview.timeline_drag = None;

    let _ = timeline
        .video_backend
        .playback_mut()
        .seek(timeline.data.duration(position));
}

impl PreviewTarget {
    pub(super) fn is_timeline(&self) -> bool {
        matches!(self, Self::Timeline)
    }

    pub(super) fn audio(&self) -> Option<&AudioBackend> {
        let Self::AudioFile(_, audio) = self else {
            return None;
        };
        Some(audio)
    }
}

impl Editor {
    pub(super) fn preview_player(
        &self,
        origin_x: f32,
        origin_y: f32,
        width: f32,
        height: f32,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
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
                preview_timeline_view(self, origin_x, origin_y, width, height, cx)
            }
            PreviewTarget::VideoFile(_, _) => {
                self.preview_video_file(origin_x, origin_y, width, height, cx)
            }
            PreviewTarget::AudioFile(path, _) => {
                self.preview_audio_file(path, origin_x, width, height, cx)
            }
            PreviewTarget::ImageFile(path) => {
                preview_image_file(self.project_root.join(path), width, height)
            }
        }
    }

    pub(super) fn playback_toggle_fullscreen(
        &mut self,
        _: &gpui::ClickEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.preview.fullscreen = !self.preview.fullscreen;
        cx.notify();
    }
}

impl PlaybackViewDelegate for Editor {
    fn playback_toggle(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.emit_event(cx, AppEvent::Preview(PreviewEvent::TogglePlayback));
        cx.notify();
    }

    fn playback_seek(
        &mut self,
        fraction: f32,
        phase: DragPhase,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.emit_event(
            cx,
            AppEvent::Preview(PreviewEvent::Scrub { fraction, phase }),
        );
    }

    fn playback_set_volume(
        &mut self,
        volume: f64,
        phase: DragPhase,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.emit_event(
            cx,
            AppEvent::Preview(PreviewEvent::SetVolume { volume, phase }),
        );
    }

    fn playback_toggle_volume(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let has_playable_target = match &self.preview.target {
            PreviewTarget::Timeline => self
                .timeline
                .as_ref()
                .is_some_and(|timeline| !timeline.data.clips.is_empty()),
            PreviewTarget::VideoFile(_, _) => true,
            PreviewTarget::None | PreviewTarget::AudioFile(_, _) | PreviewTarget::ImageFile(_) => {
                false
            }
        };
        if has_playable_target {
            self.preview.volume_control_open = !self.preview.volume_control_open;
            cx.notify();
        }
    }

    fn playback_dismiss_volume(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.preview.volume_control_open {
            self.preview.volume_control_open = false;
            cx.notify();
        }
    }
}

impl Editor {
    pub fn preview_file_video(&self) -> Option<&VideoBackend> {
        match &self.preview.target {
            PreviewTarget::Timeline => None,
            PreviewTarget::VideoFile(_, video) => Some(video),
            PreviewTarget::None | PreviewTarget::AudioFile(_, _) | PreviewTarget::ImageFile(_) => {
                None
            }
        }
    }
}
