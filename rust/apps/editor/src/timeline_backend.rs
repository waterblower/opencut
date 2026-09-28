//! Timeline frame preparation with an owned playhead and no playback or UI state.
//!
//! The backend owns its decoding worker. Blocking open and seek wait for that
//! worker; snapshot reads never decode or advance time.
//! Layers are prepared for a renderer to compose over a black canvas; text is
//! retained as text so the renderer can perform font shaping and layout.

use crate::editor::preview_timeline::TimelinePreviewFrame;
use ::engine::timeline_decoder::{TimelineDecoder, TimelineFrame};
use ::timeline::{TimelineEditingState, TimelineTime};
use anyhow::{Context as _, Error, Result, anyhow, bail};
use std::{
    path::Path,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::Duration,
};

pub struct TimelineBackend {
    timeline: Arc<TimelineEditingState>,
    revision: u64,
    commands: mpsc::Sender<SeekRequest>,
    frame: Arc<Mutex<Arc<TimelineFrame>>>,
    preview: Arc<Mutex<PreviewRequest>>,
}

impl TimelineBackend {
    /// Validates the document and media root, then starts preparing frame zero.
    /// Returns validation and worker-start errors immediately. Media decoding
    /// stays on the worker; its failures are returned by `preview_frame`.
    pub fn new(timeline: TimelineEditingState, media_root: &Path) -> Result<Self> {
        let metadata = media_root.metadata().context(format!(
            "Inspecting timeline media root {}",
            media_root.display()
        ))?;
        if !metadata.is_dir() {
            bail!(
                "Timeline media root is not a directory: {}",
                media_root.display()
            );
        }
        timeline.validate()?;
        let timeline = Arc::new(timeline);
        let frame = Arc::new(Mutex::new(Arc::new(TimelineFrame {
            timestamp: Duration::ZERO,
            width: timeline.settings.width,
            height: timeline.settings.height,
            layers: Vec::new(),
        })));
        let preview = Arc::new(Mutex::new(PreviewRequest {
            revision: 0,
            position: Duration::ZERO,
            result: None,
        }));
        let (commands, requests) = mpsc::channel::<SeekRequest>();
        let backend = Self {
            timeline,
            revision: 0,
            commands,
            frame,
            preview,
        };
        let media_root = media_root.to_owned();
        let frame = Arc::clone(&backend.frame);
        let preview = Arc::clone(&backend.preview);
        thread::Builder::new()
            .name("timeline-preview".into())
            .spawn(move || {
                let mut decoder: Option<(u64, TimelineDecoder)> = None;
                // Dropping the backend closes the channel; decoder cleanup stays here.
                for request in requests {
                    if request.reply.is_none() {
                        let preview = preview.lock().unwrap();
                        if preview.revision != request.revision
                            || preview.position != request.position
                        {
                            continue;
                        }
                    }
                    let result = (|| -> Result<Arc<TimelinePreviewFrame>> {
                        if decoder
                            .as_ref()
                            .is_none_or(|(revision, _)| *revision != request.revision)
                        {
                            decoder = Some((request.revision, TimelineDecoder::new(&media_root)));
                        }
                        let (_, decoder) = decoder.as_mut().unwrap();
                        let frame =
                            Arc::new(decoder.frame_at(&request.timeline, request.position)?);
                        let prepared = Arc::new(TimelinePreviewFrame::new(frame));
                        Ok(prepared)
                    })();
                    let result = match result {
                        Ok(prepared) => Ok(prepared),
                        Err(error) => Err(Arc::new(error)),
                    };
                    {
                        let mut preview = preview.lock().unwrap();
                        if preview.revision == request.revision
                            && preview.position == request.position
                        {
                            if let Ok(prepared) = &result {
                                *frame.lock().unwrap() = Arc::clone(&prepared.frame);
                            }
                            preview.result = Some(result.clone());
                        }
                    }
                    if let Some(reply) = request.reply {
                        let _ = reply.send(result);
                    }
                }
            })
            .context("Starting timeline preview worker")?;
        backend
            .commands
            .send(SeekRequest {
                timeline: Arc::clone(&backend.timeline),
                revision: backend.revision,
                position: Duration::ZERO,
                reply: None,
            })
            .context("Requesting initial timeline frame")?;
        Ok(backend)
    }

    pub fn timeline(&self) -> &TimelineEditingState {
        &self.timeline
    }

    /// Commits validated content and requests a frame at the current playhead.
    /// Rejection preserves content, position, and cached frames. Worker snapshots
    /// keep their previous content until decoding completes.
    pub fn replace_timeline(&mut self, timeline: TimelineEditingState) -> Result<()> {
        timeline.validate()?;
        let mut preview = self.preview.lock().unwrap();
        let position = clamp_position(&timeline, preview.position)?;
        let revision = self.revision.wrapping_add(1);
        let timeline = Arc::new(timeline);
        self.commands
            .send(SeekRequest {
                timeline: Arc::clone(&timeline),
                revision,
                position,
                reply: None,
            })
            .context("Requesting edited timeline frame")?;
        self.timeline = timeline;
        self.revision = revision;
        *preview = PreviewRequest {
            revision,
            position,
            result: None,
        };
        Ok(())
    }

    /// Moves the playhead immediately and prepares its frame on the worker.
    /// Snaps and clamps to the timeline; repeated requests reuse the cached result.
    /// Decode failures preserve the last successful snapshot, not the old playhead.
    pub fn seek(&self, position: Duration) -> Result<()> {
        let position = clamp_position(&self.timeline, position)?;
        let mut preview = self.preview.lock().unwrap();
        if preview.revision != self.revision || preview.position != position {
            self.commands
                .send(SeekRequest {
                    timeline: Arc::clone(&self.timeline),
                    revision: self.revision,
                    position,
                    reply: None,
                })
                .context("Requesting timeline preview frame")?;
            *preview = PreviewRequest {
                revision: self.revision,
                position,
                result: None,
            };
        }
        Ok(())
    }

    /// Requests the given timeline frame and immediately returns its cached
    /// presentation, or `None` while the internal worker prepares it.
    pub fn preview_frame(
        &self,
        position: TimelineTime,
    ) -> Result<Option<Arc<TimelinePreviewFrame>>> {
        self.seek(self.timeline.duration(position))?;
        let preview = self.preview.lock().unwrap();
        match &preview.result {
            Some(Ok(frame)) => Ok(Some(Arc::clone(frame))),
            Some(Err(error)) => Err(anyhow!("{error:?}")),
            None => Ok(None),
        }
    }

    pub fn frame_size(&self) -> (u32, u32) {
        (self.timeline.settings.width, self.timeline.settings.height)
    }

    pub fn framerate(&self) -> Option<f64> {
        Some(self.timeline.settings.frame_rate.frames_per_second())
    }

    /// Includes audio clips and invisible tracks in the document's duration.
    pub fn duration(&self) -> Duration {
        self.timeline.duration(self.timeline.content_duration())
    }

    /// The requested playhead, independent of decoding progress or failures.
    pub fn position(&self) -> Duration {
        self.preview.lock().unwrap().position
    }

    /// Waits until the worker publishes the requested frame. A failed seek
    /// preserves the previous snapshot and position.
    pub fn seek_sync(&mut self, position: Duration) -> Result<()> {
        let position = clamp_position(&self.timeline, position)?;
        let (reply, completion) = mpsc::channel();
        self.commands
            .send(SeekRequest {
                timeline: Arc::clone(&self.timeline),
                revision: self.revision,
                position,
                reply: Some(reply),
            })
            .context("Requesting timeline seek")?;
        let prepared = match completion.recv().context("Waiting for timeline seek")? {
            Ok(prepared) => prepared,
            Err(error) => return Err(anyhow!("{error:?}")),
        };
        let mut preview = self.preview.lock().unwrap();
        *self.frame.lock().unwrap() = Arc::clone(&prepared.frame);
        *preview = PreviewRequest {
            revision: self.revision,
            position,
            result: Some(Ok(prepared)),
        };
        Ok(())
    }

    /// Clones a snapshot handle without performing I/O. Older snapshots remain
    /// valid after later seeks and after this backend is dropped.
    pub fn get_current_frame(&self) -> Result<Arc<TimelineFrame>> {
        Ok(Arc::clone(&self.frame.lock().unwrap()))
    }
}

struct SeekRequest {
    timeline: Arc<TimelineEditingState>,
    revision: u64,
    position: Duration,
    reply: Option<mpsc::Sender<Result<Arc<TimelinePreviewFrame>, Arc<Error>>>>,
}

struct PreviewRequest {
    revision: u64,
    position: Duration,
    result: Option<Result<Arc<TimelinePreviewFrame>, Arc<Error>>>,
}

fn clamp_position(timeline: &TimelineEditingState, position: Duration) -> Result<Duration> {
    let rate = timeline.settings.frame_rate;
    if rate.numerator == 0 || rate.denominator == 0 {
        bail!("Timeline frame rate must have a positive numerator and denominator");
    }
    let last = (timeline.content_duration() - TimelineTime::ONE_FRAME).max(TimelineTime::ZERO);
    let position = rate
        .frames_from_duration_nearest(position)
        .clamp(TimelineTime::ZERO, last);
    Ok(rate.duration(position))
}

#[cfg(test)]
#[path = "tests/timeline_backend.test.rs"]
mod tests;
