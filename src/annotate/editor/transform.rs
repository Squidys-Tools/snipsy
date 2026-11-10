use crate::annotate::doc::*;
use std::sync::Arc;
use super::Editor;

impl Editor {
    pub(super) fn reset_crop(&mut self) {
        if self.tool == Tool::Crop {
            let (w, h) = (self.doc.base.width() as f32, self.doc.base.height() as f32);
            self.crop = Some(self.aspect_crop_rect(w, h));
        }
    }

    fn aspect_crop_rect(&self, w: f32, h: f32) -> Rect {
        let Some(r) = self.crop_aspect.ratio() else { return Rect { x0: 0.0, y0: 0.0, x1: w, y1: h } };
        let (cw, ch) = if w / h > r { (h * r, h) } else { (w, w / r) };
        let (x0, y0) = (((w - cw) / 2.0).floor(), ((h - ch) / 2.0).floor());
        Rect { x0, y0, x1: x0 + cw.floor(), y1: y0 + ch.floor() }
    }

    pub(super) fn set_crop_aspect(&mut self, a: Aspect) {
        self.crop_aspect = a;
        self.reset_crop();
    }

    fn resize(&mut self, w: u32, h: u32) {
        let (ow, oh) = (self.doc.base.width(), self.doc.base.height());
        let (w, h) = (w.clamp(8, 16384), h.clamp(8, 16384));
        if (w, h) == (ow, oh) {
            return;
        }
        self.commit_text();
        self.push_undo();
        let (sx, sy) = (w as f32 / ow as f32, h as f32 / oh as f32);
        self.doc.base = Arc::new(image::imageops::resize(&*self.doc.base, w, h, image::imageops::FilterType::CatmullRom));
        for a in &mut self.doc.annotations {
            a.scale(sx, sy);
        }
        self.reset_crop();
    }

    /// Uniform resize by `f`, limited so both sides stay within [8, 16384]
    /// (never distorting the aspect ratio).
    pub(super) fn resize_by(&mut self, f: f32) {
        let (w, h) = (self.doc.base.width() as f32, self.doc.base.height() as f32);
        let (lo, hi) = ((8.0 / w).max(8.0 / h), (16384.0 / w).min(16384.0 / h));
        if lo > hi {
            return;
        }
        let f = f.clamp(lo, hi);
        self.resize((w * f).round() as u32, (h * f).round() as u32);
    }

    pub(super) fn resize_fit(&mut self, max: u32) {
        let (w, h) = (self.doc.base.width(), self.doc.base.height());
        let long = w.max(h);
        if long > max {
            self.resize_by(max as f32 / long as f32);
        }
    }

    pub(super) fn apply_crop(&mut self) {
        let Some(r) = self.crop.take() else { return };
        let (w, h) = (self.doc.base.width() as f32, self.doc.base.height() as f32);
        let r = r.clamp_to(w, h);
        if r.w() < 4.0 || r.h() < 4.0 || (r.x0 == 0.0 && r.y0 == 0.0 && r.x1 == w && r.y1 == h) {
            self.tool = Tool::Arrow;
            return;
        }
        self.push_undo();
        let img = image::imageops::crop_imm(&*self.doc.base, r.x0 as u32, r.y0 as u32, r.w() as u32, r.h() as u32).to_image();
        self.doc.base = Arc::new(img);
        for a in &mut self.doc.annotations {
            a.translate(-r.x0.floor(), -r.y0.floor());
        }
        self.tool = Tool::Arrow;
    }

    pub(super) fn rotate(&mut self, clockwise: bool) {
        self.commit_text();
        self.push_undo();
        let (w, h) = (self.doc.base.width() as f32, self.doc.base.height() as f32);
        let img = if clockwise { image::imageops::rotate90(&*self.doc.base) } else { image::imageops::rotate270(&*self.doc.base) };
        self.doc.base = Arc::new(img);
        let map = |p: &mut Pt| {
            let (x, y) = (p.x, p.y);
            if clockwise {
                *p = pt(h - y, x);
            } else {
                *p = pt(y, w - x);
            }
        };
        for a in &mut self.doc.annotations {
            match &mut a.shape {
                Shape::Arrow { a, b } | Shape::Line { a, b } => {
                    map(a);
                    map(b);
                }
                Shape::Rect(r) | Shape::FilledRect(r) | Shape::Ellipse(r) | Shape::Blur(r) | Shape::Pixelate(r) | Shape::Spotlight(r) => {
                    let (mut p0, mut p1) = (pt(r.x0, r.y0), pt(r.x1, r.y1));
                    map(&mut p0);
                    map(&mut p1);
                    *r = Rect::from_pts(p0, p1);
                }
                Shape::Pencil(ps) | Shape::Highlighter(ps) => ps.iter_mut().for_each(map),
                Shape::Text { .. } => {
                    // text stays upright: keep its box centred on the rotated centre
                    let b = a.bounds();
                    let mut c = pt((b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0);
                    let old = c;
                    map(&mut c);
                    a.translate(c.x - old.x, c.y - old.y);
                }
                Shape::Counter { c, .. } => map(c),
            }
        }
        self.reset_crop();
    }

    pub(super) fn flip(&mut self, horizontal: bool) {
        self.commit_text();
        self.push_undo();
        let (w, h) = (self.doc.base.width() as f32, self.doc.base.height() as f32);
        let img = if horizontal { image::imageops::flip_horizontal(&*self.doc.base) } else { image::imageops::flip_vertical(&*self.doc.base) };
        self.doc.base = Arc::new(img);
        let map = |p: &mut Pt| {
            if horizontal {
                p.x = w - p.x
            } else {
                p.y = h - p.y
            }
        };
        for a in &mut self.doc.annotations {
            match &mut a.shape {
                Shape::Arrow { a, b } | Shape::Line { a, b } => {
                    map(a);
                    map(b);
                }
                Shape::Rect(r) | Shape::FilledRect(r) | Shape::Ellipse(r) | Shape::Blur(r) | Shape::Pixelate(r) | Shape::Spotlight(r) => {
                    let (mut p0, mut p1) = (pt(r.x0, r.y0), pt(r.x1, r.y1));
                    map(&mut p0);
                    map(&mut p1);
                    *r = Rect::from_pts(p0, p1);
                }
                Shape::Pencil(ps) | Shape::Highlighter(ps) => ps.iter_mut().for_each(map),
                Shape::Text { .. } | Shape::Counter { .. } => {
                    // keep text readable: only move its anchor
                    let b = a.bounds();
                    let (mut c, mut c2) = (pt(b.x0, b.y0), pt(b.x1, b.y1));
                    map(&mut c);
                    map(&mut c2);
                    let nb = Rect::from_pts(c, c2);
                    a.translate(nb.x0 - b.x0, nb.y0 - b.y0);
                }
            }
        }
        self.reset_crop();
    }
}
