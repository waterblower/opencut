//! Timeline editing content and queries shared by the editor and CLI.

use crate::timeline::{
    Clip, MediaAsset, MediaKind, TimelineSettings, TimelineTime, Track, TrackKind,
};
use anyhow::{Result, bail};
use std::{collections::HashSet, time::Duration};
use ulid::Ulid;

/// Timeline content used by editing operations, rendering, and editing history.
#[derive(Clone, Debug, Default)]
pub struct TimelineEditingState {
    pub settings: TimelineSettings,
    pub assets: Vec<MediaAsset>,
    pub tracks: Vec<Track>,
    pub clips: Vec<Clip>,
}

impl TimelineEditingState {
    pub fn asset(&self, id: Ulid) -> Option<&MediaAsset> {
        self.assets.iter().find(|asset| asset.id == id)
    }
    pub fn clip(&self, id: Ulid) -> Option<&Clip> {
        self.clips.iter().find(|clip| clip.id() == id)
    }
    pub fn clip_mut(&mut self, id: Ulid) -> Option<&mut Clip> {
        self.clips.iter_mut().find(|clip| clip.id() == id)
    }
    pub fn clip_index(&self, id: Ulid) -> Option<usize> {
        self.clips.iter().position(|clip| clip.id() == id)
    }
    pub fn content_duration(&self) -> TimelineTime {
        let frame_rate = self.settings.frame_rate;
        self.clips
            .iter()
            .map(|clip| clip.timeline_end(frame_rate))
            .max()
            .unwrap_or(TimelineTime::ZERO)
    }
    pub fn seconds(&self, time: TimelineTime) -> f64 {
        self.settings.frame_rate.seconds(time)
    }
    pub fn duration(&self, time: TimelineTime) -> Duration {
        self.settings.frame_rate.duration(time)
    }
    pub fn nearest_time(&self, seconds: f64) -> TimelineTime {
        self.settings.frame_rate.nearest(seconds)
    }
    pub fn audio_duration(&self, time: TimelineTime) -> Duration {
        let samples = self
            .settings
            .frame_rate
            .audio_samples(time, self.settings.audio_sample_rate);
        Duration::from_secs_f64(samples as f64 / self.settings.audio_sample_rate as f64)
    }
    pub fn source_frame_at(&self, clip: &Clip, timeline_position: TimelineTime) -> Option<i64> {
        let asset = self.asset(clip.media()?.asset_id)?;
        let source_rate = asset.frame_rate()?;
        let source_time = clip.source_time_at(timeline_position)?;
        Some(
            self.settings
                .frame_rate
                .rescale_floor(source_time, source_rate)
                .frames(),
        )
    }
    pub fn source_position_at(&self, clip: &Clip, timeline_position: TimelineTime) -> Duration {
        let Some(media) = clip.media() else {
            return Duration::ZERO;
        };
        let Some(asset) = self.asset(media.asset_id) else {
            return Duration::ZERO;
        };
        if let (Some(source_rate), Some(source_frame)) = (
            asset.frame_rate(),
            self.source_frame_at(clip, timeline_position),
        ) {
            return source_rate.duration(TimelineTime::from_frames(source_frame));
        }
        self.audio_duration(clip.source_time_at(timeline_position).unwrap_or_default())
    }
    pub fn source_start_seconds(&self, clip: &Clip) -> f64 {
        self.source_position_at(clip, clip.timeline_start())
            .as_secs_f64()
    }
    pub fn ceil_time(&self, seconds: f64) -> TimelineTime {
        self.settings.frame_rate.ceil(seconds)
    }
    pub fn clip_locked(&self, clip_id: Ulid) -> bool {
        self.clip(clip_id)
            .and_then(|clip| self.track(clip.track_id()))
            .is_some_and(|track| track.locked)
    }
    pub fn track(&self, id: Ulid) -> Option<&Track> {
        self.tracks.iter().find(|track| track.id == id)
    }
    pub fn track_mut(&mut self, id: Ulid) -> Option<&mut Track> {
        self.tracks.iter_mut().find(|track| track.id == id)
    }
    pub fn clips_on_track(&self, track_id: Ulid) -> impl Iterator<Item = &Clip> {
        self.clips
            .iter()
            .filter(move |clip| clip.track_id() == track_id)
    }
}

impl TimelineEditingState {
    /// Validates settings, unique track and asset IDs, and visual clip references,
    /// timing, and properties. Audio clips are not validated here.
    pub fn validate(&self) -> Result<()> {
        let settings = self.settings;
        if settings.width == 0 || settings.height == 0 {
            bail!("Timeline canvas dimensions must be positive");
        }
        if settings.frame_rate.numerator == 0 || settings.frame_rate.denominator == 0 {
            bail!("Timeline frame rate must have a positive numerator and denominator");
        }
        if settings.audio_sample_rate == 0 {
            bail!("Timeline audio sample rate must be positive");
        }
        let mut track_ids = HashSet::new();
        for track in &self.tracks {
            if !track_ids.insert(track.id) {
                bail!("Duplicate timeline track {}", track.id);
            }
        }
        let mut asset_ids = HashSet::new();
        for asset in &self.assets {
            if !asset_ids.insert(asset.id) {
                bail!("Duplicate timeline asset {}", asset.id);
            }
        }
        let mut clip_ids = HashSet::new();
        for clip in &self.clips {
            if matches!(clip, Clip::Audio(_)) {
                continue;
            }
            if !clip_ids.insert(clip.id()) {
                bail!("Duplicate visual clip {}", clip.id());
            }
            let Some(track) = self.track(clip.track_id()) else {
                bail!(
                    "Visual clip {} references missing track {}",
                    clip.id(),
                    clip.track_id()
                );
            };
            if clip.timeline_start() < TimelineTime::ZERO
                || clip.frame_length(settings.frame_rate) <= TimelineTime::ZERO
            {
                bail!("Visual clip {} has an invalid time range", clip.id());
            }
            match clip {
                Clip::Video(media) => {
                    if track.kind != TrackKind::Video {
                        bail!("Video clip {} requires a video track", media.id);
                    }
                    let Some(asset) = self.asset(media.asset_id) else {
                        bail!(
                            "Visual clip {} references missing asset {}",
                            media.id,
                            media.asset_id
                        );
                    };
                    if asset.kind == MediaKind::Audio {
                        bail!(
                            "Visual clip {} references audio asset {}",
                            media.id,
                            asset.id
                        );
                    }
                    if media.source_in < TimelineTime::ZERO {
                        bail!("Visual clip {} has a negative source trim", media.id);
                    }
                    let properties = media.video_properties;
                    if !properties.position_x.is_finite()
                        || !properties.position_y.is_finite()
                        || !properties.scale.is_finite()
                        || properties.scale < 0.0
                    {
                        bail!("Visual clip {} has invalid transform properties", media.id);
                    }
                }
                Clip::Text(text) => {
                    if track.kind != TrackKind::Text {
                        bail!("Text clip {} requires a text track", text.id);
                    }
                    let properties = &text.properties;
                    if !properties.position_x.is_finite()
                        || !properties.position_y.is_finite()
                        || !properties.font_size.is_finite()
                        || properties.font_size <= 0.0
                    {
                        bail!("Text clip {} has invalid layout properties", text.id);
                    }
                }
                Clip::Audio(_) => {}
            }
        }
        Ok(())
    }
}
