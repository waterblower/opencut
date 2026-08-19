use super::timeline_video::create_timeline_video;
use super::*;
use preview_image::preview_image_file;

pub(super) enum PreviewTarget {
    None,
    Timeline(VideoBackend),
    VideoFile(PathBuf, VideoBackend),
    AudioFile(PathBuf, AudioBackend),
    ImageFile(PathBuf),
}

impl PreviewTarget {
    pub(super) fn is_timeline(&self) -> bool {
        matches!(self, Self::Timeline(_))
    }

    pub(super) fn video(&self) -> Option<&VideoBackend> {
        match self {
            Self::Timeline(video) | Self::VideoFile(_, video) => Some(video),
            _ => None,
        }
    }

    pub(super) fn video_mut(&mut self) -> Option<&mut VideoBackend> {
        match self {
            Self::Timeline(video) | Self::VideoFile(_, video) => Some(video),
            _ => None,
        }
    }

    pub(super) fn audio(&self) -> Option<&AudioBackend> {
        let Self::AudioFile(_, audio) = self else {
            return None;
        };
        Some(audio)
    }
}

impl Editor {
    pub(super) fn rebuild_timeline_preview_if_needed(&mut self) {
        if !self.preview.timeline_needs_rebuild || !self.preview.target.is_timeline() {
            return;
        }
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let playhead = timeline.playhead;
        self.load_timeline_position_with_options(playhead, true);
    }

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
            PreviewTarget::Timeline(_) => {
                self.preview_timeline(origin_x, origin_y, width, height, cx)
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

    pub(super) fn load_timeline_position_with_options(
        &mut self,
        position: TimelineTime,
        accurate: bool,
    ) {
        let Some(timeline) = self.timeline.as_mut() else {
            self.preview.timeline_clock = None;
            return;
        };
        let was_timeline = self.preview.target.is_timeline();
        if !was_timeline {
            self.preview.target = PreviewTarget::None;
            self.preview.timeline_needs_rebuild = true;
        }
        self.explorer.selected_file = None;
        self.explorer.context_menu = None;
        let duration = timeline.data.content_duration();
        let position = position.clamp(TimelineTime::ZERO, duration);
        timeline.playhead = position;

        self.preview.timeline_clock = None;
        self.preview.timeline_drag = None;

        if timeline.data.clips.is_empty() {
            if let Some(video) = self.preview.target.video() {
                video.set_paused(true);
            }
            self.preview.target = PreviewTarget::None;
            self.preview.timeline_needs_rebuild = true;

            return;
        }

        if self.preview.target.video().is_none() || self.preview.timeline_needs_rebuild {
            let volume = match &self.preview.target {
                PreviewTarget::Timeline(video) => video.volume(),
                _ => 1.0,
            };
            if let Some(video) = self.preview.target.video() {
                video.set_paused(true);
            }
            self.preview.target = PreviewTarget::None;
            match create_timeline_video(&timeline.data, &self.global_settings.project_root) {
                Ok(video) => {
                    video.set_volume(volume);
                    video.set_muted(volume <= f64::EPSILON);
                    self.preview.target = PreviewTarget::Timeline(video);
                    self.preview.timeline_needs_rebuild = false;
                }
                Err(error) => {
                    eprintln!("{error}");

                    return;
                }
            }
        }

        if let Some(video) = self.preview.target.video_mut() {
            let _ = video.seek(timeline.data.duration(position), accurate);
        }
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
            PreviewTarget::Timeline(video) => {
                let is_paused = video.paused();
                let Some(timeline) = self.timeline.as_mut() else {
                    return;
                };
                if timeline.data.clips.is_empty() {
                    return;
                }

                let duration = timeline.data.content_duration();
                let start = if timeline.playhead >= duration {
                    TimelineTime::ZERO
                } else {
                    timeline.playhead
                };
                let PreviewTarget::Timeline(video) = &self.preview.target else {
                    return;
                };
                video.set_paused(!is_paused);
                self.load_timeline_position_with_options(start, true);

                if is_paused {
                    self.preview.timeline_clock = Some((start, Instant::now()));
                }
            }
        }
    }
}

pub(super) fn update_playback(timeline: &mut TimelineRuntimeState, preview: &mut PreviewState) {
    let PreviewTarget::Timeline(video) = &preview.target else {
        preview.timeline_clock = None;
        return;
    };
    if video.paused() {
        preview.timeline_clock = None;
        return;
    }
    let duration = timeline.data.content_duration();
    let (origin, started_at) = *preview
        .timeline_clock
        .get_or_insert((timeline.playhead, Instant::now()));
    timeline.playhead = timeline_playhead_from_elapsed(
        timeline.data.settings.frame_rate,
        origin,
        started_at.elapsed(),
    )
    .clamp(TimelineTime::ZERO, duration);
    if timeline.playhead >= duration {
        video.set_paused(true);
        timeline.playhead = duration;

        preview.timeline_clock = None;
    }
}

fn timeline_playhead_from_elapsed(
    frame_rate: FrameRate,
    origin: TimelineTime,
    elapsed: Duration,
) -> TimelineTime {
    origin + frame_rate.floor_duration(elapsed)
}

impl Editor {
    fn seek_preview_to_fraction(&mut self, fraction: f32) {
        let fraction = fraction.clamp(0.0, 1.0);
        if self.preview.target.is_timeline() {
            let Some(timeline) = self.timeline.as_ref() else {
                return;
            };
            let duration = timeline.data.content_duration().frames();
            let position =
                TimelineTime::from_frames((duration as f64 * fraction as f64).round() as i64);
            self.load_timeline_position_with_options(position, true);
            return;
        }
        if let PreviewTarget::VideoFile(_, video) = &mut self.preview.target {
            let target = video.duration().mul_f64(fraction as f64);
            let _ = video.seek(target, true);
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
                if let Some(video) = self.preview.target.video() {
                    video.set_paused(true);
                }

                self.preview.timeline_clock = None;
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
                if self.preview.target.is_timeline() {
                    self.save_timeline_playhead();
                }
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
        match &self.preview.target {
            PreviewTarget::Timeline(video) => {
                video.set_volume(volume.clamp(0.0, 1.0));
                video.set_muted(volume.clamp(0.0, 1.0) <= f64::EPSILON);
            }
            PreviewTarget::VideoFile(_, video) => {
                video.set_volume(volume.clamp(0.0, 1.0));
                video.set_muted(volume.clamp(0.0, 1.0) <= f64::EPSILON);
            }
            _ => {}
        }
        cx.notify();
    }

    fn playback_toggle_volume(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let has_playable_target = match &self.preview.target {
            PreviewTarget::Timeline(_) => self
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

#[cfg(test)]
#[path = "preview.test.rs"]
mod tests;
