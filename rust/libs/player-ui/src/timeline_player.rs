//! Standalone timeline player entity: a [`TimelineBackend`] plus its own loop and controls.
//! Audio is not played yet.

use anyhow::Result;
use engine::timeline_backend::{MAX_CONTROL_WAIT, TimelineBackend};
use gpui::{Context, Task};
use std::{path::Path, time::Duration};
use timeline::TimelineEditingState;

pub struct TimelinePlayer {
    pub backend: TimelineBackend, // 直接修改后需自行 notify 并释放旧图像。
    pub title: String,
    pub error: Option<String>, // 最近一次播放失败；播放已暂停，保留上一帧。
}

impl TimelinePlayer {
    /// Validates the timeline and prepares frame zero, paused.
    /// Media paths resolve against `project_root`. Call [`Self::start`] once the player is in an entity.
    pub fn new(timeline: TimelineEditingState, project_root: &Path) -> Result<Self> {
        Ok(Self {
            backend: TimelineBackend::new(timeline, project_root)?,
            title: String::new(),
            error: None,
        })
    }

    /// Starts the playback loop once. The owner must retain the task and drop it before the player.
    pub fn start(&mut self, cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |player, cx| {
            loop {
                let Ok(wait) = player.update(cx, |player, cx| match player.backend.advance() {
                    Ok(advance) => {
                        if advance.changed {
                            player.changed(cx);
                        }
                        advance.wait
                    }
                    Err(error) => {
                        player.fail(error, cx);
                        MAX_CONTROL_WAIT
                    }
                }) else {
                    return; // 播放器已释放。
                };
                cx.background_executor().timer(wait).await;
            }
        })
    }

    pub fn play(&mut self, cx: &mut Context<Self>) -> Result<()> {
        self.error = None;
        let result = self.backend.play();
        self.changed(cx);
        result
    }

    pub fn toggle_playback(&mut self, cx: &mut Context<Self>) -> Result<()> {
        if self.backend.is_playing() {
            self.backend.pause();
            self.changed(cx);
            Ok(())
        } else {
            self.play(cx)
        }
    }

    pub fn seek(&mut self, position: Duration, cx: &mut Context<Self>) -> Result<()> {
        let result = self.backend.seek(position);
        self.changed(cx);
        result
    }

    /// Pauses on the last good frame and records the error for display.
    pub(crate) fn fail(&mut self, error: anyhow::Error, cx: &mut Context<Self>) {
        eprintln!("Timeline player failed: {error:?}");
        self.backend.pause();
        self.error = Some(format!("{error:#}"));
        self.changed(cx);
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.backend.release_retired_images(cx);
        cx.notify();
    }
}
