//! CPU rasterisation of a [`Doc`] to a final RGBA image (tiny-skia + ab_glyph).
//! Also hosts the blur/pixelate effects used by the live editor preview.

use super::doc::*;
use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use image::{Rgba as IRgba, RgbaImage};
use std::sync::OnceLock;
use tiny_skia::{
    FillRule, GradientStop, LineCap, LineJoin, LinearGradient, Paint, PathBuilder, Pixmap, Point,
    Shader, SpreadMode, Stroke, Transform,
};

// ---------------------------------------------------------------- fonts

pub fn font_regular() -> Option<&'static FontArc> {
    static F: OnceLock<Option<FontArc>> = OnceLock::new();
    F.get_or_init(|| load_font(&["segoeui.ttf", "arial.ttf"])).as_ref()
}

pub fn font_bold() -> Option<&'static FontArc> {
    static F: OnceLock<Option<FontArc>> = OnceLock::new();
    F.get_or_init(|| load_font(&["segoeuib.ttf", "arialbd.ttf", "segoeui.ttf"])).as_ref()
}

/// Fonts consulted, in order, for characters the primary font lacks
/// (symbols, emoji outlines, CJK, Korean).
fn fallback_fonts() -> &'static [FontArc] {
    static F: OnceLock<Vec<FontArc>> = OnceLock::new();
    F.get_or_init(|| {
        ["seguisym.ttf", "seguiemj.ttf", "msyh.ttc", "YuGothM.ttc", "malgun.ttf", "Nirmala.ttc"]
            .iter()
            .filter_map(|n| load_font(&[n]))
            .collect()
    })
}

/// The font that should draw `ch`: the primary one if it has the glyph.
fn font_for(primary: &'static FontArc, ch: char) -> &'static FontArc {
    if ch.is_whitespace() || primary.glyph_id(ch).0 != 0 {
        return primary;
    }
    fallback_fonts().iter().find(|f| f.glyph_id(ch).0 != 0).unwrap_or(primary)
}

fn load_font(names: &[&str]) -> Option<FontArc> {
    let dir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
    names.iter().find_map(|n| {
        let data = std::fs::read(format!("{dir}\\Fonts\\{n}")).ok()?;
        if n.ends_with(".ttc") {
            ab_glyph::FontVec::try_from_vec_and_index(data, 0).ok().map(FontArc::new)
        } else {
            FontArc::try_from_vec(data).ok()
        }
    })
}

// ---------------------------------------------------------------- effects

/// Apply all blur / pixelate annotations directly to a copy of the base.
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
    // downscale + upscale is a cheap, strong low-pass
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

// ---------------------------------------------------------------- render

fn paint_of(c: Rgba) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c[0], c[1], c[2], c[3]);
    p.anti_alias = true;
    p
}

fn rgba_to_pixmap(img: &RgbaImage) -> Pixmap {
    let mut pm = Pixmap::new(img.width(), img.height()).unwrap();
    let dst = pm.data_mut();
    for (d, s) in dst.chunks_exact_mut(4).zip(img.as_raw().chunks_exact(4)) {
        let a = s[3] as u16;
        d[0] = ((s[0] as u16 * a + 127) / 255) as u8;
        d[1] = ((s[1] as u16 * a + 127) / 255) as u8;
        d[2] = ((s[2] as u16 * a + 127) / 255) as u8;
        d[3] = s[3];
    }
    pm
}

fn pixmap_to_rgba(pm: &Pixmap) -> RgbaImage {
    let mut out = vec![0u8; pm.data().len()];
    for (d, s) in out.chunks_exact_mut(4).zip(pm.data().chunks_exact(4)) {
        let a = s[3] as u32;
        if a == 0 {
            continue;
        }
        d[0] = ((s[0] as u32 * 255 + a / 2) / a).min(255) as u8;
        d[1] = ((s[1] as u32 * 255 + a / 2) / a).min(255) as u8;
        d[2] = ((s[2] as u32 * 255 + a / 2) / a).min(255) as u8;
        d[3] = s[3];
    }
    RgbaImage::from_raw(pm.width(), pm.height(), out).unwrap()
}

fn ellipse_path(r: Rect) -> Option<tiny_skia::Path> {
    tiny_skia::Rect::from_ltrb(r.x0, r.y0, r.x1.max(r.x0 + 1.0), r.y1.max(r.y0 + 1.0))
        .and_then(PathBuilder::from_oval)
}

fn rrect_path(r: Rect, rad: f32) -> Option<tiny_skia::Path> {
    let rad = rad.min(r.w() / 2.0).min(r.h() / 2.0).max(0.0);
    let k = 0.5523 * rad;
    let mut pb = PathBuilder::new();
    pb.move_to(r.x0 + rad, r.y0);
    pb.line_to(r.x1 - rad, r.y0);
    pb.cubic_to(r.x1 - rad + k, r.y0, r.x1, r.y0 + rad - k, r.x1, r.y0 + rad);
    pb.line_to(r.x1, r.y1 - rad);
    pb.cubic_to(r.x1, r.y1 - rad + k, r.x1 - rad + k, r.y1, r.x1 - rad, r.y1);
    pb.line_to(r.x0 + rad, r.y1);
    pb.cubic_to(r.x0 + rad - k, r.y1, r.x0, r.y1 - rad + k, r.x0, r.y1 - rad);
    pb.line_to(r.x0, r.y0 + rad);
    pb.cubic_to(r.x0, r.y0 + rad - k, r.x0 + rad - k, r.y0, r.x0 + rad, r.y0);
    pb.close();
    pb.finish()
}

fn polyline(pts: &[Pt]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    let first = pts.first()?;
    pb.move_to(first.x, first.y);
    if pts.len() == 1 {
        pb.line_to(first.x + 0.1, first.y + 0.1);
    }
    for p in &pts[1..] {
        pb.line_to(p.x, p.y);
    }
    pb.finish()
}

fn polygon(pts: &[Pt]) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    let first = pts.first()?;
    pb.move_to(first.x, first.y);
    for p in &pts[1..] {
        pb.line_to(p.x, p.y);
    }
    pb.close();
    pb.finish()
}

fn stroke(w: f32, cap: LineCap) -> Stroke {
    Stroke { width: w, line_cap: cap, line_join: LineJoin::Round, ..Default::default() }
}

/// Draw a soft drop shadow beneath vector annotations (CleanShot adds a subtle one).
fn shadow_paint() -> Paint<'static> {
    paint_of([0, 0, 0, 60])
}

pub fn draw_annotation(pm: &mut Pixmap, a: &Annotation, t: Transform) {
    let col = paint_of(a.color);
    let w = a.width;
    let sh = Transform::from_translate(0.0, (w * 0.35).max(1.0)).post_concat(t);
    match &a.shape {
        Shape::Arrow { a: p0, b: p1 } => {
            if let Some(path) = polygon(&arrow_polygon(*p0, *p1, w)) {
                pm.fill_path(&path, &shadow_paint(), FillRule::Winding, sh, None);
                pm.fill_path(&path, &col, FillRule::Winding, t, None);
                pm.stroke_path(&path, &col, &stroke(w * 0.35, LineCap::Round), t, None);
            }
        }
        Shape::Line { a: p0, b: p1 } => {
            if let Some(path) = polyline(&[*p0, *p1]) {
                pm.stroke_path(&path, &shadow_paint(), &stroke(w, LineCap::Round), sh, None);
                pm.stroke_path(&path, &col, &stroke(w, LineCap::Round), t, None);
            }
        }
        Shape::Rect(r) => {
            if let Some(path) = rrect_path(*r, w.min(6.0)) {
                pm.stroke_path(&path, &shadow_paint(), &stroke(w, LineCap::Round), sh, None);
                pm.stroke_path(&path, &col, &stroke(w, LineCap::Round), t, None);
            }
        }
        Shape::FilledRect(r) => {
            if let Some(path) = rrect_path(*r, w.min(6.0)) {
                pm.fill_path(&path, &col, FillRule::Winding, t, None);
            }
        }
        Shape::Ellipse(r) => {
            if let Some(path) = ellipse_path(*r) {
                pm.stroke_path(&path, &shadow_paint(), &stroke(w, LineCap::Round), sh, None);
                pm.stroke_path(&path, &col, &stroke(w, LineCap::Round), t, None);
            }
        }
        Shape::Pencil(pts) => {
            if let Some(path) = polyline(pts) {
                pm.stroke_path(&path, &shadow_paint(), &stroke(w, LineCap::Round), sh, None);
                pm.stroke_path(&path, &col, &stroke(w, LineCap::Round), t, None);
            }
        }
        Shape::Highlighter(pts) => {
            if let Some(path) = polyline(pts) {
                // Plain source-over at ~43% alpha, matching the editor preview.
                let p = paint_of([a.color[0], a.color[1], a.color[2], 110]);
                pm.stroke_path(&path, &p, &stroke(a.highlighter_width(), LineCap::Square), t, None);
            }
        }
        Shape::Counter { c, n } => {
            let r = a.counter_radius();
            let rect = Rect { x0: c.x - r, y0: c.y - r, x1: c.x + r, y1: c.y + r };
            if let Some(path) = ellipse_path(rect) {
                pm.fill_path(&path, &shadow_paint(), FillRule::Winding, sh, None);
                pm.fill_path(&path, &col, FillRule::Winding, t, None);
                pm.stroke_path(&path, &paint_of([255, 255, 255, 255]), &stroke((r * 0.12).max(1.5), LineCap::Round), t, None);
            }
            let fg = contrast_on(a.color);
            draw_text_line(pm, &n.to_string(), r * 1.15, *c, fg, true, t, true);
        }
        Shape::Text { pos, text, style } => draw_text(pm, a, *pos, text, *style, t),
        Shape::Blur(_) | Shape::Pixelate(_) | Shape::Spotlight(_) => {}
    }
}

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
fn draw_text_line(pm: &mut Pixmap, text: &str, size: f32, pos: Pt, color: Rgba, bold: bool, t: Transform, centered: bool) {
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

fn draw_text(pm: &mut Pixmap, a: &Annotation, pos: Pt, text: &str, style: TextStyle, t: Transform) {
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

fn hex(c: u32) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8((c >> 16) as u8, (c >> 8) as u8, c as u8, 255)
}

/// Full export: effects, spotlight, annotations, then backdrop composition.
pub fn render(doc: &Doc) -> RgbaImage {
    let base = apply_effects(&doc.base, &doc.annotations);
    let (w, h) = (base.width() as f32, base.height() as f32);
    let mut pm = rgba_to_pixmap(&base);

    let spots: Vec<Rect> = doc
        .annotations
        .iter()
        .filter_map(|a| if let Shape::Spotlight(r) = a.shape { Some(r) } else { None })
        .collect();
    for c in spotlight_dim_cells(w, h, &spots) {
        if let Some(r) = tiny_skia::Rect::from_ltrb(c.x0, c.y0, c.x1, c.y1) {
            pm.fill_rect(r, &paint_of([0, 0, 0, 128]), Transform::identity(), None);
        }
    }
    for a in &doc.annotations {
        draw_annotation(&mut pm, a, Transform::identity());
    }

    if !doc.backdrop.enabled {
        return pixmap_to_rgba(&pm);
    }
    compose_backdrop(&pm, &doc.backdrop, doc.trim())
}

fn compose_backdrop(content: &Pixmap, bd: &Backdrop, trim: Option<Rect>) -> RgbaImage {
    let (w, h) = (content.width() as f32, content.height() as f32);
    let lay = layout(w, h, bd, trim);
    let (cw, ch) = (lay.canvas_w as u32, lay.canvas_h as u32);
    let mut out = Pixmap::new(cw.max(1), ch.max(1)).unwrap();

    // gradient background
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

    // soft shadow: blur an alpha mask on a downscaled buffer
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

    // screenshot, clipped to a rounded rect
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

/// Transparent PNG of a window with a soft macOS-like drop shadow.
pub fn window_shadow(img: &RgbaImage, scale: f32) -> RgbaImage {
    let pad = (40.0 * scale).round() as u32;
    let off = (10.0 * scale).round() as i64;
    let (w, h) = (img.width() + pad * 2, img.height() + pad * 2);
    // alpha mask of the window, blurred at quarter resolution
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
