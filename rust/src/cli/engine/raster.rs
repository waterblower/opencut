use crate::{
    cli::error::Result, cli_error, cli_try, timeline::TextClipProperties as TextProperties,
};
use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache, SwashContent};
use image::{Pixel, Rgba, RgbaImage};
use std::path::Path;

pub struct TextRaster {
    fonts: FontSystem,
    cache: SwashCache,
}

impl Default for TextRaster {
    fn default() -> Self {
        let mut db = cosmic_text::fontdb::Database::new();
        db.load_font_data(
            include_bytes!(
                "../../../vendor/zed/assets/fonts/ibm-plex-sans/IBMPlexSans-Regular.ttf"
            )
            .to_vec(),
        );
        db.set_sans_serif_family("IBM Plex Sans");
        Self {
            fonts: FontSystem::new_with_locale_and_db("en-US".into(), db),
            cache: SwashCache::new(),
        }
    }
}

impl TextRaster {
    pub fn raster(
        &mut self,
        properties: &TextProperties,
        width: u32,
        height: u32,
    ) -> Result<RgbaImage> {
        let size = properties.font_size as f32;
        let mut buffer = Buffer::new(&mut self.fonts, Metrics::new(size, size * 1.2));
        buffer.set_size(&mut self.fonts, Some(width as f32), Some(height as f32));
        let family = if matches!(properties.font.as_str(), "Sans" | "sans-serif" | "Inter") {
            Family::SansSerif
        } else {
            Family::Name(&properties.font)
        };
        buffer.set_text(
            &mut self.fonts,
            &properties.text,
            &Attrs::new().family(family),
            Shaping::Advanced,
        );
        buffer.shape_until_scroll(&mut self.fonts, false);
        let mut image = RgbaImage::new(width, height);
        let [alpha, red, green, blue] = properties.color.to_be_bytes();
        let base = [red, green, blue, alpha];
        let mut max_width = 0.0_f32;
        let mut max_height = 0.0_f32;
        for run in buffer.layout_runs() {
            max_width = max_width.max(run.line_w);
            max_height = max_height.max(run.line_top + run.line_height);
        }
        let left = (width as f32 * properties.position_x as f32 - max_width * 0.5)
            .clamp(0.0, (width as f32 - max_width).max(0.0))
            .round() as i32;
        let top = (height as f32 * properties.position_y as f32 - max_height * 0.5)
            .clamp(0.0, (height as f32 - max_height).max(0.0))
            .round() as i32;
        for run in buffer.layout_runs() {
            for glyph in run.glyphs {
                let physical = glyph.physical((0.0, 0.0), 1.0);
                let Some(bitmap) = self.cache.get_image(&mut self.fonts, physical.cache_key) else {
                    continue;
                };
                for y in 0..bitmap.placement.height {
                    for x in 0..bitmap.placement.width {
                        let px = left + physical.x + bitmap.placement.left + x as i32;
                        let py =
                            top + run.line_y as i32 + physical.y - bitmap.placement.top + y as i32;
                        if px < 0 || py < 0 || px >= width as i32 || py >= height as i32 {
                            continue;
                        }
                        let offset = (y * bitmap.placement.width + x) as usize;
                        let rgba = match bitmap.content {
                            SwashContent::Mask => [
                                base[0],
                                base[1],
                                base[2],
                                ((bitmap.data[offset] as u16 * base[3] as u16 + 127) / 255) as u8,
                            ],
                            SwashContent::Color => [
                                bitmap.data[offset * 4],
                                bitmap.data[offset * 4 + 1],
                                bitmap.data[offset * 4 + 2],
                                ((bitmap.data[offset * 4 + 3] as u16 * base[3] as u16 + 127) / 255)
                                    as u8,
                            ],
                            SwashContent::SubpixelMask => {
                                [base[0], base[1], base[2], bitmap.data[offset * 3]]
                            }
                        };
                        image.get_pixel_mut(px as u32, py as u32).blend(&Rgba(rgba));
                    }
                }
            }
        }
        Ok(image)
    }
}

pub fn load_image(path: &Path) -> Result<RgbaImage> {
    if path
        .extension()
        .unwrap_or_default()
        .eq_ignore_ascii_case("svg")
    {
        let options = resvg::usvg::Options {
            resources_dir: path.parent().map(Path::to_path_buf),
            ..Default::default()
        };
        let bytes = cli_try!(std::fs::read(path), "unreadable_media", "", 4);
        let tree = cli_try!(
            resvg::usvg::Tree::from_data(&bytes, &options),
            "unreadable_media",
            "",
            4
        );
        let size = tree.size().to_int_size();
        if size.width() > 16384 || size.height() > 16384 {
            return Err(cli_error!(
                "image_too_large",
                "",
                4,
                "SVG dimensions exceed 16384"
            ));
        }
        let Some(mut pixmap) = resvg::tiny_skia::Pixmap::new(size.width(), size.height()) else {
            return Err(cli_error!(
                "raster_failure",
                "",
                5,
                "cannot allocate SVG raster"
            ));
        };
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::default(),
            &mut pixmap.as_mut(),
        );
        let mut bytes = pixmap.take();
        for pixel in bytes.chunks_exact_mut(4) {
            let alpha = pixel[3] as u32;
            for channel in &mut pixel[..3] {
                *channel = (*channel as u32 * 255 + alpha / 2)
                    .checked_div(alpha)
                    .unwrap_or(0)
                    .min(255) as u8;
            }
        }
        return Ok(RgbaImage::from_raw(size.width(), size.height(), bytes).unwrap());
    }
    Ok(cli_try!(image::open(path), "unreadable_media", "", 4).to_rgba8())
}
