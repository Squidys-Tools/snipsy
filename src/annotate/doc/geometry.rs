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

pub fn seg_dist(p: Pt, a: Pt, b: Pt) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    if l2 < 1e-6 {
        return p.dist(a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0);
    p.dist(pt(a.x + t * dx, a.y + t * dy))
}

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
