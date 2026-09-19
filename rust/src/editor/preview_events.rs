use super::*;
use crate::editor::explorer::select_preview_file;
use anyhow::Result;
use gpui::{AsyncApp, WeakEntity};
use opencut_player::video2::{AudioBackend, VideoBackend};
use std::{future::Future, path::Path, pin::Pin};

#[derive(Clone)]
pub enum PreviewEvent {
    SelectFile(PathBuf),
    TogglePlayback,
    Scrub { fraction: f32, phase: DragPhase },
    SetVolume { volume: f64, phase: DragPhase },
}

impl Editor {
    pub async fn handle_preview_event(
        editor: WeakEntity<Self>,
        event: &PreviewEvent,
        cx: &mut AsyncApp,
    ) -> Result<()> {
        match event {
            PreviewEvent::SelectFile(path) => select_preview_file(editor, path.clone(), cx).await,
            PreviewEvent::TogglePlayback => editor.update(cx, |editor, cx| {
                editor.toggle_preview_playback(cx)?;
                cx.notify();
                Ok(())
            })?,
            PreviewEvent::Scrub { fraction, phase } => editor.update(cx, |editor, cx| {
                editor.scrub_preview(*fraction, *phase, cx)?;
                cx.notify();
                Ok(())
            })?,
            PreviewEvent::SetVolume { volume, phase } => editor.update(cx, |editor, cx| {
                match phase {
                    DragPhase::Start => editor.preview.is_adjusting_volume = true,
                    DragPhase::Update | DragPhase::End if !editor.preview.is_adjusting_volume => {
                        return Ok(());
                    }
                    DragPhase::End => editor.preview.is_adjusting_volume = false,
                    DragPhase::Update => {}
                }
                let volume = volume.clamp(0.0, 1.0);
                match &mut editor.preview.target {
                    PreviewTarget::VideoFile(_, video) => {
                        video.set_volume(volume)?;
                        video.set_muted(volume <= f64::EPSILON)?;
                    }
                    PreviewTarget::AudioFile(_, audio) => audio.set_volume(volume)?,
                    PreviewTarget::Timeline => {
                        if let Some(timeline) = &editor.timeline {
                            timeline.video_backend.playback().set_volume(volume);
                            timeline
                                .video_backend
                                .playback()
                                .set_muted(volume <= f64::EPSILON);
                        }
                    }
                    _ => {}
                }
                cx.notify();
                Ok(())
            })?,
        }
    }

    pub fn pause_preview(&mut self) -> Result<()> {
        match &mut self.preview.target {
            PreviewTarget::VideoFile(_, video) => video.set_paused(true)?,
            PreviewTarget::AudioFile(_, audio) => audio.set_paused(true)?,
            PreviewTarget::Timeline => {
                if let Some(timeline) = &self.timeline {
                    timeline.video_backend.playback().set_paused(true);
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn preview_video_playing(&self) -> bool {
        match &self.preview.target {
            PreviewTarget::VideoFile(_, video) => !video.paused(),
            PreviewTarget::Timeline => self
                .timeline
                .as_ref()
                .is_some_and(|timeline| !timeline.video_backend.playback().paused()),
            _ => false,
        }
    }

    pub async fn open_file_preview(
        editor: WeakEntity<Self>,
        project_root: PathBuf,
        relative_path: PathBuf,
        audio_only: bool,
        cx: &mut AsyncApp,
    ) -> Result<()> {
        let source = project_root.join(&relative_path);
        let mut target = if audio_only {
            PreviewTarget::AudioFile(relative_path.clone(), AudioBackend::open(&source).await?)
        } else {
            PreviewTarget::VideoFile(relative_path.clone(), VideoBackend::open(&source).await?)
        };
        editor.update(cx, |editor, cx| -> Result<()> {
            if !file_preview_requested(
                &editor.project_root,
                editor.explorer.selected_file.as_deref(),
                &editor.preview.target,
                &project_root,
                &relative_path,
            ) {
                return Ok(());
            }
            if let PreviewTarget::VideoFile(_, video) = &mut target {
                video.set_paused(false)?;
            }
            editor.preview.target = target;
            editor.status = Some(
                if audio_only {
                    "Audio preview ready."
                } else {
                    "Video preview ready."
                }
                .into(),
            );
            cx.notify();
            Ok(())
        })??;
        Ok(())
    }
}

pub fn file_preview_requested(
    project_root: &Path,
    selected_file: Option<&Path>,
    target: &PreviewTarget,
    requested_root: &Path,
    requested_file: &Path,
) -> bool {
    project_root == requested_root
        && selected_file == Some(requested_file)
        && matches!(target, PreviewTarget::None)
}

impl Editor {
    fn toggle_preview_playback(&mut self, cx: &mut Context<Self>) -> Result<()> {
        let ended = match &self.preview.target {
            PreviewTarget::VideoFile(_, video) => video.ended(),
            PreviewTarget::AudioFile(_, audio) => audio.ended(),
            _ => false,
        };
        if ended {
            return self.seek_file_preview(Duration::ZERO, true, cx);
        }
        match &mut self.preview.target {
            PreviewTarget::VideoFile(_, video) => video.set_paused(!video.paused())?,
            PreviewTarget::AudioFile(_, audio) => audio.set_paused(!audio.paused())?,
            PreviewTarget::Timeline => {
                let Some(timeline) = self.timeline.as_mut() else {
                    return Ok(());
                };
                let paused = timeline.video_backend.playback().paused();
                if timeline.data.clips.is_empty() {
                    return Ok(());
                }
                if paused && timeline.playhead() >= timeline.data.content_duration() {
                    load_timeline_position_with_options(
                        &mut self.preview,
                        timeline,
                        TimelineTime::ZERO,
                    );
                }
                timeline.video_backend.playback().set_paused(!paused);
            }
            _ => {}
        }
        Ok(())
    }

    fn scrub_preview(
        &mut self,
        fraction: f32,
        phase: DragPhase,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        if matches!(
            self.preview.target,
            PreviewTarget::None | PreviewTarget::ImageFile(_)
        ) {
            return Ok(());
        }
        let now = Instant::now();
        match phase {
            DragPhase::Start => {
                self.pause_preview()?;
                self.preview.is_scrubbing = true;
                self.preview.last_scrub_seek = Some(now);
            }
            DragPhase::Update if self.preview.is_scrubbing => {
                if self
                    .preview
                    .last_scrub_seek
                    .is_some_and(|last| now.duration_since(last) < SCRUB_SEEK_INTERVAL)
                {
                    return Ok(());
                }
                self.preview.last_scrub_seek = Some(now);
            }
            DragPhase::End if self.preview.is_scrubbing => {
                self.preview.last_scrub_seek = None;
                self.preview.is_scrubbing = false;
            }
            _ => return Ok(()),
        }
        let fraction = fraction.clamp(0.0, 1.0);
        let duration = match &self.preview.target {
            PreviewTarget::VideoFile(_, video) => video.duration(),
            PreviewTarget::AudioFile(_, audio) => audio.duration(),
            PreviewTarget::Timeline => {
                let Some(timeline) = self.timeline.as_mut() else {
                    return Ok(());
                };
                let duration = timeline.data.content_duration().frames();
                let position =
                    TimelineTime::from_frames((duration as f64 * fraction as f64).round() as i64);
                load_timeline_position_with_options(&mut self.preview, timeline, position);
                return Ok(());
            }
            _ => return Ok(()),
        };
        self.seek_file_preview(duration.mul_f64(fraction as f64), false, cx)
    }

    fn seek_file_preview(
        &mut self,
        position: Duration,
        resume: bool,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let completion: Pin<Box<dyn Future<Output = Result<()>> + Send>> =
            match &mut self.preview.target {
                PreviewTarget::VideoFile(_, video) => {
                    let completion = video.seek(position);
                    if resume {
                        video.set_paused(false)?;
                    }
                    Box::pin(completion)
                }
                PreviewTarget::AudioFile(_, audio) => {
                    let completion = audio.seek(position);
                    if resume {
                        audio.set_paused(false)?;
                    }
                    Box::pin(completion)
                }
                _ => return Ok(()),
            };
        cx.spawn(async move |editor, cx| {
            if let Err(error) = completion.await {
                log::error!("Could not seek file preview: {error:?}");
            }
            let _ = editor.update(cx, |_, cx| cx.notify());
        })
        .detach();
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/preview_events.test.rs"]
mod tests;
