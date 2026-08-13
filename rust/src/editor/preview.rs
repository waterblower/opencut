use super::timeline_video::{create_timeline_video, set_timeline_audio};
use super::*;
use preview_image::preview_image_file;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum PreviewTarget {
    Timeline,
    VideoFile(PathBuf),
    AudioFile(PathBuf),
    ImageFile(PathBuf),
}

impl Editor {
    pub(super) fn rebuild_timeline_preview_if_needed(&mut self) {
        if !self.preview.timeline_needs_rebuild || self.preview.target != PreviewTarget::Timeline {
            return;
        }
        let Some(timeline) = self.timeline.as_ref() else {
            return;
        };
        let playhead = timeline.playhead;
        self.load_timeline_position_with_options(playhead, self.preview.playing, true);
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
            PreviewTarget::Timeline => self.preview_timeline(origin_x, origin_y, width, height, cx),
            PreviewTarget::VideoFile(_) => {
                self.preview_video_file(origin_x, origin_y, width, height, cx)
            }
            PreviewTarget::AudioFile(path) => {
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
        play: bool,
        accurate: bool,
    ) {
        let Some(timeline) = self.timeline.as_mut() else {
            self.preview.playing = false;
            self.preview.timeline_clock = None;
            return;
        };
        let was_timeline = self.preview.target == PreviewTarget::Timeline;
        self.preview.audio = None;
        self.preview.target = PreviewTarget::Timeline;
        self.explorer.selected_file = None;
        self.explorer.context_menu = None;
        let duration = timeline.data.timeline_duration();
        let position = position.clamp(TimelineTime::ZERO, duration);
        timeline.playhead = position;
        self.preview.playing = play;
        self.preview.timeline_clock = None;
        self.preview.timeline_drag = None;

        if timeline.data.clips.is_empty() {
            if let Some(video) = &self.preview.video {
                video.set_paused(true);
            }
            self.preview.video = None;
            self.preview.timeline_needs_rebuild = true;
            self.preview.playing = false;
            self.preview.refresh_ticks = 2;
            return;
        }

        if !was_timeline {
            if let Some(video) = &self.preview.video {
                video.set_paused(true);
            }
            self.preview.video = None;
            self.preview.timeline_needs_rebuild = true;
        }

        if self.preview.video.is_none() || self.preview.timeline_needs_rebuild {
            if let Some(video) = &self.preview.video {
                video.set_paused(true);
            }
            self.preview.video = None;
            match create_timeline_video(&timeline.data, &self.global_settings.project_root) {
                Ok(video) => {
                    set_timeline_audio(
                        &video,
                        self.preview.volume,
                        self.preview.volume <= f64::EPSILON,
                    );
                    self.preview.video = Some(video);
                    self.preview.timeline_needs_rebuild = false;
                }
                Err(error) => {
                    eprintln!("{error}");
                    self.preview.playing = false;
                    return;
                }
            }
        }

        if let Some(video) = &self.preview.video {
            let _ = video.seek(timeline.data.duration(position), accurate);
            set_timeline_audio(
                video,
                self.preview.volume,
                self.preview.volume <= f64::EPSILON,
            );
            video.set_paused(!play);
            if play {
                self.preview.timeline_clock = Some((position, Instant::now()));
            }
        }
        self.preview.refresh_ticks = 12;
    }

    pub(super) fn toggle_playback(&mut self) {
        match &self.preview.target {
            PreviewTarget::ImageFile(_) => return,
            PreviewTarget::VideoFile(_) => {
                let Some(video) = &self.preview.video else {
                    return;
                };
                if video.eos() {
                    let _ = video.restart_stream();
                    video.set_paused(false);
                } else {
                    video.set_paused(!video.paused());
                }
                self.preview.refresh_ticks = 12;
                return;
            }
            PreviewTarget::AudioFile(_) => {
                let Some(audio) = &self.preview.audio else {
                    return;
                };
                if audio.finished() {
                    audio.seek(Duration::ZERO);
                    audio.set_playing(true);
                } else {
                    audio.set_playing(!audio.playing());
                }
                self.preview.refresh_ticks = 12;
                return;
            }
            PreviewTarget::Timeline => {}
        }

        let Some(timeline) = self.timeline.as_mut() else {
            return;
        };
        if timeline.data.clips.is_empty() {
            return;
        }
        if self.preview.playing {
            update_playback(timeline, &mut self.preview);
            if let Some(video) = &self.preview.video {
                video.set_paused(true);
            }
            self.preview.playing = false;
            self.preview.timeline_clock = None;
            return;
        }
        let duration = timeline.data.timeline_duration();
        let start = if timeline.playhead >= duration {
            TimelineTime::ZERO
        } else {
            timeline.playhead
        };
        self.load_timeline_position_with_options(start, true, true);
    }
}

pub(super) fn update_playback(timeline: &mut TimelineState, preview: &mut PreviewState) {
    if !preview.playing {
        preview.timeline_clock = None;
        return;
    }
    let Some(video) = preview.video.as_ref() else {
        preview.playing = false;
        preview.timeline_clock = None;
        return;
    };
    let duration = timeline.data.timeline_duration();
    let (origin, started_at) = *preview
        .timeline_clock
        .get_or_insert((timeline.playhead, Instant::now()));
    timeline.playhead = timeline_playhead_from_elapsed(
        timeline.data.settings.frame_rate,
        origin,
        started_at.elapsed(),
    )
    .clamp(TimelineTime::ZERO, duration);
    if video.eos() || timeline.playhead >= duration {
        video.set_paused(true);
        timeline.playhead = duration;
        preview.playing = false;
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
    fn seek_preview_to_fraction(&mut self, fraction: f32, accurate: bool, play: bool) {
        let fraction = fraction.clamp(0.0, 1.0);
        match self.preview.target.clone() {
            PreviewTarget::Timeline => {
                let Some(timeline) = self.timeline.as_ref() else {
                    return;
                };
                let duration = timeline.data.timeline_duration().frames();
                let position =
                    TimelineTime::from_frames((duration as f64 * fraction as f64).round() as i64);
                self.load_timeline_position_with_options(position, play, accurate);
            }
            PreviewTarget::VideoFile(_) => {
                if let Some(video) = &self.preview.video {
                    let target = video.duration().mul_f64(fraction as f64);
                    let _ = video.seek(target, accurate);
                    video.set_paused(!play);
                }
            }
            PreviewTarget::AudioFile(_) | PreviewTarget::ImageFile(_) => {}
        }
    }
}

pub(super) fn reconcile_preview_seek(preview: &mut PreviewState) {
    if preview.is_scrubbing {
        return;
    }
    let (Some(fraction), Some(started)) = (preview.scrub_fraction, preview.pending_seek_started)
    else {
        return;
    };

    let settled = match preview.target {
        PreviewTarget::Timeline => true,
        PreviewTarget::VideoFile(_) => preview.video.as_ref().is_some_and(|video| {
            let target = video.duration().mul_f64(fraction as f64);
            video.position().abs_diff(target) <= Duration::from_millis(40)
        }),
        PreviewTarget::AudioFile(_) => preview.audio.as_ref().is_some_and(|audio| {
            let target = audio.duration().mul_f64(fraction as f64);
            audio.position().abs_diff(target) <= Duration::from_millis(40)
        }),
        PreviewTarget::ImageFile(_) => true,
    };
    if settled || started.elapsed() >= Duration::from_secs(2) {
        preview.scrub_fraction = None;
        preview.pending_seek_started = None;
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
            self.preview.target,
            PreviewTarget::AudioFile(_) | PreviewTarget::ImageFile(_)
        ) {
            return;
        }

        match phase {
            DragPhase::Start => {
                self.preview.resume_after_scrub = match self.preview.target {
                    PreviewTarget::Timeline => self.preview.playing,
                    PreviewTarget::VideoFile(_) => self
                        .preview
                        .video
                        .as_ref()
                        .is_some_and(|video| !video.paused()),
                    PreviewTarget::AudioFile(_) | PreviewTarget::ImageFile(_) => return,
                };
                if let Some(video) = &self.preview.video {
                    video.set_paused(true);
                }
                self.preview.playing = false;
                self.preview.timeline_clock = None;
                self.preview.is_scrubbing = true;
                self.preview.scrub_fraction = Some(fraction);
                self.preview.pending_seek_started = None;
                self.preview.last_scrub_seek = Some(Instant::now());
                self.seek_preview_to_fraction(fraction, false, false);
            }
            DragPhase::Update if self.preview.is_scrubbing => {
                self.preview.scrub_fraction = Some(fraction);
                let now = Instant::now();
                let should_seek = self
                    .preview
                    .last_scrub_seek
                    .is_none_or(|last_seek| now.duration_since(last_seek) >= SCRUB_SEEK_INTERVAL);
                if should_seek {
                    self.preview.last_scrub_seek = Some(now);
                    self.seek_preview_to_fraction(fraction, false, false);
                }
            }
            DragPhase::End if self.preview.is_scrubbing => {
                let resume = self.preview.resume_after_scrub;
                self.preview.scrub_fraction = Some(fraction);
                self.preview.pending_seek_started = Some(Instant::now());
                self.preview.last_scrub_seek = None;
                self.preview.is_scrubbing = false;
                self.seek_preview_to_fraction(fraction, true, resume);
                self.preview.resume_after_scrub = false;
                if self.preview.target == PreviewTarget::Timeline {
                    self.save_timeline_playhead();
                }
            }
            _ => return,
        }
        self.preview.refresh_ticks = 12;
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
        self.preview.volume = volume.clamp(0.0, 1.0);
        if let Some(video) = &self.preview.video {
            if self.preview.target == PreviewTarget::Timeline {
                set_timeline_audio(
                    video,
                    self.preview.volume,
                    self.preview.volume <= f64::EPSILON,
                );
            } else {
                video.set_volume(self.preview.volume);
                video.set_muted(self.preview.volume <= f64::EPSILON);
            }
        }
        cx.notify();
    }

    fn playback_toggle_volume(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let has_playable_target = match self.preview.target {
            PreviewTarget::Timeline => self
                .timeline
                .as_ref()
                .is_some_and(|timeline| !timeline.data.clips.is_empty()),
            PreviewTarget::VideoFile(_) => self.preview.video.is_some(),
            PreviewTarget::AudioFile(_) | PreviewTarget::ImageFile(_) => false,
        };
        if has_playable_target {
            self.preview.volume_open = !self.preview.volume_open;
            cx.notify();
        }
    }

    fn playback_dismiss_volume(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.preview.volume_open {
            self.preview.volume_open = false;
            cx.notify();
        }
    }
}

#[cfg(test)]
#[path = "preview.test.rs"]
mod tests;
