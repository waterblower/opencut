use super::{
    decode::VideoWorker,
    raster::{self, TextRaster},
};
use crate::{
    cli::error::Result,
    cli_error,
    timeline::{Clip, MediaKind, TimelineSerialization, TimelineTime, TrackKind},
};
use image::{Rgba, RgbaImage, imageops};
use std::{collections::HashMap, path::Path};
use ulid::Ulid;

#[derive(Default)]
pub struct Composer {
    decoders: HashMap<Ulid, VideoWorker>,
    rasters: HashMap<Ulid, RgbaImage>,
    text: TextRaster,
}

impl Composer {
    pub fn frame(
        &mut self,
        doc: &TimelineSerialization,
        base: &Path,
        frame: i64,
    ) -> Result<RgbaImage> {
        let settings = &doc.settings;
        let mut output =
            RgbaImage::from_pixel(settings.width, settings.height, Rgba([0, 0, 0, 255]));
        let mut active = Vec::new();
        let time = TimelineTime::from_frames(frame);
        // GES assigns higher priority to earlier tracks, with text above all video tracks.
        for track in doc
            .tracks
            .iter()
            .rev()
            .filter(|track| track.kind == TrackKind::Video)
            .chain(
                doc.tracks
                    .iter()
                    .rev()
                    .filter(|track| track.kind == TrackKind::Text),
            )
        {
            if !track.visible {
                continue;
            }
            for clip in &doc.clips {
                if clip.track_id() != track.id
                    || time < clip.timeline_start()
                    || time >= clip.timeline_end(settings.frame_rate)
                {
                    continue;
                }
                active.push(clip.id());
                let layer = self.layer(doc, base, clip, time)?;
                imageops::overlay(&mut output, &layer, 0, 0);
                break;
            }
        }
        self.decoders.retain(|id, _| active.contains(id));
        self.rasters.retain(|id, _| active.contains(id));
        Ok(output)
    }

    fn layer(
        &mut self,
        doc: &TimelineSerialization,
        base: &Path,
        clip: &Clip,
        time: TimelineTime,
    ) -> Result<RgbaImage> {
        let settings = &doc.settings;
        if let Clip::Text(text) = clip {
            if !self.rasters.contains_key(&text.id) {
                self.rasters.insert(
                    text.id,
                    self.text
                        .raster(&text.properties, settings.width, settings.height)?,
                );
            }
            return Ok(self.rasters[&text.id].clone());
        }
        let Some(data) = clip.media() else {
            return Err(cli_error!(
                "invalid_clip",
                "/clips",
                3,
                "visual track requires media or text"
            ));
        };
        let Some(asset) = doc.asset(data.asset_id) else {
            return Err(cli_error!(
                "unknown_asset",
                "/clips",
                3,
                "clip references missing asset {}",
                data.asset_id
            ));
        };
        let path = base.join(&asset.path);
        let source = if asset.kind == MediaKind::Image {
            if !self.rasters.contains_key(&data.id) {
                self.rasters.insert(data.id, raster::load_image(&path)?);
            }
            self.rasters[&data.id].clone()
        } else {
            if !self.decoders.contains_key(&data.id) {
                self.decoders.insert(data.id, VideoWorker::new(path));
            }
            // Use the same source-frame floor mapping as the GUI/GES source in-point.
            self.decoders[&data.id].at(doc.source_position_at(clip, time).as_secs_f64())?
        };
        let properties = data.video_properties;
        let fit = (settings.width as f64 / source.width() as f64)
            .min(settings.height as f64 / source.height() as f64);
        let width = source.width() as f64 * fit * properties.scale;
        let height = source.height() as f64 * fit * properties.scale;
        let mut layer = RgbaImage::new(settings.width, settings.height);
        if width <= 0.0 || height <= 0.0 {
            return Ok(layer);
        }
        if width * height > 268_435_456.0 {
            return Err(cli_error!(
                "raster_too_large",
                "",
                5,
                "transformed clip exceeds pixel allocation limit"
            ));
        }
        let left = settings.width as f64 * 0.5 + properties.position_x - width * 0.5;
        let top = settings.height as f64 * 0.5 + properties.position_y - height * 0.5;
        let source = imageops::resize(
            &source,
            width.round().max(1.0) as u32,
            height.round().max(1.0) as u32,
            imageops::FilterType::Triangle,
        );
        imageops::overlay(&mut layer, &source, left.round() as i64, top.round() as i64);
        Ok(layer)
    }
}
