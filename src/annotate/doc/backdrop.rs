use super::geometry::Rect;
use image::RgbaImage;

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
