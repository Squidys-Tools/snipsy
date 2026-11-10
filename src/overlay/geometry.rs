use gpui::*;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Handle {
    N,
    S,
    E,
    W,
    NE,
    NW,
    SE,
    SW,
}

#[derive(Clone, Copy)]
pub(super) enum DragKind {
    New { start: Point<Pixels> },
    Move { offset: Point<Pixels> },
    Resize { handle: Handle, orig: Bounds<Pixels> },
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Aspect {
    Free,
    Square,
    Classic,
    Wide,
}

impl Aspect {
    pub(super) const ALL: [Aspect; 4] = [Aspect::Free, Aspect::Square, Aspect::Classic, Aspect::Wide];

    pub(super) fn ratio(self) -> Option<f32> {
        match self {
            Aspect::Free => None,
            Aspect::Square => Some(1.0),
            Aspect::Classic => Some(4.0 / 3.0),
            Aspect::Wide => Some(16.0 / 9.0),
        }
    }

    pub(super) fn label(self) -> &'static str {
        match self {
            Aspect::Free => "Free",
            Aspect::Square => "1:1",
            Aspect::Classic => "4:3",
            Aspect::Wide => "16:9",
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Dim {
    W,
    H,
}

/// The corner opposite `a` for a drag towards `p` locked to width/height = `r`,
/// growing along the dominant axis and shrunk to stay inside `full`.
pub(super) fn ratio_corner(a: Point<Pixels>, p: Point<Pixels>, r: f32, full: Size<Pixels>) -> Point<Pixels> {
    let (ax, ay) = (f32::from(a.x), f32::from(a.y));
    let (dx, dy) = (f32::from(p.x) - ax, f32::from(p.y) - ay);
    let (sx, sy) = (if dx < 0.0 { -1.0 } else { 1.0 }, if dy < 0.0 { -1.0 } else { 1.0 });
    let (mut w, mut h) = (dx.abs(), dy.abs());
    if w >= h * r {
        h = w / r;
    } else {
        w = h * r;
    }
    let avail_w = if sx < 0.0 { ax } else { f32::from(full.width) - ax }.max(0.0);
    let avail_h = if sy < 0.0 { ay } else { f32::from(full.height) - ay }.max(0.0);
    if w > avail_w {
        w = avail_w;
        h = w / r;
    }
    if h > avail_h {
        h = avail_h;
        w = h * r;
    }
    point(px(ax + sx * w), px(ay + sy * h))
}

/// Keep `b` inside a window of size `full` (shrinking it if it is larger).
pub(super) fn clamp_into(mut b: Bounds<Pixels>, full: Size<Pixels>) -> Bounds<Pixels> {
    b.size.width = b.size.width.min(full.width);
    b.size.height = b.size.height.min(full.height);
    b.origin.x = b.origin.x.max(px(0.)).min((full.width - b.size.width).max(px(0.)));
    b.origin.y = b.origin.y.max(px(0.)).min((full.height - b.size.height).max(px(0.)));
    b
}

pub(super) fn norm(a: Point<Pixels>, b: Point<Pixels>) -> Bounds<Pixels> {
    Bounds::from_corners(point(a.x.min(b.x), a.y.min(b.y)), point(a.x.max(b.x), a.y.max(b.y)))
}
