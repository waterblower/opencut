//! Timeline editing content and queries shared by the editor and CLI.

use crate::{Clip, MediaAsset, MediaKind, TimelineFrameIndex, TimelineSettings, Track, TrackKind};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, time::Duration};
use ulid::Ulid;

/// Timeline content used by editing operations, rendering, and editing history.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "timeline-schema", derive(schemars::JsonSchema))]
#[serde(default)]
pub struct TimelineEditingState {
    pub settings: TimelineSettings,
    pub assets: Vec<MediaAsset>,
    #[serde(alias = "layers")]
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
    pub fn content_duration(&self) -> TimelineFrameIndex {
        let frame_rate = self.settings.frame_rate;
        self.clips
            .iter()
            .map(|clip| clip.timeline_end(frame_rate))
            .max()
            .unwrap_or(TimelineFrameIndex::ZERO)
    }
    pub fn seconds(&self, time: TimelineFrameIndex) -> f64 {
        self.settings.frame_rate.seconds(time)
    }
    pub fn position_at_frame(&self, time: TimelineFrameIndex) -> Duration {
        self.settings.frame_rate.duration(time)
    }
    pub fn nearest_time(&self, seconds: f64) -> TimelineFrameIndex {
        self.settings.frame_rate.nearest(seconds)
    }
    pub fn audio_duration(&self, time: TimelineFrameIndex) -> Duration {
        let samples = self
            .settings
            .frame_rate
            .audio_samples(time, self.settings.audio_sample_rate);
        Duration::from_secs_f64(samples as f64 / self.settings.audio_sample_rate as f64)
    }
    pub fn source_frame_at(
        &self,
        clip: &Clip,
        timeline_position: TimelineFrameIndex,
    ) -> Option<i64> {
        let asset = self.asset(clip.asset_id()?)?;
        let source_rate = asset.frame_rate()?;
        let source_time = clip.source_time_at(timeline_position)?;
        Some(
            self.settings
                .frame_rate
                .rescale_floor(source_time, source_rate)
                .into(),
        )
    }
    pub fn source_position_at(
        &self,
        clip: &Clip,
        timeline_position: TimelineFrameIndex,
    ) -> Duration {
        let Some(asset_id) = clip.asset_id() else {
            return Duration::ZERO;
        };
        let Some(asset) = self.asset(asset_id) else {
            return Duration::ZERO;
        };
        if let (Some(source_rate), Some(source_frame)) = (
            asset.frame_rate(),
            self.source_frame_at(clip, timeline_position),
        ) {
            return source_rate.duration(source_frame.into());
        }
        self.audio_duration(clip.source_time_at(timeline_position).unwrap_or_default())
    }
    pub fn source_start_seconds(&self, clip: &Clip) -> f64 {
        self.source_position_at(clip, clip.timeline_start())
            .as_secs_f64()
    }
    pub fn ceil_time(&self, seconds: f64) -> TimelineFrameIndex {
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
    /// Validates settings, unique IDs, clip references, timing, properties, and no overlaps within tracks.
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
            if !clip_ids.insert(clip.id()) {
                bail!("Duplicate clip {}", clip.id());
            }
            let Some(track) = self.track(clip.track_id()) else {
                bail!(
                    "Clip {} references missing track {}",
                    clip.id(),
                    clip.track_id()
                );
            };
            if clip.timeline_start() < TimelineFrameIndex::ZERO
                || clip.frame_length(settings.frame_rate) <= TimelineFrameIndex::ZERO
            {
                bail!("Clip {} has an invalid time range", clip.id());
            }
            match clip {
                Clip::Video(media) => {
                    if track.kind != TrackKind::Video {
                        bail!("Video clip {} requires a video track", media.id());
                    }
                    let Some(asset) = self.asset(media.asset_id) else {
                        bail!(
                            "Clip {} references missing asset {}",
                            media.id(),
                            media.asset_id
                        );
                    };
                    if asset.kind == MediaKind::Audio {
                        bail!("Clip {} references audio asset {}", media.id(), asset.id);
                    }
                    if media.source_in < TimelineFrameIndex::ZERO {
                        bail!("Clip {} has a negative source trim", media.id());
                    }
                    let properties = media.video_properties;
                    if !properties.position_x.is_finite()
                        || !properties.position_y.is_finite()
                        || !properties.scale.is_finite()
                        || properties.scale < 0.0
                    {
                        bail!("Clip {} has invalid transform properties", media.id());
                    }
                }
                Clip::Text(text) => {
                    if track.kind != TrackKind::Text {
                        bail!("Text clip {} requires a text track", text.id());
                    }
                    let properties = &text.properties;
                    if !properties.position.x.is_finite()
                        || !properties.position.y.is_finite()
                        || !properties.font_size.is_finite()
                        || properties.font_size <= 0.0
                    {
                        bail!("Text clip {} has invalid layout properties", text.id());
                    }
                }
                Clip::Audio(audio) => {
                    if track.kind != TrackKind::Audio {
                        bail!("Audio clip {} requires an audio track", audio.id());
                    }
                    let Some(asset) = self.asset(audio.asset_id) else {
                        bail!(
                            "Audio clip {} references missing asset {}",
                            audio.id(),
                            audio.asset_id
                        );
                    };
                    if asset.kind == MediaKind::Image {
                        bail!(
                            "Audio clip {} references image asset {}",
                            audio.id(),
                            asset.id
                        );
                    }
                    if audio.source_in < TimelineFrameIndex::ZERO {
                        bail!("Audio clip {} has a negative source trim", audio.id());
                    }
                    if !audio.audio_properties.gain_db.is_finite() {
                        bail!("Audio clip {} has invalid audio gain", audio.id());
                    }
                }
            }
        }
        let mut intervals = self
            .clips
            .iter()
            .map(|clip| {
                (
                    clip.track_id(),
                    clip.timeline_start(),
                    clip.timeline_end(settings.frame_rate),
                    clip.id(),
                )
            })
            .collect::<Vec<_>>();
        intervals.sort_unstable();
        for pair in intervals.windows(2) {
            let (track_id, _, previous_end, previous_id) = pair[0];
            let (next_track_id, next_start, _, next_id) = pair[1];
            if track_id == next_track_id && next_start < previous_end {
                // 区间为 [start, end)，首尾相接不算重叠。
                bail!("Clips {previous_id} and {next_id} overlap on track {track_id}");
            }
        }
        Ok(())
    }
}
