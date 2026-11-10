use super::fonts::{font_bold, font_for, font_regular};
use super::{paint_of, rrect_path, shadow_paint};
use crate::annotate::doc::*;
use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use tiny_skia::{FillRule, Pixmap, Transform};

pub fn contrast_on(c: Rgba) -> Rgba {
    let l = 0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32;
    if l > 170.0 { [20, 20, 22, 255] } else { [255, 255, 255, 255] }
}

/// Measure a single line with the bold font, returns (width, ascent, descent).
pub fn measure(text: &str, size: f32) -> (f32, f32, f32) {
    let Some(font) = font_bold() else { return (text.len() as f32 * size * 0.55, size * 0.8, size * 0.2) };
    let sf = font.as_scaled(px_scale(font, size));
    let mut w = 0.0;
    let mut prev = None;
    for ch in text.chars() {
        let f = font_for(font, ch);
        if !std::ptr::eq(f, font) {
            let fs = f.as_scaled(px_scale(f, size));
            w += fs.h_advance(fs.glyph_id(ch));
            prev = None;
            continue;
        }
        let id = sf.glyph_id(ch);
        if let Some(p) = prev {
            w += sf.kern(p, id);
        }
        w += sf.h_advance(id);
        prev = Some(id);
    }
    (w, sf.ascent(), -sf.descent())
}

fn px_scale(font: &FontArc, size: f32) -> PxScale {
    // convert CSS-like font size (em) to ab_glyph's height-based scale
    let upm = font.units_per_em().unwrap_or(2048.0);
    PxScale::from(size * font.height_unscaled() / upm)
}

/// Draw one line of text. `pos` is the top-left (or centre when `centered`).
#[allow(clippy::too_many_arguments)]
pub(super) fn draw_text_line(pm: &mut Pixmap, text: &str, size: f32, pos: Pt, color: Rgba, bold: bool, t: Transform, centered: bool) {
    let font = if bold { font_bold() } else { font_regular() };
    let Some(font) = font else { return };
    let scale = px_scale(font, size);
    let sf = font.as_scaled(scale);
    let (tw, asc, desc) = measure(text, size);
    let (ox, baseline) = if centered {
        (pos.x - tw / 2.0, pos.y + (asc - desc) / 2.0)
    } else {
        let line_h = size * TEXT_LINE_HEIGHT;
        (pos.x, pos.y + (line_h - (asc + desc)) / 2.0 + asc)
    };
    let (tx, ty) = (t.tx, t.ty);
    let mut x = ox;
    let mut prev = None;
    let (pw, ph) = (pm.width() as i32, pm.height() as i32);
    let data = pm.data_mut();
    for ch in text.chars() {
        let f = font_for(font, ch);
        let fallback = !std::ptr::eq(f, font);
        let (fscale, fsf) = if fallback { let s = px_scale(f, size); (s, f.as_scaled(s)) } else { (scale, sf) };
        let id = fsf.glyph_id(ch);
        if let (Some(p), false) = (prev, fallback) {
            x += sf.kern(p, id);
        }
        let g = id.with_scale_and_position(fscale, ab_glyph::point(x + tx, baseline + ty));
        if let Some(og) = f.outline_glyph(g) {
            let b = og.px_bounds();
            og.draw(|gx, gy, cov| {
                let px = b.min.x as i32 + gx as i32;
                let py = b.min.y as i32 + gy as i32;
                if px < 0 || py < 0 || px >= pw || py >= ph {
                    return;
                }
                let i = ((py * pw + px) * 4) as usize;
                let a = cov.clamp(0.0, 1.0) * color[3] as f32 / 255.0;
                // premultiplied src-over
                for c in 0..3 {
                    let s = color[c] as f32 * a;
                    data[i + c] = (s + data[i + c] as f32 * (1.0 - a)).round().min(255.0) as u8;
                }
                data[i + 3] = (a * 255.0 + data[i + 3] as f32 * (1.0 - a)).round().min(255.0) as u8;
            });
        }
        x += fsf.h_advance(id);
        prev = if fallback { None } else { Some(id) };
    }
}

pub fn text_block_size(text: &str, size: f32) -> (f32, f32) {
    let lines: Vec<&str> = text.split('\n').collect();
    let w = lines.iter().map(|l| measure(l, size).0).fold(0.0, f32::max);
    (w, lines.len() as f32 * size * TEXT_LINE_HEIGHT)
}

pub(super) fn draw_text(pm: &mut Pixmap, a: &Annotation, pos: Pt, text: &str, style: TextStyle, t: Transform) {
    let size = a.text_size();
    let (bw, bh) = text_block_size(text, size);
    let pad = size * 0.35;
    let mut fg = a.color;
    match style {
        TextStyle::Pill | TextStyle::Card => {
            let bg = if style == TextStyle::Pill { a.color } else { [255, 255, 255, 255] };
            if style == TextStyle::Pill {
                fg = contrast_on(a.color);
            } else if a.color == [255, 255, 255, 255] {
                fg = [28, 28, 30, 255];
            }
            let r = Rect { x0: pos.x - pad, y0: pos.y - pad * 0.5, x1: pos.x + bw + pad, y1: pos.y + bh + pad * 0.5 };
            if let Some(path) = rrect_path(r, size * 0.3) {
                let sh = Transform::from_translate(0.0, 2.0).post_concat(t);
                pm.fill_path(&path, &shadow_paint(), FillRule::Winding, sh, None);
                pm.fill_path(&path, &paint_of(bg), FillRule::Winding, t, None);
            }
        }
        TextStyle::Outline => {
            let oc = contrast_on(fg);
            let o = (size * 0.06).max(1.0);
            for (dx, dy) in [(-o, 0.0), (o, 0.0), (0.0, -o), (0.0, o), (-o, -o), (o, o), (-o, o), (o, -o)] {
                for (i, line) in text.split('\n').enumerate() {
                    let p = pt(pos.x + dx, pos.y + dy + i as f32 * size * TEXT_LINE_HEIGHT);
                    draw_text_line(pm, line, size, p, [oc[0], oc[1], oc[2], 255], true, t, false);
                }
            }
        }
        TextStyle::Plain => {}
    }
    for (i, line) in text.split('\n').enumerate() {
        let p = pt(pos.x, pos.y + i as f32 * size * TEXT_LINE_HEIGHT);
        draw_text_line(pm, line, size, p, fg, true, t, false);
    }
}
