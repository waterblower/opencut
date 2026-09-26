//! Explicit mapping between live editor state and its persisted subset.
//! Runtime resources and editing history are never serialized.

use crate::editor::timeline::TimelineRuntimeState;
use crate::editor::{MAX_TIMELINE_PIXELS_PER_SECOND, MIN_TIMELINE_PIXELS_PER_SECOND};
use anyhow::{Result, ensure};
use gpui::{point, px};
use opencut_player::timeline::TimelineSerialization;
use std::path::{Path, PathBuf};

impl TimelineRuntimeState {
    /// Loads an absolute timeline file path. Media paths resolve against `media_root`.
    pub fn load(path: PathBuf, media_root: &Path) -> Result<Self> {
        ensure!(path.is_absolute(), "Timeline path must be absolute");
        let document = TimelineSerialization::load(&path)?;
        Self::from_serialize(document, path, media_root)
    }

    pub fn save(&self) -> Result<()> {
        ensure!(self.path.is_absolute(), "Timeline path must be absolute");
        self.to_serialize().save(&self.path)
    }

    /// Selects content, backend position, scroll offsets, zoom, and UI preferences.
    /// Excludes worker state, caches, tasks, history, and transient interactions.
    pub fn to_serialize(&self) -> TimelineSerialization {
        let mut document = TimelineSerialization::from_editing_state(self.backend.timeline());
        document.set_view_state(
            self.playhead(),
            (
                -f32::from(self.h_scroll.offset().x),
                -f32::from(self.v_scroll.offset().y),
            ),
            self.pixels_per_second,
            self.snapping_enabled,
            self.track_magnet_enabled,
        );
        document
    }

    /// Rebuilds runtime resources and restores only persisted content/preferences.
    /// `path` must be absolute; media paths resolve against `media_root`.
    pub fn from_serialize(
        document: TimelineSerialization,
        path: PathBuf,
        media_root: &Path,
    ) -> Result<Self> {
        ensure!(path.is_absolute(), "Timeline path must be absolute");
        let mut runtime = Self::new(path, document.to_editing_state(), media_root)?;
        runtime
            .backend
            .seek(runtime.backend.timeline().duration(document.playhead()))?;
        let (horizontal, vertical) = document.scroll_offset();
        runtime.h_scroll.set_offset(point(px(-horizontal), px(0.0)));
        runtime.v_scroll.set_offset(point(px(0.0), px(-vertical)));
        runtime.pixels_per_second = document.pixels_per_second().clamp(
            MIN_TIMELINE_PIXELS_PER_SECOND,
            MAX_TIMELINE_PIXELS_PER_SECOND,
        );
        runtime.snapping_enabled = document.snapping_enabled();
        runtime.track_magnet_enabled = document.track_magnet_enabled();
        Ok(runtime)
    }
}
