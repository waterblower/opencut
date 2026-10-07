use crate::{TimelineEditingState, TimelineFrameIndex};
use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use ulid::Ulid;

/// Timeline editing content and the view preferences selected for persistence.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
pub struct TimelineSerialization {
    pub editing_state: TimelineEditingState,
    #[serde(default)]
    view_state: TimelineViewState,
}

// Public APIs only
impl TimelineSerialization {
    /// Frame count from frame zero to the latest clip end, including gaps.
    pub fn frame_count(&self) -> i64 {
        self.editing_state
            .content_duration()
            .max(TimelineFrameIndex::ZERO)
            .into()
    }

    /// Captures editing content with default persisted view preferences.
    pub fn from_editing_state(editing_state: &TimelineEditingState) -> Self {
        Self {
            editing_state: editing_state.clone(),
            view_state: TimelineViewState::default(),
        }
    }

    pub fn set_view_state(
        &mut self,
        playhead: TimelineFrameIndex,
        scroll: (f32, f32),
        pixels_per_second: f32,
        snapping_enabled: bool,
        track_magnet_enabled: bool,
    ) {
        self.view_state = TimelineViewState {
            saved_playhead_frame: i64::from(playhead).max(0),
            horizontal_scroll: nonnegative_finite(scroll.0),
            vertical_scroll: nonnegative_finite(scroll.1),
            pixels_per_second: if pixels_per_second.is_finite() && pixels_per_second > 0.0 {
                pixels_per_second
            } else {
                72.0
            },
            snapping_enabled,
            track_magnet_enabled,
        };
    }

    pub fn playhead(&self) -> TimelineFrameIndex {
        self.view_state.saved_playhead_frame.max(0).into()
    }

    pub fn scroll_offset(&self) -> (f32, f32) {
        (
            nonnegative_finite(self.view_state.horizontal_scroll),
            nonnegative_finite(self.view_state.vertical_scroll),
        )
    }

    pub fn pixels_per_second(&self) -> f32 {
        let zoom = self.view_state.pixels_per_second;
        if zoom.is_finite() && zoom > 0.0 {
            zoom
        } else {
            72.0
        }
    }

    pub fn snapping_enabled(&self) -> bool {
        self.view_state.snapping_enabled
    }

    pub fn track_magnet_enabled(&self) -> bool {
        self.view_state.track_magnet_enabled
    }

    pub fn load(path: &Path) -> Result<Self> {
        let contents = fs::read(path).context(format!("Reading timeline {}", path.display()))?;
        let document: Self = serde_json::from_slice(&contents)
            .context(format!("Parsing timeline JSON {}", path.display()))?;
        document
            .editing_state
            .validate()
            .context(format!("Validating timeline {}", path.display()))?;
        Ok(document)
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let directory = path
            .parent()
            .context("Timeline path has no parent directory")?;
        fs::create_dir_all(directory).context(format!(
            "Creating timeline directory {}",
            directory.display()
        ))?;
        let mut bytes = serde_json::to_vec_pretty(self).context("Serializing timeline")?;
        bytes.push(b'\n');
        let temporary = path.with_extension("json.tmp");
        fs::write(&temporary, bytes).context(format!(
            "Writing temporary timeline {}",
            temporary.display()
        ))?;
        fs::rename(&temporary, path).context(format!("Replacing timeline {}", path.display()))?;
        Ok(())
    }
}

pub fn deserialize_ulid<'de, D>(deserializer: D) -> Result<Ulid, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum SerializedUlid {
        String(String),
        Legacy(u64),
    }

    match SerializedUlid::deserialize(deserializer)? {
        SerializedUlid::String(value) => value.parse().map_err(serde::de::Error::custom),
        SerializedUlid::Legacy(value) => Ok(Ulid::from(u128::from(value))),
    }
}

/// Only the view preferences selected for persistence.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
#[serde(default)]
struct TimelineViewState {
    saved_playhead_frame: i64,
    horizontal_scroll: f32,
    vertical_scroll: f32,
    pixels_per_second: f32,
    snapping_enabled: bool,
    track_magnet_enabled: bool,
}

impl Default for TimelineViewState {
    fn default() -> Self {
        Self {
            saved_playhead_frame: 0,
            horizontal_scroll: 0.0,
            vertical_scroll: 0.0,
            pixels_per_second: 72.0,
            snapping_enabled: true,
            track_magnet_enabled: true,
        }
    }
}

fn nonnegative_finite(value: f32) -> f32 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}
