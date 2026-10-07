//! Explicit mapping between live editor state and its persisted subset.
//! Runtime resources and editing history are never serialized.

use crate::layout::MAX_TIMELINE_PIXELS_PER_SECOND;
use crate::layout::MIN_TIMELINE_PIXELS_PER_SECOND;
use crate::timeline::TimelineRuntimeState;
use ::timeline::TimelineSerialization;
use anyhow::{Result, ensure};
use gpui::{point, px};
use std::path::PathBuf;

impl TimelineRuntimeState {
    /// Loads an absolute timeline file path.
    pub fn load(path: PathBuf) -> Result<Self> {
        ensure!(path.is_absolute(), "Timeline path must be absolute");
        let document = TimelineSerialization::load(&path)?;
        Self::from_serialize(document, path)
    }

    pub fn save(&self) -> Result<()> {
        ensure!(self.path.is_absolute(), "Timeline path must be absolute");
        self.to_serialize().save(&self.path)
    }

    /// Selects content, playhead, scroll offsets, zoom, and UI preferences.
    /// Excludes worker state, caches, tasks, history, and transient interactions.
    pub fn to_serialize(&self) -> TimelineSerialization {
        let mut document = TimelineSerialization::from_editing_state(&self.editing_state);
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
    /// `path` must be absolute.
    pub fn from_serialize(document: TimelineSerialization, path: PathBuf) -> Result<Self> {
        ensure!(path.is_absolute(), "Timeline path must be absolute");
        let playhead = document.playhead();
        let (horizontal, vertical) = document.scroll_offset();
        let pixels_per_second = document.pixels_per_second();
        let snapping_enabled = document.snapping_enabled();
        let track_magnet_enabled = document.track_magnet_enabled();
        let mut runtime = Self::new(path, document.editing_state)?;
        runtime.set_playhead(playhead);
        runtime.h_scroll.set_offset(point(px(-horizontal), px(0.0)));
        runtime.v_scroll.set_offset(point(px(0.0), px(-vertical)));
        runtime.pixels_per_second = pixels_per_second.clamp(
            MIN_TIMELINE_PIXELS_PER_SECOND,
            MAX_TIMELINE_PIXELS_PER_SECOND,
        );
        runtime.snapping_enabled = snapping_enabled;
        runtime.track_magnet_enabled = track_magnet_enabled;
        Ok(runtime)
    }
}
