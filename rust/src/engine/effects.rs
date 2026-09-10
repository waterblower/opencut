use crate::core::document::{Direction, Effect, TransitionEffect, color};
use image::{Rgba, RgbaImage, imageops};

pub fn apply(mut image: RgbaImage, effects: &[Effect], opacity: f64) -> RgbaImage {
    for effect in effects {
        match *effect {
            Effect::GaussianBlur { radius } => {
                if radius > 0.0 {
                    image = imageops::blur(&image, radius as f32);
                }
            }
            Effect::ColorAdjust {
                brightness,
                contrast,
                saturation,
            } => {
                for p in image.pixels_mut() {
                    let gray = 0.2126 * p[0] as f64 + 0.7152 * p[1] as f64 + 0.0722 * p[2] as f64;
                    for c in &mut p.0[..3] {
                        *c = (((gray + (*c as f64 - gray) * saturation - 127.5) * contrast + 127.5)
                            + brightness * 255.0)
                            .round()
                            .clamp(0.0, 255.0) as u8;
                    }
                }
            }
            Effect::Crop {
                x,
                y,
                width,
                height,
            } => {
                let w = image.width();
                let h = image.height();
                let x = (x * w as f64).floor() as u32;
                let y = (y * h as f64).floor() as u32;
                let cw = ((width * w as f64).round() as u32).max(1).min(w - x);
                let ch = ((height * h as f64).round() as u32).max(1).min(h - y);
                image = imageops::crop_imm(&image, x, y, cw, ch).to_image();
            }
            Effect::Flip {
                horizontal,
                vertical,
            } => {
                if horizontal {
                    imageops::flip_horizontal_in_place(&mut image);
                }
                if vertical {
                    imageops::flip_vertical_in_place(&mut image);
                }
            }
        }
    }
    if opacity != 1.0 {
        for p in image.pixels_mut() {
            p[3] = (p[3] as f64 * opacity).round() as u8;
        }
    }
    image
}

pub fn transition(
    from: &RgbaImage,
    to: &RgbaImage,
    effect: &TransitionEffect,
    progress: f64,
) -> RgbaImage {
    let w = from.width();
    let h = from.height();
    let mut output = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let a = *from.get_pixel(x, y);
            let b = *to.get_pixel(x, y);
            let pixel = match effect {
                TransitionEffect::Crossfade => mix(a, b, progress),
                TransitionEffect::DipToColor { color: c } => {
                    let color = Rgba(color(c).unwrap());
                    if progress < 0.5 {
                        mix(a, color, progress * 2.0)
                    } else {
                        mix(color, b, progress * 2.0 - 1.0)
                    }
                }
                TransitionEffect::Wipe { direction } => {
                    let coordinate = match direction {
                        Direction::Left => 1.0 - (x as f64 + 0.5) / w as f64,
                        Direction::Right => (x as f64 + 0.5) / w as f64,
                        Direction::Up => 1.0 - (y as f64 + 0.5) / h as f64,
                        Direction::Down => (y as f64 + 0.5) / h as f64,
                    };
                    if coordinate < progress { b } else { a }
                }
                TransitionEffect::Slide { direction } => {
                    let (dx, dy) = match direction {
                        Direction::Left => (-1.0, 0.0),
                        Direction::Right => (1.0, 0.0),
                        Direction::Up => (0.0, -1.0),
                        Direction::Down => (0.0, 1.0),
                    };
                    let ax = x as i64 - (dx * progress * w as f64).round() as i64;
                    let ay = y as i64 - (dy * progress * h as f64).round() as i64;
                    let bx = x as i64 + (dx * (1.0 - progress) * w as f64).round() as i64;
                    let by = y as i64 + (dy * (1.0 - progress) * h as f64).round() as i64;
                    if ax >= 0 && ay >= 0 && ax < w as i64 && ay < h as i64 {
                        *from.get_pixel(ax as u32, ay as u32)
                    } else if bx >= 0 && by >= 0 && bx < w as i64 && by < h as i64 {
                        *to.get_pixel(bx as u32, by as u32)
                    } else {
                        Rgba([0, 0, 0, 0])
                    }
                }
            };
            output.put_pixel(x, y, pixel);
        }
    }
    output
}

fn mix(a: Rgba<u8>, b: Rgba<u8>, t: f64) -> Rgba<u8> {
    let alpha = a[3] as f64 * (1.0 - t) + b[3] as f64 * t;
    let mut result = [0, 0, 0, alpha.round() as u8];
    if alpha > 0.0 {
        for c in 0..3 {
            result[c] = ((a[c] as f64 * a[3] as f64 * (1.0 - t) + b[c] as f64 * b[3] as f64 * t)
                / alpha)
                .round() as u8;
        }
    }
    Rgba(result)
}
