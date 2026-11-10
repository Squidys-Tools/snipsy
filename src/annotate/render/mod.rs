mod backdrop;
mod effects;
mod fonts;
mod text;

pub use backdrop::window_shadow;
pub use effects::apply_effects;
pub use text::contrast_on;

use super::doc::*;
use backdrop::compose_backdrop;
use image::RgbaImage;
use text::{draw_text, draw_text_line};
use tiny_skia::{FillRule, LineCap, LineJoin, Paint, PathBuilder, Pixmap, Stroke, Transform};

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
