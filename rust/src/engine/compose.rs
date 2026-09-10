use super::{
    decode::VideoWorker,
    effects,
    raster::{self, TextRaster},
};
use crate::{
    cli_error,
    core::{
        document::{Clip, Document, TrackKind, color},
        error::Result,
    },
};
use image::{Rgba, RgbaImage, imageops};
use std::{collections::HashMap, path::Path};

#[derive(Default)]
pub struct Composer {
    decoders: HashMap<String, VideoWorker>,
    rasters: HashMap<String, RgbaImage>,
    text: TextRaster,
}

impl Composer {
    pub fn frame(&mut self, doc: &Document, base: &Path, frame: i64) -> Result<RgbaImage> {
        let s = &doc.settings;
        let mut output =
            RgbaImage::from_pixel(s.width, s.height, Rgba(color(&s.background).unwrap()));
        let mut active = Vec::new();
        for track in &doc.tracks {
            if track.kind == TrackKind::Audio {
                continue;
            }
            let mut layer = None;
            for transition in &doc.transitions {
                let from = doc.clip(&transition.from_clip).unwrap();
                if from.common().track_id != track.id {
                    continue;
                }
                let to = doc.clip(&transition.to_clip).unwrap();
                let start = to.common().timeline_start - transition.duration / 2;
                if frame < start || frame >= start + transition.duration {
                    continue;
                }
                active.push(from.common().id.clone());
                active.push(to.common().id.clone());
                let a = self.layer(doc, base, from, frame)?;
                let b = self.layer(doc, base, to, frame)?;
                layer = Some(effects::transition(
                    &a,
                    &b,
                    &transition.effect,
                    (frame - start) as f64 / transition.duration as f64,
                ));
                break;
            }
            if layer.is_none() {
                for clip in &doc.clips {
                    if clip.common().track_id == track.id
                        && frame >= clip.common().timeline_start
                        && frame < clip.end(s.frame_rate)
                    {
                        active.push(clip.common().id.clone());
                        layer = Some(self.layer(doc, base, clip, frame)?);
                        break;
                    }
                }
            }
            if let Some(layer) = layer {
                imageops::overlay(&mut output, &layer, 0, 0);
            }
        }
        self.decoders.retain(|id, _| active.contains(id));
        self.rasters.retain(|id, _| active.contains(id));
        Ok(output)
    }

    fn layer(&mut self, doc: &Document, base: &Path, clip: &Clip, frame: i64) -> Result<RgbaImage> {
        let c = clip.common();
        let s = &doc.settings;
        let source = match clip {
            Clip::Media {
                asset_id,
                source_in,
                ..
            } => {
                if !self.decoders.contains_key(&c.id) {
                    self.decoders.insert(
                        c.id.clone(),
                        VideoWorker::new(doc.asset_path(asset_id, base)?),
                    );
                }
                self.decoders[&c.id]
                    .at(s.frame_rate.seconds(source_in + frame - c.timeline_start))?
            }
            Clip::Image { asset_id, .. } => {
                if !self.rasters.contains_key(&c.id) {
                    self.rasters.insert(
                        c.id.clone(),
                        raster::load_image(&doc.asset_path(asset_id, base)?)?,
                    );
                }
                self.rasters[&c.id].clone()
            }
            Clip::Text { properties, .. } => {
                if !self.rasters.contains_key(&c.id) {
                    self.rasters.insert(
                        c.id.clone(),
                        self.text.raster(properties, s.width, s.height)?,
                    );
                }
                self.rasters[&c.id].clone()
            }
        };
        let source = effects::apply(source, &c.effects, c.opacity);
        let v = clip.video();
        let fit = (s.width as f64 / source.width() as f64)
            .min(s.height as f64 / source.height() as f64)
            * v.scale;
        let width = (source.width() as f64 * fit).round().max(1.0) as u32;
        let height = (source.height() as f64 * fit).round().max(1.0) as u32;
        if width as u64 * height as u64 > 268_435_456 {
            return Err(cli_error!(
                "raster_too_large",
                "",
                5,
                "transformed clip exceeds pixel allocation limit"
            ));
        }
        let source = imageops::resize(&source, width, height, imageops::FilterType::Triangle);
        let x = (v.position_x * s.width as f64 - width as f64 / 2.0).round() as i64;
        let y = (v.position_y * s.height as f64 - height as f64 / 2.0).round() as i64;
        let mut layer = RgbaImage::new(s.width, s.height);
        imageops::overlay(&mut layer, &source, x, y);
        Ok(layer)
    }
}
