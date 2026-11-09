//! Annotation document model, shared by the live editor and the exporter.
//! All coordinates are in *image pixels* of the base image.

use image::RgbaImage;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Select,
    Crop,
    Arrow,
    Rect,
    FilledRect,
    Ellipse,
    Line,
    Pencil,
    Highlighter,
    Text,
    Counter,
    Blur,
    Pixelate,
    Spotlight,
}

impl Tool {
    pub const TOOLBAR: [Tool; 14] = [
        Tool::Select,
        Tool::Crop,
        Tool::Rect,
        Tool::FilledRect,
        Tool::Ellipse,
        Tool::Line,
        Tool::Arrow,
        Tool::Text,
        Tool::Pencil,
        Tool::Highlighter,
        Tool::Counter,
        Tool::Spotlight,
        Tool::Blur,
        Tool::Pixelate,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select (V)",
            Tool::Crop => "Crop (C)",
            Tool::Arrow => "Arrow (A)",
            Tool::Rect => "Rectangle (R)",
            Tool::FilledRect => "Filled Rectangle (F)",
            Tool::Ellipse => "Ellipse (O)",
            Tool::Line => "Line (L)",
            Tool::Pencil => "Pencil (P)",
            Tool::Highlighter => "Highlighter (H)",
            Tool::Text => "Text (T)",
            Tool::Counter => "Counter (N)",
            Tool::Blur => "Blur (B)",
            Tool::Pixelate => "Pixelate (X)",
            Tool::Spotlight => "Spotlight (S)",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Tool::Select => "icons/cursor.svg",
            Tool::Crop => "icons/crop.svg",
            Tool::Arrow => "icons/arrow.svg",
            Tool::Rect => "icons/rect.svg",
            Tool::FilledRect => "icons/rect-filled.svg",
            Tool::Ellipse => "icons/ellipse.svg",
            Tool::Line => "icons/line.svg",
            Tool::Pencil => "icons/pencil.svg",
            Tool::Highlighter => "icons/highlighter.svg",
            Tool::Text => "icons/text.svg",
            Tool::Counter => "icons/counter.svg",
            Tool::Blur => "icons/blur.svg",
            Tool::Pixelate => "icons/pixelate.svg",
            Tool::Spotlight => "icons/spotlight.svg",
        }
    }

    pub fn shortcut(c: &str) -> Option<Tool> {
        Some(match c {
            "v" => Tool::Select,
            "c" => Tool::Crop,
            "a" => Tool::Arrow,
            "r" => Tool::Rect,
            "f" => Tool::FilledRect,
            "o" => Tool::Ellipse,
            "l" => Tool::Line,
            "p" => Tool::Pencil,
            "h" => Tool::Highlighter,
            "t" => Tool::Text,
            "n" => Tool::Counter,
            "b" => Tool::Blur,
            "x" => Tool::Pixelate,
            "s" => Tool::Spotlight,
            _ => return None,
        })
    }

    /// Tools that use the color / stroke controls.
    pub fn uses_color(self) -> bool {
        !matches!(self, Tool::Select | Tool::Crop | Tool::Blur | Tool::Pixelate | Tool::Spotlight)
    }
}

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

pub fn pt(x: f32, y: f32) -> Pt {
    Pt { x, y }
}

impl Pt {
    pub fn dist(self, o: Pt) -> f32 {
        ((self.x - o.x).powi(2) + (self.y - o.y).powi(2)).sqrt()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Rect {
    pub fn from_pts(a: Pt, b: Pt) -> Self {
        Rect { x0: a.x.min(b.x), y0: a.y.min(b.y), x1: a.x.max(b.x), y1: a.y.max(b.y) }
    }
    pub fn w(&self) -> f32 {
        self.x1 - self.x0
    }
    pub fn h(&self) -> f32 {
        self.y1 - self.y0
    }
    pub fn contains(&self, p: Pt) -> bool {
        p.x >= self.x0 && p.x <= self.x1 && p.y >= self.y0 && p.y <= self.y1
    }
    pub fn inflate(&self, d: f32) -> Rect {
        Rect { x0: self.x0 - d, y0: self.y0 - d, x1: self.x1 + d, y1: self.y1 + d }
    }
    pub fn translate(&self, dx: f32, dy: f32) -> Rect {
        Rect { x0: self.x0 + dx, y0: self.y0 + dy, x1: self.x1 + dx, y1: self.y1 + dy }
    }
    pub fn clamp_to(&self, w: f32, h: f32) -> Rect {
        Rect {
            x0: self.x0.clamp(0.0, w),
            y0: self.y0.clamp(0.0, h),
            x1: self.x1.clamp(0.0, w),
            y1: self.y1.clamp(0.0, h),
        }
    }
}

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

pub fn seg_dist(p: Pt, a: Pt, b: Pt) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    if l2 < 1e-6 {
        return p.dist(a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0);
    p.dist(pt(a.x + t * dx, a.y + t * dy))
}

/// CleanShot-style tapered arrow: thin tail growing into a solid head.
pub fn arrow_polygon(a: Pt, b: Pt, width: f32) -> Vec<Pt> {
    let len = a.dist(b).max(1.0);
    let (ux, uy) = ((b.x - a.x) / len, (b.y - a.y) / len);
    let (nx, ny) = (-uy, ux);
    let head_len = (width * 4.5).min(len * 0.6).max(8.0);
    let head_w = head_len * 0.62;
    let shaft_w = (width * 0.9).min(head_w * 0.55);
    let tail_w = shaft_w * 0.3;
    let base = pt(b.x - ux * head_len, b.y - uy * head_len);
    let inner = pt(b.x - ux * head_len * 0.82, b.y - uy * head_len * 0.82);
    vec![
        pt(a.x + nx * tail_w, a.y + ny * tail_w),
        pt(inner.x + nx * shaft_w, inner.y + ny * shaft_w),
        pt(base.x + nx * head_w, base.y + ny * head_w),
        b,
        pt(base.x - nx * head_w, base.y - ny * head_w),
        pt(inner.x - nx * shaft_w, inner.y - ny * shaft_w),
        pt(a.x - nx * tail_w, a.y - ny * tail_w),
    ]
}

/// Chaikin corner-cutting for smooth freehand strokes.
pub fn smooth(points: &[Pt], iterations: usize) -> Vec<Pt> {
    let mut pts = simplify(points, 1.2);
    for _ in 0..iterations {
        if pts.len() < 3 {
            break;
        }
        let mut out = Vec::with_capacity(pts.len() * 2);
        out.push(pts[0]);
        for w in pts.windows(2) {
            let (p, q) = (w[0], w[1]);
            out.push(pt(0.75 * p.x + 0.25 * q.x, 0.75 * p.y + 0.25 * q.y));
            out.push(pt(0.25 * p.x + 0.75 * q.x, 0.25 * p.y + 0.75 * q.y));
        }
        out.push(*pts.last().unwrap());
        pts = out;
    }
    pts
}

fn simplify(points: &[Pt], min_dist: f32) -> Vec<Pt> {
    let mut out: Vec<Pt> = Vec::with_capacity(points.len());
    for &p in points {
        if out.last().map_or(true, |l: &Pt| l.dist(p) >= min_dist) {
            out.push(p);
        }
    }
    if let (Some(&last), Some(&o)) = (points.last(), out.last()) {
        if o != last {
            out.push(last);
        }
    }
    out
}

/// Cells of the area outside all spotlight rects (to be dimmed).
pub fn spotlight_dim_cells(w: f32, h: f32, spots: &[Rect]) -> Vec<Rect> {
    if spots.is_empty() {
        return vec![];
    }
    let mut xs = vec![0.0, w];
    let mut ys = vec![0.0, h];
    for s in spots {
        let s = s.clamp_to(w, h);
        xs.extend([s.x0, s.x1]);
        ys.extend([s.y0, s.y1]);
    }
    xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ys.sort_by(|a, b| a.partial_cmp(b).unwrap());
    xs.dedup();
    ys.dedup();
    let mut cells = Vec::new();
    for yw in ys.windows(2) {
        // merge horizontally adjacent dim cells per row
        let mut run: Option<Rect> = None;
        for xw in xs.windows(2) {
            let c = Rect { x0: xw[0], y0: yw[0], x1: xw[1], y1: yw[1] };
            let mid = pt((c.x0 + c.x1) / 2.0, (c.y0 + c.y1) / 2.0);
            let lit = spots.iter().any(|s| s.contains(mid));
            match (&mut run, lit) {
                (Some(r), false) => r.x1 = c.x1,
                (None, false) => run = Some(c),
                (Some(_), true) => cells.push(run.take().unwrap()),
                (None, true) => {}
            }
        }
        if let Some(r) = run {
            cells.push(r);
        }
    }
    cells
}

// ---------------------------------------------------------------- backdrop

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Aspect {
    Auto,
    Square,
    Wide16x9,
    Classic4x3,
    Portrait9x16,
}

impl Aspect {
    pub const ALL: [Aspect; 5] = [Aspect::Auto, Aspect::Square, Aspect::Wide16x9, Aspect::Classic4x3, Aspect::Portrait9x16];
    pub fn label(self) -> &'static str {
        match self {
            Aspect::Auto => "Auto",
            Aspect::Square => "1:1",
            Aspect::Wide16x9 => "16:9",
            Aspect::Classic4x3 => "4:3",
            Aspect::Portrait9x16 => "9:16",
        }
    }
    pub fn ratio(self) -> Option<f32> {
        match self {
            Aspect::Auto => None,
            Aspect::Square => Some(1.0),
            Aspect::Wide16x9 => Some(16.0 / 9.0),
            Aspect::Classic4x3 => Some(4.0 / 3.0),
            Aspect::Portrait9x16 => Some(9.0 / 16.0),
        }
    }
}

/// (angle in degrees, from, to)
pub const BACKGROUNDS: [(f32, u32, u32); 20] = [
    (135.0, 0x8E2DE2, 0x4A00E0),
    (135.0, 0xFF6A88, 0xFF99AC),
    (135.0, 0x00C6FB, 0x005BEA),
    (135.0, 0xF7971E, 0xFFD200),
    (135.0, 0x43E97B, 0x38F9D7),
    (135.0, 0xFA709A, 0xFEE140),
    (135.0, 0x30CFD0, 0x330867),
    (135.0, 0xA18CD1, 0xFBC2EB),
    (135.0, 0xFF0844, 0xFFB199),
    (135.0, 0x667EEA, 0x764BA2),
    (160.0, 0x0F2027, 0x2C5364),
    (135.0, 0xF093FB, 0xF5576C),
    (135.0, 0x5EE7DF, 0xB490CA),
    (135.0, 0xD4FC79, 0x96E6A1),
    (135.0, 0x84FAB0, 0x8FD3F4),
    (135.0, 0xFCCB90, 0xD57EEB),
    (180.0, 0xE0E0E0, 0xF5F5F7),
    (180.0, 0x1C1C1E, 0x3A3A3C),
    (135.0, 0xFF9A9E, 0xFAD0C4),
    (135.0, 0x13547A, 0x80D0C7),
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Backdrop {
    pub enabled: bool,
    pub preset: usize,
    /// padding as a fraction of the shorter image side
    pub padding: f32,
    /// corner radius of the screenshot, in image px
    pub radius: f32,
    pub shadow: bool,
    pub aspect: Aspect,
    /// Auto Balance: trim uniform borders so the content sits visually centred.
    pub balance: bool,
}

impl Default for Backdrop {
    fn default() -> Self {
        Self {
            enabled: false,
            preset: 0,
            padding: 0.10,
            radius: 12.0,
            shadow: true,
            aspect: Aspect::Auto,
            balance: false,
        }
    }
}

/// Where the (possibly trimmed) image sits inside the backdrop canvas.
#[derive(Clone, Copy, Debug)]
pub struct Layout {
    pub canvas_w: f32,
    pub canvas_h: f32,
    /// image placement (image px space of canvas)
    pub img_x: f32,
    pub img_y: f32,
    /// visible part of the base image (after balance trim)
    pub src: Rect,
}

pub fn layout(img_w: f32, img_h: f32, bd: &Backdrop, trim: Option<Rect>) -> Layout {
    let src = trim.unwrap_or(Rect { x0: 0.0, y0: 0.0, x1: img_w, y1: img_h });
    if !bd.enabled {
        return Layout { canvas_w: img_w, canvas_h: img_h, img_x: 0.0, img_y: 0.0, src: Rect { x0: 0.0, y0: 0.0, x1: img_w, y1: img_h } };
    }
    let (w, h) = (src.w(), src.h());
    let pad = (w.min(h) * bd.padding).max(if bd.padding > 0.0 { 16.0 } else { 0.0 }).round();
    let mut cw = w + pad * 2.0;
    let mut ch = h + pad * 2.0;
    if let Some(r) = bd.aspect.ratio() {
        if cw / ch < r {
            cw = ch * r;
        } else {
            ch = cw / r;
        }
    }
    Layout {
        canvas_w: cw.round(),
        canvas_h: ch.round(),
        img_x: ((cw - w) / 2.0).round(),
        img_y: ((ch - h) / 2.0).round(),
        src,
    }
}

/// Detect uniform borders (same color as the edge) for Auto Balance.
pub fn balance_trim(img: &RgbaImage) -> Option<Rect> {
    let (w, h) = (img.width(), img.height());
    if w < 32 || h < 32 {
        return None;
    }
    let c = *img.get_pixel(0, 0);
    let same = |x: u32, y: u32| {
        let p = img.get_pixel(x, y);
        (0..3).all(|i| (p[i] as i32 - c[i] as i32).abs() <= 6)
    };
    let row_uniform = |y: u32| (0..w).step_by(2).all(|x| same(x, y));
    let col_uniform = |x: u32| (0..h).step_by(2).all(|y| same(x, y));
    let mut top = 0;
    while top < h / 2 && row_uniform(top) {
        top += 1;
    }
    let mut bottom = h;
    while bottom > h / 2 && row_uniform(bottom - 1) {
        bottom -= 1;
    }
    let mut left = 0;
    while left < w / 2 && col_uniform(left) {
        left += 1;
    }
    let mut right = w;
    while right > w / 2 && col_uniform(right - 1) {
        right -= 1;
    }
    if top == 0 && left == 0 && bottom == h && right == w {
        return None;
    }
    // keep an equal margin of the uniform color on every side
    let m = top.min(left).min(h - bottom).min(w - right).max(8) as f32;
    let m = m.min(24.0);
    Some(Rect {
        x0: (left as f32 - m).max(0.0),
        y0: (top as f32 - m).max(0.0),
        x1: (right as f32 + m).min(w as f32),
        y1: (bottom as f32 + m).min(h as f32),
    })
}

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
