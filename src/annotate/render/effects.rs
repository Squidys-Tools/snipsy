use crate::annotate::doc::*;
use image::{Rgba as IRgba, RgbaImage};

pub fn apply_effects(base: &RgbaImage, anns: &[Annotation]) -> RgbaImage {
    let mut img = base.clone();
    for a in anns {
        match a.shape {
            Shape::Blur(r) => blur_region(&mut img, r),
            Shape::Pixelate(r) => pixelate_region(&mut img, r),
            _ => {}
        }
    }
    img
}

fn region(img: &RgbaImage, r: Rect) -> Option<(u32, u32, u32, u32)> {
    let r = r.clamp_to(img.width() as f32, img.height() as f32);
    let (x, y) = (r.x0.floor() as u32, r.y0.floor() as u32);
    let (w, h) = ((r.x1.ceil() as u32).saturating_sub(x), (r.y1.ceil() as u32).saturating_sub(y));
    (w >= 2 && h >= 2).then_some((x, y, w, h))
}

/// "Secure" blur: pixelate first (destroys detail), then smooth heavily.
pub fn blur_region(img: &mut RgbaImage, r: Rect) {
    let Some((x, y, w, h)) = region(img, r) else { return };
    let sub = image::imageops::crop_imm(img, x, y, w, h).to_image();
    let f = 6u32;
    let small = image::imageops::resize(&sub, (w / f).max(1), (h / f).max(1), image::imageops::FilterType::Triangle);
    let up = image::imageops::resize(&small, w, h, image::imageops::FilterType::Triangle);
    let blurred = image::imageops::fast_blur(&up, (w.min(h) as f32 / 18.0).clamp(4.0, 18.0));
    image::imageops::replace(img, &blurred, x as i64, y as i64);
}

/// Randomised pixelation (block colors jittered so the original can't be recovered).
pub fn pixelate_region(img: &mut RgbaImage, r: Rect) {
    let Some((x0, y0, w, h)) = region(img, r) else { return };
    let block = (w.min(h) / 8).clamp(8, 28);
    let mut seed = (x0 * 7919 + y0 * 104729 + w * 31 + h) | 1;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        (seed % 17) as i32 - 8
    };
    let mut by = y0;
    while by < y0 + h {
        let bh = block.min(y0 + h - by);
        let mut bx = x0;
        while bx < x0 + w {
            let bw = block.min(x0 + w - bx);
            let (mut s, mut n) = ([0u64; 4], 0u64);
            for yy in by..by + bh {
                for xx in bx..bx + bw {
                    let p = img.get_pixel(xx, yy);
                    for i in 0..4 {
                        s[i] += p[i] as u64;
                    }
                    n += 1;
                }
            }
            let j = rnd();
            let c = IRgba([
                ((s[0] / n) as i32 + j).clamp(0, 255) as u8,
                ((s[1] / n) as i32 + j).clamp(0, 255) as u8,
                ((s[2] / n) as i32 + j).clamp(0, 255) as u8,
                (s[3] / n) as u8,
            ]);
            for yy in by..by + bh {
                for xx in bx..bx + bw {
                    img.put_pixel(xx, yy, c);
                }
            }
            bx += block;
        }
        by += block;
    }
}
