//! Coordinates are in image pixels of the base image.

mod backdrop;
mod geometry;
mod tool;

pub use backdrop::*;
pub use geometry::*;
pub use tool::*;

use image::RgbaImage;
use std::sync::Arc;

pub type Rgba = [u8; 4];

pub const PALETTE: [Rgba; 8] = [
    [255, 59, 48, 255],   // red
    [255, 149, 0, 255],   // orange
    [255, 204, 0, 255],   // yellow
    [52, 199, 89, 255],   // green
    [10, 132, 255, 255],  // blue
    [175, 82, 222, 255],  // purple
    [255, 255, 255, 255], // white
    [28, 28, 30, 255],    // black
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextStyle {
    Outline,
    Plain,
    Pill,
    Card,
}

impl TextStyle {
    pub const ALL: [TextStyle; 4] = [TextStyle::Outline, TextStyle::Plain, TextStyle::Pill, TextStyle::Card];
    pub fn label(self) -> &'static str {
        match self {
            TextStyle::Outline => "Outline",
            TextStyle::Plain => "Plain",
            TextStyle::Pill => "Pill",
            TextStyle::Card => "Card",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Arrow { a: Pt, b: Pt },
    Line { a: Pt, b: Pt },
    Rect(Rect),
    FilledRect(Rect),
    Ellipse(Rect),
    Pencil(Vec<Pt>),
    Highlighter(Vec<Pt>),
    Text { pos: Pt, text: String, style: TextStyle },
    Counter { c: Pt, n: u32 },
    Blur(Rect),
    Pixelate(Rect),
    Spotlight(Rect),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Annotation {
    pub id: u64,
    pub shape: Shape,
    pub color: Rgba,
    /// Stroke width in image px (font size / counter radius derive from it).
    pub width: f32,
    /// Accumulated image-resize factor; scales the minimum text / counter /
    /// highlighter sizes so they shrink and grow with the image (1.0 = none).
    pub floor_scale: f32,
}

impl Annotation {
    pub fn is_effect(&self) -> bool {
        matches!(self.shape, Shape::Blur(_) | Shape::Pixelate(_))
    }

    pub fn text_size(&self) -> f32 {
        (self.width * 8.0).max(18.0 * self.floor_scale)
    }

    pub fn counter_radius(&self) -> f32 {
        (self.width * 3.2).max(12.0 * self.floor_scale)
    }

    pub fn highlighter_width(&self) -> f32 {
        (self.width * 6.0).max(16.0 * self.floor_scale)
    }

    pub fn bounds(&self) -> Rect {
        let pad = self.width;
        match &self.shape {
            Shape::Arrow { a, b } | Shape::Line { a, b } => Rect::from_pts(*a, *b).inflate(pad * 2.0),
            Shape::Rect(r) | Shape::Ellipse(r) => r.inflate(pad),
            Shape::FilledRect(r) | Shape::Blur(r) | Shape::Pixelate(r) | Shape::Spotlight(r) => *r,
            Shape::Pencil(p) | Shape::Highlighter(p) => {
                let mut r = Rect { x0: f32::MAX, y0: f32::MAX, x1: f32::MIN, y1: f32::MIN };
                for q in p {
                    r.x0 = r.x0.min(q.x);
                    r.y0 = r.y0.min(q.y);
                    r.x1 = r.x1.max(q.x);
                    r.y1 = r.y1.max(q.y);
                }
                let k = if matches!(self.shape, Shape::Highlighter(_)) { self.highlighter_width() } else { self.width };
                r.inflate(k)
            }
            Shape::Text { pos, text, .. } => {
                let s = self.text_size();
                let lines: Vec<&str> = text.split('\n').collect();
                let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1).max(1);
                Rect {
                    x0: pos.x,
                    y0: pos.y,
                    x1: pos.x + longest as f32 * s * 0.58 + s * 0.8,
                    y1: pos.y + lines.len() as f32 * s * TEXT_LINE_HEIGHT + s * 0.4,
                }
            }
            Shape::Counter { c, .. } => {
                let r = self.counter_radius();
                Rect { x0: c.x - r, y0: c.y - r, x1: c.x + r, y1: c.y + r }
            }
        }
    }

    pub fn hit(&self, p: Pt) -> bool {
        match &self.shape {
            Shape::Arrow { a, b } | Shape::Line { a, b } => seg_dist(p, *a, *b) < self.width * 2.0 + 6.0,
            Shape::Pencil(pts) | Shape::Highlighter(pts) => {
                pts.windows(2).any(|w| seg_dist(p, w[0], w[1]) < self.width * 2.0 + 6.0)
            }
            Shape::Rect(r) | Shape::Ellipse(r) => {
                let outer = r.inflate(self.width + 6.0);
                let inner = r.inflate(-(self.width + 6.0));
                outer.contains(p) && !(inner.w() > 0.0 && inner.h() > 0.0 && inner.contains(p))
            }
            _ => self.bounds().contains(p),
        }
    }

    pub fn translate(&mut self, dx: f32, dy: f32) {
        let mv = |p: &mut Pt| {
            p.x += dx;
            p.y += dy;
        };
        match &mut self.shape {
            Shape::Arrow { a, b } | Shape::Line { a, b } => {
                mv(a);
                mv(b);
            }
            Shape::Rect(r) | Shape::FilledRect(r) | Shape::Ellipse(r) | Shape::Blur(r) | Shape::Pixelate(r) | Shape::Spotlight(r) => {
                *r = r.translate(dx, dy)
            }
            Shape::Pencil(p) | Shape::Highlighter(p) => p.iter_mut().for_each(mv),
            Shape::Text { pos, .. } => mv(pos),
            Shape::Counter { c, .. } => mv(c),
        }
    }

    pub fn scale(&mut self, sx: f32, sy: f32) {
        let sc = |p: &mut Pt| {
            p.x *= sx;
            p.y *= sy;
        };
        match &mut self.shape {
            Shape::Arrow { a, b } | Shape::Line { a, b } => {
                sc(a);
                sc(b);
            }
            Shape::Rect(r) | Shape::FilledRect(r) | Shape::Ellipse(r) | Shape::Blur(r) | Shape::Pixelate(r) | Shape::Spotlight(r) => {
                *r = Rect { x0: r.x0 * sx, y0: r.y0 * sy, x1: r.x1 * sx, y1: r.y1 * sy }
            }
            Shape::Pencil(p) | Shape::Highlighter(p) => p.iter_mut().for_each(sc),
            Shape::Text { pos, .. } => sc(pos),
            Shape::Counter { c, .. } => sc(c),
        }
        let k = (sx + sy) / 2.0;
        self.width = (self.width * k).max(0.5);
        self.floor_scale *= k;
    }
}

pub const TEXT_LINE_HEIGHT: f32 = 1.25;

#[derive(Clone)]
pub struct Doc {
    pub base: Arc<RgbaImage>,
    pub annotations: Vec<Annotation>,
    pub backdrop: Backdrop,
}

impl Doc {
    pub fn new(base: Arc<RgbaImage>) -> Self {
        Self { base, annotations: vec![], backdrop: Backdrop::default() }
    }

    pub fn same_content(&self, o: &Doc) -> bool {
        Arc::ptr_eq(&self.base, &o.base) && self.annotations == o.annotations && self.backdrop == o.backdrop
    }

    pub fn next_counter(&self) -> u32 {
        self.annotations
            .iter()
            .filter_map(|a| match a.shape {
                Shape::Counter { n, .. } => Some(n),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            + 1
    }

    pub fn effects_key(&self) -> Vec<(u8, i32, i32, i32, i32)> {
        self.annotations
            .iter()
            .filter_map(|a| match a.shape {
                Shape::Blur(r) => Some((0, r.x0 as i32, r.y0 as i32, r.x1 as i32, r.y1 as i32)),
                Shape::Pixelate(r) => Some((1, r.x0 as i32, r.y0 as i32, r.x1 as i32, r.y1 as i32)),
                _ => None,
            })
            .collect()
    }

    pub fn trim(&self) -> Option<Rect> {
        if self.backdrop.enabled && self.backdrop.balance {
            balance_trim(&self.base)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ann(shape: Shape, width: f32) -> Annotation {
        Annotation { id: 1, shape, color: [255, 0, 0, 255], width, floor_scale: 1.0 }
    }

    #[test]
    fn rect_normalises_corners_and_clamps_to_canvas() {
        let r = Rect::from_pts(pt(30.0, 40.0), pt(10.0, 5.0));
        assert_eq!(r, Rect { x0: 10.0, y0: 5.0, x1: 30.0, y1: 40.0 });
        assert!(r.contains(pt(10.0, 5.0)) && r.contains(pt(30.0, 40.0)) && !r.contains(pt(30.1, 40.0)));
        let c = r.inflate(100.0).clamp_to(50.0, 60.0);
        assert_eq!(c, Rect { x0: 0.0, y0: 0.0, x1: 50.0, y1: 60.0 });
    }

    #[test]
    fn seg_dist_handles_interior_beyond_endpoints_and_degenerate_segments() {
        let (a, b) = (pt(0.0, 0.0), pt(10.0, 0.0));
        assert_eq!(seg_dist(pt(5.0, 3.0), a, b), 3.0);
        assert_eq!(seg_dist(pt(13.0, 4.0), a, b), 5.0);
        assert_eq!(seg_dist(pt(-3.0, 4.0), a, b), 5.0);
        assert_eq!(seg_dist(pt(3.0, 4.0), a, a), 5.0);
    }

    #[test]
    fn hollow_shapes_hit_on_outline_only_and_lines_hit_near_segment() {
        let r = ann(Shape::Rect(Rect { x0: 0.0, y0: 0.0, x1: 100.0, y1: 100.0 }), 4.0);
        assert!(r.hit(pt(0.0, 50.0)));
        assert!(r.hit(pt(-8.0, 50.0)));
        assert!(!r.hit(pt(50.0, 50.0)));
        assert!(!r.hit(pt(-11.0, 50.0)));
        let filled = ann(Shape::FilledRect(Rect { x0: 0.0, y0: 0.0, x1: 100.0, y1: 100.0 }), 4.0);
        assert!(filled.hit(pt(50.0, 50.0)));
        let line = ann(Shape::Line { a: pt(0.0, 0.0), b: pt(100.0, 0.0) }, 2.0);
        assert!(line.hit(pt(50.0, 9.0)));
        assert!(!line.hit(pt(50.0, 11.0)));
    }

    #[test]
    fn translate_and_scale_move_coordinates_and_scale_stroke_by_mean_factor() {
        let mut a = ann(Shape::Arrow { a: pt(10.0, 10.0), b: pt(20.0, 30.0) }, 4.0);
        a.translate(5.0, -5.0);
        assert_eq!(a.shape, Shape::Arrow { a: pt(15.0, 5.0), b: pt(25.0, 25.0) });
        a.scale(2.0, 4.0);
        assert_eq!(a.shape, Shape::Arrow { a: pt(30.0, 20.0), b: pt(50.0, 100.0) });
        assert_eq!(a.width, 12.0);
        assert_eq!(a.floor_scale, 3.0);
        let mut thin = ann(Shape::Counter { c: pt(1.0, 1.0), n: 1 }, 1.0);
        thin.scale(0.1, 0.1);
        assert_eq!(thin.width, 0.5);
    }

    #[test]
    fn next_counter_follows_highest_existing_number() {
        let mut doc = Doc::new(Arc::new(RgbaImage::new(4, 4)));
        assert_eq!(doc.next_counter(), 1);
        doc.annotations.push(ann(Shape::Counter { c: pt(0.0, 0.0), n: 3 }, 2.0));
        doc.annotations.push(ann(Shape::Counter { c: pt(0.0, 0.0), n: 1 }, 2.0));
        doc.annotations.push(ann(Shape::Blur(Rect { x0: 0.0, y0: 0.0, x1: 1.0, y1: 1.0 }), 2.0));
        assert_eq!(doc.next_counter(), 4);
    }

    #[test]
    fn layout_without_backdrop_keeps_image_size_and_with_backdrop_pads_and_fits_aspect() {
        let off = layout(200.0, 100.0, &Backdrop::default(), None);
        assert_eq!((off.canvas_w, off.canvas_h, off.img_x, off.img_y), (200.0, 100.0, 0.0, 0.0));

        let mut bd = Backdrop { enabled: true, ..Backdrop::default() };
        let padded = layout(100.0, 100.0, &bd, None);
        assert_eq!((padded.canvas_w, padded.canvas_h, padded.img_x, padded.img_y), (132.0, 132.0, 16.0, 16.0));

        bd.aspect = Aspect::Wide16x9;
        let wide = layout(100.0, 100.0, &bd, None);
        assert_eq!(wide.canvas_h, 132.0);
        assert!((wide.canvas_w / wide.canvas_h - 16.0 / 9.0).abs() < 0.01);
        assert!(wide.img_x > wide.img_y);
    }

    #[test]
    fn balance_trim_keeps_equal_margin_around_content_and_ignores_unbordered_images() {
        let mut img = RgbaImage::from_pixel(100, 100, image::Rgba([255, 255, 255, 255]));
        for y in 40..60 {
            for x in 40..60 {
                img.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
            }
        }
        assert_eq!(balance_trim(&img), Some(Rect { x0: 16.0, y0: 16.0, x1: 84.0, y1: 84.0 }));

        let mut edged = RgbaImage::from_pixel(64, 64, image::Rgba([255, 255, 255, 255]));
        for (x, y) in [(2, 0), (0, 2), (2, 63), (63, 2)] {
            edged.put_pixel(x, y, image::Rgba([0, 0, 0, 255]));
        }
        assert_eq!(balance_trim(&edged), None);
        assert_eq!(balance_trim(&RgbaImage::new(31, 100)), None);
    }

    #[test]
    fn spotlight_dim_cells_cover_exactly_the_area_outside_spotlights() {
        assert!(spotlight_dim_cells(100.0, 100.0, &[]).is_empty());
        let spot = Rect { x0: 25.0, y0: 25.0, x1: 75.0, y1: 75.0 };
        let cells = spotlight_dim_cells(100.0, 100.0, &[spot]);
        let area: f32 = cells.iter().map(|c| c.w() * c.h()).sum();
        assert_eq!(area, 100.0 * 100.0 - 50.0 * 50.0);
        assert!(cells.iter().all(|c| !c.contains(pt(50.0, 50.0))));
        let all = Rect { x0: 0.0, y0: 0.0, x1: 100.0, y1: 100.0 };
        assert!(spotlight_dim_cells(100.0, 100.0, &[all]).is_empty());
    }

    #[test]
    fn smooth_keeps_endpoints_and_arrow_polygon_ends_at_tip() {
        let pts = [pt(0.0, 0.0), pt(10.0, 10.0), pt(20.0, 0.0), pt(30.0, 10.0)];
        let s = smooth(&pts, 2);
        assert_eq!((s[0], *s.last().unwrap()), (pts[0], pts[3]));
        assert!(s.len() > pts.len());
        assert_eq!(smooth(&pts[..2], 3), pts[..2].to_vec());
        let poly = arrow_polygon(pt(0.0, 0.0), pt(100.0, 0.0), 4.0);
        assert_eq!(poly.len(), 7);
        assert_eq!(poly[3], pt(100.0, 0.0));
    }
}
