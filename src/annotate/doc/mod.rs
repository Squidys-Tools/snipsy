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
    /// Colored text with a subtle dark outline.
    Outline,
    /// Plain colored text.
    Plain,
    /// White text on a colored rounded pill.
    Pill,
    /// Colored text on a white rounded card.
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

    /// Rough bounding box for hit-testing / selection.
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

    /// Scale all coordinates by (sx, sy) and the stroke width by their mean
    /// (used when the base image is resized).
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

    /// True when both docs hold the same content (same base image buffer,
    /// annotations and backdrop).
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
