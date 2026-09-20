use crate::timeline::{Clip, MediaKind, TimelineEditingState, TimelineTime, TrackKind};
use anyhow::{Result, bail};
use std::collections::HashSet;

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
