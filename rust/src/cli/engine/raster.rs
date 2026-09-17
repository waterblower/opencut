use anyhow::{Context as _, Result, anyhow};
use image::RgbaImage;
use std::path::Path;

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
        let bytes =
            std::fs::read(path).context(format!("unreadable_media at {}:{}", file!(), line!()))?;
        let tree = resvg::usvg::Tree::from_data(&bytes, &options).context(format!(
            "unreadable_media at {}:{}",
            file!(),
            line!()
        ))?;
        let size = tree.size().to_int_size();
        if size.width() > 16384 || size.height() > 16384 {
            return Err(anyhow!(
                "image_too_large: SVG dimensions exceed 16384 at {}:{}",
                file!(),
                line!()
            ));
        }
        let Some(mut pixmap) = resvg::tiny_skia::Pixmap::new(size.width(), size.height()) else {
            return Err(anyhow!(
                "raster_failure: cannot allocate SVG raster at {}:{}",
                file!(),
                line!()
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
    Ok(image::open(path)
        .context(format!("unreadable_media at {}:{}", file!(), line!()))?
        .to_rgba8())
}
