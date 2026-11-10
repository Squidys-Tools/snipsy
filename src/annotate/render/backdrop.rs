use super::{pixmap_to_rgba, rrect_path};
use crate::annotate::doc::*;
use image::{Rgba as IRgba, RgbaImage};
use tiny_skia::{FillRule, GradientStop, LinearGradient, Paint, Pixmap, Point, Shader, SpreadMode, Transform};

fn hex(c: u32) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8((c >> 16) as u8, (c >> 8) as u8, c as u8, 255)
}

pub(super) fn compose_backdrop(content: &Pixmap, bd: &Backdrop, trim: Option<Rect>) -> RgbaImage {
    let (w, h) = (content.width() as f32, content.height() as f32);
    let lay = layout(w, h, bd, trim);
    let (cw, ch) = (lay.canvas_w as u32, lay.canvas_h as u32);
    let mut out = Pixmap::new(cw.max(1), ch.max(1)).unwrap();

    let (angle, c0, c1) = BACKGROUNDS[bd.preset % BACKGROUNDS.len()];
    // Mirrors gpui's linear_gradient shader (shaders.hlsl gradient_color) so the
    // export matches the editor preview: scale the short-axis component of the
    // direction by the aspect ratio, then normalize t by the dominant axis.
    let rad = ((angle % 360.0) - 90.0).to_radians();
    let (mut dx, mut dy) = (rad.cos(), rad.sin());
    let (fw, fh) = (cw as f32, ch as f32);
    if fw > fh {
        dy *= fh / fw;
    } else {
        dx *= fw / fh;
    }
    let len = (dx * dx + dy * dy).sqrt().max(f32::EPSILON);
    let (ux, uy) = (dx / len, dy / len);
    let half = if dx.abs() > dy.abs() { fw / 2.0 } else { fh / 2.0 };
    let (mx, my) = (fw / 2.0, fh / 2.0);
    let shader = LinearGradient::new(
        Point::from_xy(mx - ux * half, my - uy * half),
        Point::from_xy(mx + ux * half, my + uy * half),
        vec![GradientStop::new(0.0, hex(c0)), GradientStop::new(1.0, hex(c1))],
        SpreadMode::Pad,
        Transform::identity(),
    )
    .unwrap_or(Shader::SolidColor(hex(c0)));
    let mut bgp = Paint::default();
    bgp.shader = shader;
    out.fill_rect(tiny_skia::Rect::from_xywh(0.0, 0.0, cw as f32, ch as f32).unwrap(), &bgp, Transform::identity(), None);

    let src = lay.src;
    let dst = Rect { x0: lay.img_x, y0: lay.img_y, x1: lay.img_x + src.w(), y1: lay.img_y + src.h() };

    if bd.shadow {
        let blur_r = (src.w().min(src.h()) * 0.04).clamp(8.0, 48.0);
        let s = 4.0;
        let (sw, shh) = ((cw as f32 / s).ceil() as u32, (ch as f32 / s).ceil() as u32);
        let mut mask = image::GrayImage::new(sw.max(1), shh.max(1));
        let off = blur_r * 0.35;
        for y in 0..mask.height() {
            for x in 0..mask.width() {
                let (fx, fy) = (x as f32 * s, y as f32 * s);
                if fx >= dst.x0 && fx <= dst.x1 && fy >= dst.y0 + off && fy <= dst.y1 + off {
                    mask.put_pixel(x, y, image::Luma([255]));
                }
            }
        }
        let mask = image::imageops::fast_blur(&mask, blur_r / s);
        let data = out.data_mut();
        for y in 0..ch {
            for x in 0..cw {
                let m = mask.get_pixel(((x as f32 / s) as u32).min(mask.width() - 1), ((y as f32 / s) as u32).min(mask.height() - 1))[0];
                let a = m as f32 / 255.0 * 0.45;
                let i = ((y * cw + x) * 4) as usize;
                for c in 0..3 {
                    data[i + c] = (data[i + c] as f32 * (1.0 - a)) as u8;
                }
            }
        }
    }

    let mut mask = tiny_skia::Mask::new(cw, ch).unwrap();
    if let Some(path) = rrect_path(dst, bd.radius) {
        mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
    }
    let pat = tiny_skia::Pattern::new(
        content.as_ref(),
        SpreadMode::Pad,
        tiny_skia::FilterQuality::Nearest,
        1.0,
        Transform::from_translate(dst.x0 - src.x0, dst.y0 - src.y0),
    );
    let mut ip = Paint::default();
    ip.shader = pat;
    out.fill_rect(tiny_skia::Rect::from_ltrb(dst.x0, dst.y0, dst.x1, dst.y1).unwrap(), &ip, Transform::identity(), Some(&mask));
    pixmap_to_rgba(&out)
}

pub fn window_shadow(img: &RgbaImage, scale: f32) -> RgbaImage {
    let pad = (40.0 * scale).round() as u32;
    let off = (10.0 * scale).round() as i64;
    let (w, h) = (img.width() + pad * 2, img.height() + pad * 2);
    let s = 4u32;
    let mut mask = image::GrayImage::new(w / s + 1, h / s + 1);
    for y in 0..img.height() / s {
        for x in 0..img.width() / s {
            let a = img.get_pixel((x * s).min(img.width() - 1), (y * s).min(img.height() - 1))[3];
            let my = (y as i64 + (pad as i64 + off) / s as i64) as u32;
            let mx = x + pad / s;
            if mx < mask.width() && my < mask.height() {
                mask.put_pixel(mx, my, image::Luma([a]));
            }
        }
    }
    let mask = image::imageops::fast_blur(&mask, 16.0 * scale / s as f32);
    let mut out = RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let m = mask.get_pixel((x / s).min(mask.width() - 1), (y / s).min(mask.height() - 1))[0];
            out.put_pixel(x, y, IRgba([0, 0, 0, (m as f32 * 0.5) as u8]));
        }
    }
    image::imageops::overlay(&mut out, img, pad as i64, pad as i64);
    out
}
