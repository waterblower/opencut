use super::*;
use crate::playback_view::{CONTROL_HEIGHT, PlaybackViewProps, playback_view};
use crate::video_backend::{VideoOptions, create_timeline_video, video};
use url::Url;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum PreviewTarget {
    Timeline,
    VideoFile(PathBuf),
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
        let selected_image_path = match &self.preview_target {
            PreviewTarget::ImageFile(path) => Some(self.project_root.join(path)),
            PreviewTarget::Timeline => self
                .project
                .visual_clip_at_time(self.playhead)
                .and_then(|clip| clip.asset_id)
                .and_then(|id| self.project.asset(id))
                .filter(|asset| asset.kind == MediaKind::Image)
                .map(|asset| self.project_root.join(&asset.path)),
            _ => None,
        };
        let preview_uses_timeline = self.preview_target == PreviewTarget::Timeline;
        let preview_can_toggle_playback = match self.preview_target {
            PreviewTarget::Timeline => !self.project.clips.is_empty(),
            PreviewTarget::VideoFile(_) => self.video.is_some(),
            PreviewTarget::ImageFile(_) => false,
        };
        let surface_height = (height - CONTROL_HEIGHT).max(1.0);
        let timeline_visual_overlays = if preview_uses_timeline {
            let base_track_index = self
                .project
                .visual_clip_at_time(self.playhead)
                .and_then(|clip| {
                    self.project
                        .tracks
                        .iter()
                        .position(|track| track.id == clip.track_id)
                })
                .unwrap_or(self.project.tracks.len());
            self.project.tracks[..base_track_index]
                .iter()
                .rev()
                .filter(|track| track.visible)
                .flat_map(|track| {
                    self.project
                        .clips_on_track(track.id)
                        .filter(|clip| clip.contains(self.playhead))
                        .filter_map(|clip| {
                            let asset = clip.asset_id.and_then(|id| self.project.asset(id))?;
                            (asset.kind == MediaKind::Image).then(|| {
                                img(self.project_root.join(&asset.path))
                                    .absolute()
                                    .inset_0()
                                    .size_full()
                                    .object_fit(ObjectFit::Contain)
                                    .into_any_element()
                            })
                        })
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let preview = if let Some(image_path) = selected_image_path {
            img(image_path)
                .id("editor-selected-image-preview")
                .w(px(width))
                .h(px(surface_height))
                .object_fit(ObjectFit::Contain)
                .into_any_element()
        } else if let Some(video_handle) = &self.video {
            video(video_handle.clone())
                .id("editor-preview-video")
                .size(px(width), px(surface_height))
                .buffer_capacity(3)
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
            .id("editor-preview-content")
            .relative()
            .w(px(width))
            .h(px(surface_height))
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden()
            .bg(rgb(0x000000))
            .child(preview)
            .children(timeline_visual_overlays)
            .into_any_element();

        let (reported_position, duration, paused) = match self.preview_target {
            PreviewTarget::Timeline => (
                self.project.duration(self.playhead),
                self.project.duration(self.project.timeline_duration()),
                !self.playing,
            ),
            PreviewTarget::VideoFile(_) => self
                .video
                .as_ref()
                .map_or((Duration::ZERO, Duration::ZERO, true), |video| {
                    (video.position(), video.duration(), video.paused())
                }),
            PreviewTarget::ImageFile(_) => (Duration::ZERO, Duration::ZERO, true),
        };
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
                has_media: preview_can_toggle_playback,
                can_play: preview_can_toggle_playback,
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
            return;
        }
        let Some(started) = self.still_playback_started else {
            self.playing = false;
            self.pause_audio_previews();
            return;
        };
        self.playhead = self.still_playback_origin + self.project.floor_duration(started.elapsed());
        let duration = self.project.timeline_duration();
        if self.playhead >= duration {
            if let Some(video) = &self.video {
                video.set_paused(true);
            }
            self.pause_audio_previews();
            self.playhead = duration;
            self.playing = false;
            self.still_playback_started = None;
            return;
        }

        let desired_clip_id = self
            .project
            .visual_clip_at_time(self.playhead)
            .map(|clip| clip.id);
        if desired_clip_id != self.loaded_clip_id {
            self.load_timeline_position(self.playhead, true);
            return;
        }

        if let (Some(clip_id), Some(video)) = (self.loaded_clip_id, self.video.as_ref())
            && let Some(clip) = self.project.clip(clip_id)
        {
            let expected = self.project.source_position_at(clip, self.playhead);
            if video.position().abs_diff(expected) > Duration::from_millis(250) {
                let _ = video.seek(expected, false);
            }
        }
        self.sync_audio_previews(self.playhead, true);
    }

    pub(super) fn load_timeline_position(&mut self, position: TimelineTime, play: bool) {
        self.preview_target = PreviewTarget::Timeline;
        self.selected_file = None;
        self.file_context_menu = None;
        let duration = self.project.timeline_duration();
        let position = position.clamp(TimelineTime::ZERO, duration);
        let clip = self.project.visual_clip_at_time(position).cloned();
        self.still_playback_origin = position;
        self.still_playback_started = play.then(Instant::now);
        self.playhead = position;
        self.playing = play;

        let Some(clip) = clip else {
            if let Some(video) = &self.video {
                video.set_paused(true);
            }
            self.video = None;
            self.loaded_clip_id = None;
            self.sync_audio_previews(position, play);
            self.preview_refresh_ticks = 2;
            return;
        };

        let Some(asset_id) = clip.asset_id else {
            return;
        };
        let Some(asset) = self.project.asset(asset_id).cloned() else {
            self.error = Some("The selected clip's source file is missing.".to_string());
            return;
        };
        let source_position = self.project.source_position_at(&clip, position);
        let loaded_clip = self
            .loaded_clip_id
            .and_then(|clip_id| self.project.clip(clip_id));
        let reuses_loaded_source = self.video.is_some()
            && loaded_clip.is_some_and(|loaded| loaded.asset_id == clip.asset_id);
        let seamless_transition =
            play && loaded_clip.is_some_and(|loaded| loaded.is_continuous_with(&clip));

        if asset.kind == MediaKind::Image {
            if let Some(video) = &self.video {
                video.set_paused(true);
            }
            self.video = None;
            self.loaded_clip_id = Some(clip.id);
        } else if !reuses_loaded_source {
            let source_path = self.project_root.join(&asset.path);
            let Ok(url) = Url::from_file_path(&source_path) else {
                self.error = Some(format!("Could not open {}", source_path.display()));
                return;
            };
            let frame_rate = self.project.settings.frame_rate;
            match create_timeline_video(
                &url,
                VideoOptions {
                    frame_buffer_capacity: Some(3),
                    looping: Some(false),
                    speed: Some(1.0),
                },
                frame_rate.numerator,
                frame_rate.denominator,
            ) {
                Ok(video) => {
                    video.set_volume(self.preview_volume);
                    self.video = Some(video);
                    self.loaded_clip_id = Some(clip.id);
                }
                Err(error) => {
                    self.error = Some(format!("Could not preview {}: {error}", asset.name));
                    return;
                }
            }
        }
        self.loaded_clip_id = Some(clip.id);

        if asset.kind == MediaKind::Video
            && let Some(video) = &self.video
        {
            if !seamless_transition {
                let _ = video.seek(source_position, true);
            }
            let muted = self
                .project
                .track(clip.track_id)
                .is_some_and(|track| track.muted);
            video.set_muted(muted);
            video.set_paused(!play);
        }
        self.preview_refresh_ticks = 12;
        self.selected_asset_id = Some(asset_id);
        self.error = None;
        self.sync_audio_previews(position, play);
    }

    pub(super) fn sync_audio_previews(&mut self, position: TimelineTime, play: bool) {
        let loaded_clip_id = self.loaded_clip_id;
        let desired = self
            .project
            .clips
            .iter()
            .filter(|clip| clip.contains(position) && Some(clip.id) != loaded_clip_id)
            .filter_map(|clip| {
                let track = self.project.track(clip.track_id)?;
                let asset = self.project.asset(clip.asset_id?)?;
                if track.muted || !asset.has_audio {
                    return None;
                }
                let source_position = clip.source_in
                    + (position - clip.timeline_start).clamp(TimelineTime::ZERO, clip.duration());
                let path = self.project_root.join(&asset.path);
                let url = Url::from_file_path(path).ok()?;
                Some((clip.id, self.project.audio_duration(source_position), url))
            })
            .collect::<Vec<_>>();
        let desired_ids = desired
            .iter()
            .map(|(clip_id, _, _)| *clip_id)
            .collect::<HashSet<_>>();

        self.audio_previews
            .retain(|clip_id, _| desired_ids.contains(clip_id));
        for (clip_id, source_position, url) in desired {
            if let std::collections::hash_map::Entry::Vacant(entry) =
                self.audio_previews.entry(clip_id)
            {
                match AudioPreview::new(&url) {
                    Ok(preview) => {
                        preview.set_volume(self.preview_volume);
                        preview.seek(source_position);
                        preview.set_playing(play);
                        entry.insert(preview);
                    }
                    Err(error) => {
                        self.error = Some(error);
                        continue;
                    }
                }
            }
            if let Some(preview) = self.audio_previews.get(&clip_id) {
                let expected = source_position;
                if preview.position().abs_diff(expected) > Duration::from_millis(250) {
                    preview.seek(expected);
                }
                preview.set_playing(play);
            }
        }
    }

    pub(super) fn pause_audio_previews(&self) {
        for preview in self.audio_previews.values() {
            preview.set_playing(false);
        }
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
            PreviewTarget::Timeline => {}
        }

        if self.project.clips.is_empty() {
            return;
        }
        if self.playing {
            if let Some(video) = &self.video {
                video.set_paused(true);
            }
            self.pause_audio_previews();
            self.still_playback_started = None;
            self.playing = false;
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

        self.selected_file = Some(relative_path.clone());
        self.selected_asset_id = self
            .project
            .assets
            .iter()
            .find(|asset| asset.path == relative_path)
            .map(|asset| asset.id);

        if is_image || is_video {
            self.preview_target = if is_video {
                PreviewTarget::VideoFile(relative_path.clone())
            } else {
                PreviewTarget::ImageFile(relative_path.clone())
            };
            self.status = None;
            self.error = None;
            if let Some(video) = &self.video {
                video.set_paused(true);
            }
            self.video = None;
            self.loaded_clip_id = None;
            self.playing = false;
            self.pause_audio_previews();
            self.audio_previews.clear();
            self.still_playback_started = None;
            self.preview_volume_open = false;
            self.preview_is_scrubbing = false;
            self.preview_is_adjusting_volume = false;
            self.preview_resume_after_scrub = false;
            self.preview_scrub_fraction = None;
            self.preview_pending_seek_started = None;
            self.preview_last_scrub_seek = None;
            self.preview_refresh_ticks = 2;
        }

        if !is_video {
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

        cx.spawn(async move |editor, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    let video = Video::new_with_options(
                        &url,
                        VideoOptions {
                            frame_buffer_capacity: Some(3),
                            looping: Some(false),
                            speed: Some(1.0),
                        },
                    )
                    .map_err(|error| {
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

impl Editor {
    fn seek_preview_to_fraction(&mut self, fraction: f32, accurate: bool, play: bool) {
        let fraction = fraction.clamp(0.0, 1.0);
        match self.preview_target.clone() {
            PreviewTarget::Timeline => {
                let duration = self.project.timeline_duration().frames();
                let position =
                    TimelineTime::from_frames((duration as f64 * fraction as f64).round() as i64);
                self.load_timeline_position(position, play);
            }
            PreviewTarget::VideoFile(_) => {
                if let Some(video) = &self.video {
                    let target = video.duration().mul_f64(fraction as f64);
                    let _ = video.seek(target, accurate);
                    video.set_paused(!play);
                }
            }
            PreviewTarget::ImageFile(_) => {}
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
        if matches!(self.preview_target, PreviewTarget::ImageFile(_)) {
            return;
        }

        match phase {
            DragPhase::Start => {
                self.preview_resume_after_scrub = match self.preview_target {
                    PreviewTarget::Timeline => self.playing,
                    PreviewTarget::VideoFile(_) => {
                        self.video.as_ref().is_some_and(|video| !video.paused())
                    }
                    PreviewTarget::ImageFile(_) => false,
                };
                if let Some(video) = &self.video {
                    video.set_paused(true);
                }
                self.pause_audio_previews();
                self.playing = false;
                self.still_playback_started = None;
                self.preview_is_scrubbing = true;
                self.preview_scrub_fraction = Some(fraction);
                self.preview_pending_seek_started = None;
                self.preview_last_scrub_seek = Some(Instant::now());
                self.seek_preview_to_fraction(fraction, false, false);
            }
            DragPhase::Update if self.preview_is_scrubbing => {
                self.preview_scrub_fraction = Some(fraction);
                let now = Instant::now();
                let should_seek = self.preview_last_scrub_seek.is_none_or(|last_seek| {
                    now.duration_since(last_seek) >= Duration::from_millis(50)
                });
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
            video.set_volume(self.preview_volume);
            video.set_muted(self.preview_volume <= f64::EPSILON);
        }
        for preview in self.audio_previews.values() {
            preview.set_volume(self.preview_volume);
        }
        cx.notify();
    }

    fn playback_toggle_volume(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let has_playable_target = match self.preview_target {
            PreviewTarget::Timeline => !self.project.clips.is_empty(),
            PreviewTarget::VideoFile(_) => self.video.is_some(),
            PreviewTarget::ImageFile(_) => false,
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
