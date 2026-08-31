use preview_timeline::preview_timeline_view;

use super::*;
use preview_image::preview_image_file;

pub(super) enum PreviewTarget {
    None,
    Timeline,
    VideoFile(PathBuf, FileVideoBackend),
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
    timeline.playhead = position;
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
                .child("No preview available")
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
                preview_image_file(self.global_settings.project_root.join(path), width, height)
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

    pub(super) fn toggle_playback(&mut self) {
        match &self.preview.target {
            PreviewTarget::None | PreviewTarget::ImageFile(_) => return,
            PreviewTarget::VideoFile(_, video) => {
                video.set_paused(!video.paused());
                return;
            }
            PreviewTarget::AudioFile(_, audio) => {
                if audio.finished() {
                    audio.seek_with_accuracy(Duration::ZERO, true);
                    audio.set_playing(true);
                } else {
                    audio.set_playing(!audio.playing());
                }

                return;
            }
            PreviewTarget::Timeline => {
                let Some(timeline) = self.timeline.as_mut() else {
                    return;
                };
                let video = timeline.video_backend.playback();
                let is_paused = video.paused();
                if timeline.data.clips.is_empty() {
                    return;
                }

                let duration = timeline.data.content_duration();
                let start = if timeline.playhead >= duration {
                    TimelineTime::ZERO
                } else {
                    timeline.playhead
                };
                video.set_paused(!is_paused);
                load_timeline_position_with_options(&mut self.preview, timeline, start);
            }
        }
    }
}

pub(super) fn update_playback(timeline: &mut TimelineRuntimeState, preview: &mut PreviewState) {
    if !preview.target.is_timeline() {
        return;
    }
    let video = timeline.video_backend.playback();
    let duration = timeline.data.content_duration();
    timeline.playhead = timeline
        .data
        .settings
        .frame_rate
        .frames_from_duration_nearest(video.position());

    if timeline.playhead >= duration {
        video.set_paused(true);
        timeline.playhead = duration;
    }
}

impl Editor {
    fn seek_preview_to_fraction(&mut self, fraction: f32) {
        let fraction = fraction.clamp(0.0, 1.0);
        if self.preview.target.is_timeline() {
            let Some(timeline) = self.timeline.as_mut() else {
                return;
            };
            let duration = timeline.data.content_duration().frames();
            let position =
                TimelineTime::from_frames((duration as f64 * fraction as f64).round() as i64);
            load_timeline_position_with_options(&mut self.preview, timeline, position);
            return;
        }
        if let PreviewTarget::VideoFile(_, video) = &mut self.preview.target {
            let target = video.duration().mul_f64(fraction as f64);
            let _ = video.seek(target);
        }
    }
}

impl PlaybackViewDelegate for Editor {
    fn playback_toggle(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.toggle_playback();
        cx.notify();
    }

    fn playback_seek(
        &mut self,
        fraction: f32,
        phase: DragPhase,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(
            &self.preview.target,
            PreviewTarget::None | PreviewTarget::AudioFile(_, _) | PreviewTarget::ImageFile(_)
        ) {
            return;
        }

        match phase {
            DragPhase::Start => {
                if let Some(video) = self.active_video() {
                    video.set_paused(true);
                }
                self.preview.is_scrubbing = true;
                self.preview.last_scrub_seek = Some(Instant::now());
                self.seek_preview_to_fraction(fraction);
            }
            DragPhase::Update if self.preview.is_scrubbing => {
                let now = Instant::now();
                let should_seek = self
                    .preview
                    .last_scrub_seek
                    .is_none_or(|last_seek| now.duration_since(last_seek) >= SCRUB_SEEK_INTERVAL);
                if should_seek {
                    self.preview.last_scrub_seek = Some(now);
                    self.seek_preview_to_fraction(fraction);
                }
            }
            DragPhase::End if self.preview.is_scrubbing => {
                self.preview.last_scrub_seek = None;
                self.preview.is_scrubbing = false;
                self.seek_preview_to_fraction(fraction);
            }
            _ => return,
        }

        cx.notify();
    }

    fn playback_set_volume(
        &mut self,
        volume: f64,
        phase: DragPhase,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match phase {
            DragPhase::Start => self.preview.is_adjusting_volume = true,
            DragPhase::Update if !self.preview.is_adjusting_volume => return,
            DragPhase::End if !self.preview.is_adjusting_volume => return,
            DragPhase::End => self.preview.is_adjusting_volume = false,
            DragPhase::Update => {}
        }
        if let Some(video) = self.active_video() {
            video.set_volume(volume.clamp(0.0, 1.0));
            video.set_muted(volume.clamp(0.0, 1.0) <= f64::EPSILON);
        }
        cx.notify();
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
    pub fn active_video(&self) -> Option<&VideoBackend> {
        match &self.preview.target {
            PreviewTarget::Timeline => Some(self.timeline.as_ref()?.video_backend.playback()),
            PreviewTarget::VideoFile(_, video) => Some(video),
            PreviewTarget::None | PreviewTarget::AudioFile(_, _) | PreviewTarget::ImageFile(_) => {
                None
            }
        }
    }
}
