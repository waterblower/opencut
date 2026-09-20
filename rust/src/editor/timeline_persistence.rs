//! Explicit mapping between live editor state and its persisted subset.
//! Runtime resources and editing history are never serialized.

use crate::editor::timeline::TimelineRuntimeState;
use anyhow::{Result, ensure};
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
        unimplemented!("map the persisted subset through the opaque document's conversion API")
    }

    /// Rebuilds runtime resources and restores only persisted content/preferences.
    /// `path` must be absolute; media paths resolve against `media_root`.
    pub fn from_serialize(
        _document: TimelineSerialization,
        path: PathBuf,
        _media_root: &Path,
    ) -> Result<Self> {
        ensure!(path.is_absolute(), "Timeline path must be absolute");
        unimplemented!("map private disk data to runtime content and restore backend position")
    }
}
