use super::timeline_video::{create_timeline_video, set_timeline_audio};
use super::*;
use crate::playback_view::{PlaybackViewProps, playback_view};
use url::Url;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum PreviewTarget {
    Timeline,
    VideoFile(PathBuf),
    AudioFile(PathBuf),
    ImageFile(PathBuf),
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
        match &self.preview_target {
            PreviewTarget::Timeline => self.preview_timeline(origin_x, origin_y, width, height, cx),
            PreviewTarget::VideoFile(_) => {
                self.preview_video_file(origin_x, origin_y, width, height, cx)
            }
            PreviewTarget::AudioFile(path) => {
                self.preview_audio_file(path, origin_x, width, height, cx)
            }
            PreviewTarget::ImageFile(path) => self.preview_image_file(path, width, height),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn playable_preview(
        &self,
        origin_x: f32,
        origin_y: f32,
        width: f32,
        height: f32,
        has_media: bool,
        paused: bool,
        reported_position: Duration,
        duration: Duration,
        content: gpui::AnyElement,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let reported_progress = if duration.is_zero() {
            0.0
        } else {
            (reported_position.as_secs_f64() / duration.as_secs_f64()).clamp(0.0, 1.0) as f32
        };
        let progress = self.preview_scrub_fraction.unwrap_or(reported_progress);
        let position = self
            .preview_scrub_fraction
            .map_or(reported_position, |fraction| {
                duration.mul_f64(fraction as f64)
            });

        playback_view(
            PlaybackViewProps {
                origin_x,
                origin_y,
                width,
                height,
                has_media,
                can_play: has_media,
                paused,
                scrubbing: self.preview_is_scrubbing,
                progress,
                position,
                duration,
                volume: self.preview_volume,
                muted: self.preview_volume <= f64::EPSILON,
                volume_open: self.preview_volume_open,
                content,
                extra_control: None,
            },
            cx,
        )
    }

    pub(super) fn update_playback(&mut self) {
        if !self.playing {
            self.timeline_playback_clock = None;
            return;
        }
        let Some(video) = self.video.as_ref() else {
            self.playing = false;
            self.timeline_playback_clock = None;
            return;
        };
        let duration = self.project.timeline_duration();
        let (origin, started_at) = *self
            .timeline_playback_clock
            .get_or_insert((self.playhead, Instant::now()));
        self.playhead = timeline_playhead_from_elapsed(&self.project, origin, started_at.elapsed())
            .clamp(TimelineTime::ZERO, duration);
        if video.eos() || self.playhead >= duration {
            video.set_paused(true);
            self.playhead = duration;
            self.playing = false;
            self.timeline_playback_clock = None;
        }
    }

    pub(super) fn load_timeline_position(&mut self, position: TimelineTime, play: bool) {
        self.load_timeline_position_with_options(position, play, true);
    }

    pub(super) fn load_timeline_position_for_scrub(
        &mut self,
        position: TimelineTime,
        accurate: bool,
        _synchronize_audio: bool,
    ) {
        self.load_timeline_position_with_options(position, false, accurate);
    }

    fn load_timeline_position_with_options(
        &mut self,
        position: TimelineTime,
        play: bool,
        accurate: bool,
    ) {
        let was_timeline = self.preview_target == PreviewTarget::Timeline;
        self.standalone_audio = None;
        self.preview_target = PreviewTarget::Timeline;
        self.explorer.selected_file = None;
        self.explorer.context_menu = None;
        let duration = self.project.timeline_duration();
        let position = position.clamp(TimelineTime::ZERO, duration);
        self.playhead = position;
        self.playing = play;
        self.timeline_playback_clock = None;

        if self.project.clips.is_empty() {
            if let Some(video) = &self.video {
                video.set_paused(true);
            }
            self.video = None;
            self.timeline_preview_needs_rebuild = true;
            self.playing = false;
            self.preview_refresh_ticks = 2;
            return;
        }

        if !was_timeline {
            if let Some(video) = &self.video {
                video.set_paused(true);
            }
            self.video = None;
            self.timeline_preview_needs_rebuild = true;
        }

        if self.video.is_none() || self.timeline_preview_needs_rebuild {
            if let Some(video) = &self.video {
                video.set_paused(true);
            }
            self.video = None;
            match create_timeline_video(&self.project, &self.project_root) {
                Ok(video) => {
                    set_timeline_audio(
                        &video,
                        self.preview_volume,
                        self.preview_volume <= f64::EPSILON,
                    );
                    self.video = Some(video);
                    self.timeline_preview_needs_rebuild = false;
                }
                Err(error) => {
                    self.error = Some(error);
                    self.playing = false;
                    return;
                }
            }
        }

        if let Some(video) = &self.video {
            let _ = video.seek(self.project.duration(position), accurate);
            set_timeline_audio(
                video,
                self.preview_volume,
                self.preview_volume <= f64::EPSILON,
            );
            video.set_paused(!play);
            if play {
                self.timeline_playback_clock = Some((position, Instant::now()));
            }
        }
        self.preview_refresh_ticks = 12;
        self.selected_asset_id = self
            .project
            .visual_clip_at_time(position)
            .map(|clip| clip.asset_id);
        self.error = None;
    }

    pub(super) fn toggle_playback(&mut self) {
        match &self.preview_target {
            PreviewTarget::ImageFile(_) => return,
            PreviewTarget::VideoFile(_) => {
                let Some(video) = &self.video else {
                    return;
                };
                if video.eos() {
                    let _ = video.restart_stream();
                    video.set_paused(false);
                } else {
                    video.set_paused(!video.paused());
                }
                self.preview_refresh_ticks = 12;
                return;
            }
            PreviewTarget::AudioFile(_) => {
                let Some(audio) = &self.standalone_audio else {
                    return;
                };
                if audio.finished() {
                    audio.seek(Duration::ZERO);
                    audio.set_playing(true);
                } else {
                    audio.set_playing(!audio.playing());
                }
                self.preview_refresh_ticks = 12;
                return;
            }
            PreviewTarget::Timeline => {}
        }

        if self.project.clips.is_empty() {
            return;
        }
        if self.playing {
            self.update_playback();
            if let Some(video) = &self.video {
                video.set_paused(true);
            }
            self.playing = false;
            self.timeline_playback_clock = None;
            return;
        }
        let duration = self.project.timeline_duration();
        let start = if self.playhead >= duration {
            TimelineTime::ZERO
        } else {
            self.playhead
        };
        self.load_timeline_position(start, true);
    }

    pub(super) fn select_file(&mut self, relative_path: PathBuf, cx: &mut Context<Self>) {
        let is_image = workspace::is_image_path(&relative_path);
        let is_video = workspace::is_video_path(&relative_path);
        let is_audio = workspace::is_audio_path(&relative_path);

        self.explorer.selected_file = Some(relative_path.clone());
        self.selected_asset_id = self
            .project
            .assets
            .iter()
            .find(|asset| asset.path == relative_path)
            .map(|asset| asset.id);

        if is_image || is_video || is_audio {
            self.preview_target = match (is_video, is_audio) {
                (true, _) => PreviewTarget::VideoFile(relative_path.clone()),
                (_, true) => PreviewTarget::AudioFile(relative_path.clone()),
                _ => PreviewTarget::ImageFile(relative_path.clone()),
            };
            self.status = None;
            self.error = None;
            if let Some(video) = &self.video {
                video.set_paused(true);
            }
            self.video = None;
            self.standalone_audio = None;
            self.playing = false;
            self.timeline_playback_clock = None;
            self.preview_volume_open = false;
            self.preview_is_scrubbing = false;
            self.preview_is_adjusting_volume = false;
            self.preview_resume_after_scrub = false;
            self.preview_scrub_fraction = None;
            self.preview_pending_seek_started = None;
            self.preview_last_scrub_seek = None;
            self.preview_refresh_ticks = 2;
        }

        if !is_video && !is_audio {
            return;
        }

        let project_root = self.project_root.clone();
        let source_path = project_root.join(&relative_path);
        let Ok(url) = Url::from_file_path(&source_path) else {
            self.error = Some(format!("Could not open {}", source_path.display()));
            return;
        };
        self.status = Some(format!("Loading preview for {}…", relative_path.display()));
        self.error = None;

        if is_audio {
            cx.spawn(async move |editor, cx| {
                let result = cx
                    .background_executor()
                    .spawn(async move { AudioPreview::new(&url) })
                    .await;

                editor
                    .update(cx, |editor, cx| {
                        let still_requested = matches!(
                            &editor.preview_target,
                            PreviewTarget::AudioFile(path) if path == &relative_path
                        );
                        if editor.project_root != project_root || !still_requested {
                            return;
                        }

                        match result {
                            Ok(audio) => {
                                audio.set_volume(editor.preview_volume);
                                audio.set_playing(false);
                                editor.standalone_audio = Some(audio);
                                editor.status = Some("Audio preview ready.".to_string());
                                editor.error = None;
                                editor.preview_refresh_ticks = 12;
                            }
                            Err(error) => {
                                editor.status = None;
                                editor.error = Some(error);
                            }
                        }
                        cx.notify();
                    })
                    .ok();
            })
            .detach();
            return;
        }

        cx.spawn(async move |editor, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let video = Video::open(&url, false).map_err(|error| {
                        format!("Could not preview {}: {error}", source_path.display())
                    })?;
                    video.set_paused(true);
                    let _ = video.seek(Duration::ZERO, true);
                    Ok::<_, String>(video)
                })
                .await;

            editor
                .update(cx, |editor, cx| {
                    let still_requested = matches!(
                        &editor.preview_target,
                        PreviewTarget::VideoFile(path) if path == &relative_path
                    );
                    if editor.project_root != project_root || !still_requested {
                        return;
                    }

                    match result {
                        Ok(video) => {
                            video.set_volume(editor.preview_volume);
                            editor.video = Some(video);
                            editor.status = Some("Video preview ready.".to_string());
                            editor.error = None;
                            editor.preview_refresh_ticks = 12;
                        }
                        Err(error) => {
                            editor.status = None;
                            editor.error = Some(error);
                        }
                    }
                    cx.notify();
                })
                .ok();
        })
        .detach();
    }
}

fn timeline_playhead_from_elapsed(
    project: &Project,
    origin: TimelineTime,
    elapsed: Duration,
) -> TimelineTime {
    origin + project.floor_duration(elapsed)
}

impl Editor {
    fn seek_preview_to_fraction(&mut self, fraction: f32, accurate: bool, play: bool) {
        let fraction = fraction.clamp(0.0, 1.0);
        match self.preview_target.clone() {
            PreviewTarget::Timeline => {
                let duration = self.project.timeline_duration().frames();
                let position =
                    TimelineTime::from_frames((duration as f64 * fraction as f64).round() as i64);
                self.load_timeline_position_with_options(position, play, accurate);
            }
            PreviewTarget::VideoFile(_) => {
                if let Some(video) = &self.video {
                    let target = video.duration().mul_f64(fraction as f64);
                    let _ = video.seek(target, accurate);
                    video.set_paused(!play);
                }
            }
            PreviewTarget::AudioFile(_) | PreviewTarget::ImageFile(_) => {}
        }
    }

    pub(super) fn reconcile_preview_seek(&mut self) {
        if self.preview_is_scrubbing {
            return;
        }
        let (Some(fraction), Some(started)) = (
            self.preview_scrub_fraction,
            self.preview_pending_seek_started,
        ) else {
            return;
        };

        let settled = match self.preview_target {
            PreviewTarget::Timeline => true,
            PreviewTarget::VideoFile(_) => self.video.as_ref().is_some_and(|video| {
                let target = video.duration().mul_f64(fraction as f64);
                video.position().abs_diff(target) <= Duration::from_millis(40)
            }),
            PreviewTarget::AudioFile(_) => self.standalone_audio.as_ref().is_some_and(|audio| {
                let target = audio.duration().mul_f64(fraction as f64);
                audio.position().abs_diff(target) <= Duration::from_millis(40)
            }),
            PreviewTarget::ImageFile(_) => true,
        };
        if settled || started.elapsed() >= Duration::from_secs(2) {
            self.preview_scrub_fraction = None;
            self.preview_pending_seek_started = None;
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
            self.preview_target,
            PreviewTarget::AudioFile(_) | PreviewTarget::ImageFile(_)
        ) {
            return;
        }

        match phase {
            DragPhase::Start => {
                self.preview_resume_after_scrub = match self.preview_target {
                    PreviewTarget::Timeline => self.playing,
                    PreviewTarget::VideoFile(_) => {
                        self.video.as_ref().is_some_and(|video| !video.paused())
                    }
                    PreviewTarget::AudioFile(_) | PreviewTarget::ImageFile(_) => return,
                };
                if let Some(video) = &self.video {
                    video.set_paused(true);
                }
                self.playing = false;
                self.timeline_playback_clock = None;
                self.preview_is_scrubbing = true;
                self.preview_scrub_fraction = Some(fraction);
                self.preview_pending_seek_started = None;
                self.preview_last_scrub_seek = Some(Instant::now());
                self.seek_preview_to_fraction(fraction, false, false);
            }
            DragPhase::Update if self.preview_is_scrubbing => {
                self.preview_scrub_fraction = Some(fraction);
                let now = Instant::now();
                let should_seek = self
                    .preview_last_scrub_seek
                    .is_none_or(|last_seek| now.duration_since(last_seek) >= SCRUB_SEEK_INTERVAL);
                if should_seek {
                    self.preview_last_scrub_seek = Some(now);
                    self.seek_preview_to_fraction(fraction, false, false);
                }
            }
            DragPhase::End if self.preview_is_scrubbing => {
                let resume = self.preview_resume_after_scrub;
                self.preview_scrub_fraction = Some(fraction);
                self.preview_pending_seek_started = Some(Instant::now());
                self.preview_last_scrub_seek = None;
                self.preview_is_scrubbing = false;
                self.seek_preview_to_fraction(fraction, true, resume);
                self.preview_resume_after_scrub = false;
            }
            _ => return,
        }
        self.preview_refresh_ticks = 12;
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
            DragPhase::Start => self.preview_is_adjusting_volume = true,
            DragPhase::Update if !self.preview_is_adjusting_volume => return,
            DragPhase::End if !self.preview_is_adjusting_volume => return,
            DragPhase::End => self.preview_is_adjusting_volume = false,
            DragPhase::Update => {}
        }
        self.preview_volume = volume.clamp(0.0, 1.0);
        if let Some(video) = &self.video {
            if self.preview_target == PreviewTarget::Timeline {
                set_timeline_audio(
                    video,
                    self.preview_volume,
                    self.preview_volume <= f64::EPSILON,
                );
            } else {
                video.set_volume(self.preview_volume);
                video.set_muted(self.preview_volume <= f64::EPSILON);
            }
        }
        cx.notify();
    }

    fn playback_toggle_volume(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let has_playable_target = match self.preview_target {
            PreviewTarget::Timeline => !self.project.clips.is_empty(),
            PreviewTarget::VideoFile(_) => self.video.is_some(),
            PreviewTarget::AudioFile(_) | PreviewTarget::ImageFile(_) => false,
        };
        if has_playable_target {
            self.preview_volume_open = !self.preview_volume_open;
            cx.notify();
        }
    }

    fn playback_dismiss_volume(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if self.preview_volume_open {
            self.preview_volume_open = false;
            cx.notify();
        }
    }
}

#[cfg(test)]
#[path = "preview.test.rs"]
mod tests;
