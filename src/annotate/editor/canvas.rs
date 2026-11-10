use crate::annotate::doc::*;
use crate::annotate::doc::{Rgba, TextStyle};
use crate::annotate::render;
use crate::ui::{self, color};
use gpui::prelude::*;
use gpui::*;
use super::Editor;

impl Editor {
    pub(super) fn canvas_view(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bounds = self.canvas_bounds.get();
        let (iw, ih) = (self.doc.base.width() as f32, self.doc.base.height() as f32);
        let trim = self.doc.trim();
        let lay = layout(iw, ih, &self.doc.backdrop, trim);
        let sf = window.scale_factor();
        let avail_w = (f32::from(bounds.size.width) - 64.0).max(50.0);
        let avail_h = (f32::from(bounds.size.height) - 64.0).max(50.0);
        let z = (avail_w / lay.canvas_w).min(avail_h / lay.canvas_h).min(1.0 / sf);
        let comp_w = lay.canvas_w * z;
        let comp_h = lay.canvas_h * z;
        let comp_o = point(
            bounds.origin.x + px((f32::from(bounds.size.width) - comp_w) / 2.0),
            bounds.origin.y + px((f32::from(bounds.size.height) - comp_h) / 2.0),
        );
        // where image px (0,0) of the *base* lands on screen
        let img_o = point(comp_o.x + px((lay.img_x - lay.src.x0) * z), comp_o.y + px((lay.img_y - lay.src.y0) * z));
        self.view.set((img_o, z));
        let local = |p: Point<Pixels>| p - bounds.origin;

        let bd = self.doc.backdrop;
        let dst = Bounds::new(
            point(comp_o.x + px(lay.img_x * z), comp_o.y + px(lay.img_y * z)),
            size(px(lay.src.w() * z), px(lay.src.h() * z)),
        );

        let mut anns: Vec<Annotation> = self.doc.annotations.clone();
        if let Some(d) = &self.drawing {
            anns.push(d.clone());
        }
        let selected = self.selected;
        let crop = if self.tool == Tool::Crop { self.crop } else { None };
        let editing = self.editing_text;
        let base_w = iw;
        let base_h = ih;
        let paint_anns = anns.clone();
        let cb = self.canvas_bounds.clone();
        let entity_id = cx.entity_id();

        let entity = cx.entity();
        let input_focus = self.focus.clone();
        let wants_input = editing.is_some();
        let dragging = self.drag.is_some();
        let geometry = canvas(
            move |b, window, _| {
                // layout above used last frame's bounds: re-render once they change
                if cb.get() != b {
                    cb.set(b);
                    let id = entity_id;
                    window.on_next_frame(move |_, cx| cx.notify(id));
                }
            },
            move |b, _, window, cx| {
                if wants_input {
                    window.handle_input(&input_focus, ElementInputHandler::new(b, entity.clone()), cx);
                }
                if dragging {
                    // keep following the pointer after it leaves the canvas
                    let entity = entity.clone();
                    window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Capture && !b.contains(&e.position) {
                            entity.update(cx, |this, cx| this.on_move(e, window, cx));
                        }
                    });
                }
                let map = |p: Pt| point(img_o.x + px(p.x * z), img_o.y + px(p.y * z));
                let mrect = |r: Rect| Bounds::from_corners(map(pt(r.x0, r.y0)), map(pt(r.x1, r.y1)));
                window.with_content_mask(Some(ContentMask { bounds: dst }), |window| {
                    let spots: Vec<Rect> = paint_anns.iter().filter_map(|a| if let Shape::Spotlight(r) = a.shape { Some(r) } else { None }).collect();
                    for c in spotlight_dim_cells(base_w, base_h, &spots) {
                        window.paint_quad(fill(mrect(c), rgba(0x00000080)));
                    }
                    for a in &paint_anns {
                        paint_annotation(window, a, &map, z);
                    }
                });
                if let Some(id) = selected {
                    if editing.is_none() || editing != Some(id) {
                        if let Some(a) = paint_anns.iter().find(|a| a.id == id) {
                            let b = mrect(a.bounds().inflate(4.0 / z));
                            window.paint_quad(quad(b, px(4.), transparent_black(), px(1.5), color::accent(), BorderStyle::Dashed));
                        }
                    }
                }
                if let Some(r) = crop {
                    paint_crop_overlay(window, &mrect, base_w, base_h, r);
                }
            },
        )
        .absolute()
        .size_full();

        let labels = overlay_labels(&anns, img_o, dst.origin, z, editing);

        let (angle, c0, c1) = BACKGROUNDS[bd.preset % BACKGROUNDS.len()];
        let lo = local(comp_o);
        let ld = local(dst.origin);
        let rad = if bd.enabled { px(bd.radius * z) } else { px(0.) };

        div()
            .id("canvas")
            .flex_1()
            .h_full()
            .relative()
            .overflow_hidden()
            .bg(color::canvas_bg())
            .cursor(match self.tool {
                Tool::Select => CursorStyle::Arrow,
                Tool::Text => CursorStyle::IBeam,
                _ => CursorStyle::Crosshair,
            })
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_down))
            .on_mouse_move(cx.listener(Self::on_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_up_out))
            .when(bd.enabled, |d| {
                d.child(
                    div()
                        .absolute()
                        .left(lo.x)
                        .top(lo.y)
                        .w(px(comp_w))
                        .h(px(comp_h))
                        .bg(linear_gradient(angle, linear_color_stop(ui::hex_hsla(c0), 0.0), linear_color_stop(ui::hex_hsla(c1), 1.0))),
                )
            })
            .child(
                div()
                    .absolute()
                    .left(ld.x)
                    .top(ld.y)
                    .w(dst.size.width)
                    .h(dst.size.height)
                    .rounded(rad)
                    .overflow_hidden()
                    .when(bd.enabled && bd.shadow, |d| d.shadow_xl())
                    .when(!bd.enabled, |d| d.shadow_lg())
                    .child(match (&trim, &self.display_trim) {
                        // trimmed: draw the cropped copy so its own corners round like the export
                        (Some(_), Some((_, t))) => img(t.clone()).absolute().left_0().top_0().w(dst.size.width).h(dst.size.height).rounded(rad),
                        _ => img(self.display.clone())
                            .absolute()
                            .left(px(-lay.src.x0 * z))
                            .top(px(-lay.src.y0 * z))
                            .w(px(iw * z))
                            .h(px(ih * z))
                            .rounded(if trim.is_none() { rad } else { px(0.) }),
                    }),
            )
            .child(geometry)
            .child(div().absolute().left(ld.x).top(ld.y).w(dst.size.width).h(dst.size.height).overflow_hidden().children(labels))
            .when(self.eyedropper, |d| {
                d.child(
                    div().absolute().top(px(12.)).left_0().right_0().flex().justify_center().child(
                        div().px(px(12.)).py(px(6.)).rounded(px(8.)).bg(color::hud()).text_size(px(12.)).text_color(color::text()).child("Click anywhere on the image to pick a color"),
                    ),
                )
            })
    }
}

fn paint_crop_overlay(window: &mut Window, mrect: &dyn Fn(Rect) -> Bounds<Pixels>, base_w: f32, base_h: f32, r: Rect) {
    let full = mrect(Rect { x0: 0.0, y0: 0.0, x1: base_w, y1: base_h });
    let c = mrect(r);
    let dim = rgba(0x000000A0);
    let (x0, y0, x1, y1) = (c.origin.x, c.origin.y, c.origin.x + c.size.width, c.origin.y + c.size.height);
    let (fx0, fy0, fx1, fy1) = (full.origin.x, full.origin.y, full.origin.x + full.size.width, full.origin.y + full.size.height);
    for b in [
        Bounds::from_corners(point(fx0, fy0), point(fx1, y0)),
        Bounds::from_corners(point(fx0, y1), point(fx1, fy1)),
        Bounds::from_corners(point(fx0, y0), point(x0, y1)),
        Bounds::from_corners(point(x1, y0), point(fx1, y1)),
    ] {
        if b.size.width > px(0.) && b.size.height > px(0.) {
            window.paint_quad(fill(b, dim));
        }
    }
    window.paint_quad(outline(c, rgb(0xFFFFFF), BorderStyle::Solid));
    for i in 1..3 {
        let fx = x0 + (x1 - x0) * (i as f32 / 3.0);
        let fy = y0 + (y1 - y0) * (i as f32 / 3.0);
        window.paint_quad(fill(Bounds::new(point(fx, y0), size(px(1.), y1 - y0)), rgba(0xFFFFFF55)));
        window.paint_quad(fill(Bounds::new(point(x0, fy), size(x1 - x0, px(1.))), rgba(0xFFFFFF55)));
    }
    for (hx, hy) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
        window.paint_quad(fill(Bounds::new(point(hx - px(4.), hy - px(4.)), size(px(8.), px(8.))), rgb(0xFFFFFF)));
    }
}

fn overlay_labels(anns: &[Annotation], img_o: Point<Pixels>, origin: Point<Pixels>, z: f32, editing: Option<u64>) -> Vec<AnyElement> {
    let mut labels: Vec<AnyElement> = vec![];
    for a in anns {
        match &a.shape {
            Shape::Text { pos, text, style } => {
                let o = point(img_o.x + px(pos.x * z), img_o.y + px(pos.y * z)) - origin;
                labels.push(text_element(a, text, *style, o, z, editing == Some(a.id)).into_any_element());
            }
            Shape::Counter { c, n } => {
                let r = a.counter_radius() * z;
                let o = point(img_o.x + px(c.x * z - r), img_o.y + px(c.y * z - r)) - origin;
                let fg = render::contrast_on(a.color);
                labels.push(
                    div()
                        .absolute()
                        .left(o.x)
                        .top(o.y)
                        .size(px(r * 2.0))
                        .rounded_full()
                        .bg(ui::hsla_of(a.color))
                        .border(px((r * 0.12).max(1.0)))
                        .border_color(rgb(0xFFFFFF))
                        .shadow_md()
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(r * 1.15))
                        .line_height(px(r * 2.0))
                        .font_weight(FontWeight::BOLD)
                        .text_color(ui::hsla_of(fg))
                        .child(n.to_string())
                        .into_any_element(),
                );
            }
            _ => {}
        }
    }
    labels
}

fn text_element(a: &Annotation, text: &str, style: TextStyle, o: Point<Pixels>, z: f32, editing: bool) -> Div {
    let size = a.text_size() * z;
    let lh = size * TEXT_LINE_HEIGHT;
    let shown = if editing { format!("{text}▏") } else { text.to_string() };
    let shown = if shown.is_empty() { " ".to_string() } else { shown };
    let mut fg = a.color;
    let mut bg: Option<Rgba> = None;
    match style {
        TextStyle::Pill => {
            bg = Some(a.color);
            fg = render::contrast_on(a.color);
        }
        TextStyle::Card => {
            bg = Some([255, 255, 255, 255]);
            if a.color == [255, 255, 255, 255] {
                fg = [28, 28, 30, 255];
            }
        }
        _ => {}
    }
    let lines = |color: Rgba| {
        let mut d = div().flex().flex_col();
        for l in shown.split('\n') {
            d = d.child(
                div()
                    .h(px(lh))
                    .line_height(px(lh))
                    .whitespace_nowrap()
                    .text_color(ui::hsla_of(color))
                    .child(if l.is_empty() { " ".to_string() } else { l.to_string() }),
            );
        }
        d
    };
    let pad = size * 0.35;
    let mut el = div()
        .absolute()
        .left(o.x - px(if bg.is_some() { pad } else { 0.0 }))
        .top(o.y - px(if bg.is_some() { pad * 0.5 } else { 0.0 }))
        .font_family(ui::FONT)
        .font_weight(FontWeight::BOLD)
        .text_size(px(size));
    if let Some(b) = bg {
        el = el.px(px(pad)).py(px(pad * 0.5)).rounded(px(size * 0.3)).bg(ui::hsla_of(b)).shadow_md();
    }
    if style == TextStyle::Outline {
        let oc = render::contrast_on(fg);
        let off = (size * 0.06).max(1.0);
        let mut stack = div().relative();
        for (dx, dy) in [(-off, 0.0), (off, 0.0), (0.0, -off), (0.0, off), (-off, -off), (off, off), (-off, off), (off, -off)] {
            stack = stack.child(lines(oc).absolute().left(px(dx)).top(px(dy)));
        }
        el = el.child(stack.child(lines(fg)));
    } else {
        el = el.child(lines(fg));
    }
    if editing {
        el = el.border_1().border_color(color::accent()).rounded(px(4.));
    }
    el
}

fn stroke_path(points: &[Point<Pixels>], width: f32, square: bool) -> Option<Path<Pixels>> {
    use lyon::tessellation::{LineCap, LineJoin};
    let cap = if square { LineCap::Square } else { LineCap::Round };
    let opts = StrokeOptions::default().with_line_width(width).with_line_cap(cap).with_line_join(LineJoin::Round);
    let mut b = PathBuilder::stroke(px(width)).with_style(PathStyle::Stroke(opts));
    let first = points.first()?;
    b.move_to(*first);
    if points.len() == 1 {
        b.line_to(*first + point(px(0.1), px(0.1)));
    }
    for p in &points[1..] {
        b.line_to(*p);
    }
    b.build().ok()
}

fn paint_annotation(window: &mut Window, a: &Annotation, map: &dyn Fn(Pt) -> Point<Pixels>, z: f32) {
    let col = ui::hsla_of(a.color);
    let w = a.width * z;
    let shadow = hsla(0., 0., 0., 0.22);
    let shift = |p: Point<Pixels>| p + point(px(0.), px((a.width * 0.35).max(1.0) * z));
    match &a.shape {
        Shape::Arrow { a: p0, b } => {
            let poly: Vec<Point<Pixels>> = arrow_polygon(*p0, *b, a.width).into_iter().map(map).collect();
            for (pts, c) in [(poly.iter().map(|p| shift(*p)).collect::<Vec<_>>(), shadow), (poly.clone(), col)] {
                let mut pb = PathBuilder::fill();
                pb.add_polygon(&pts, true);
                if let Ok(path) = pb.build() {
                    window.paint_path(path, c);
                }
                if let Some(path) = stroke_path(&[pts[0], pts[1], pts[2], pts[3], pts[4], pts[5], pts[6], pts[0]], w * 0.35, false) {
                    window.paint_path(path, c);
                }
            }
        }
        Shape::Line { a: p0, b } => {
            if let Some(path) = stroke_path(&[shift(map(*p0)), shift(map(*b))], w, false) {
                window.paint_path(path, shadow);
            }
            if let Some(path) = stroke_path(&[map(*p0), map(*b)], w, false) {
                window.paint_path(path, col);
            }
        }
        Shape::Rect(r) => {
            let b = Bounds::from_corners(map(pt(r.x0, r.y0)), map(pt(r.x1, r.y1)));
            let half = px(w / 2.0);
            let outer = Bounds::new(b.origin - point(half, half), b.size + size(px(w), px(w)));
            let rad = px(a.width.min(6.0) * z + w / 2.0);
            let sh = Bounds::new(shift(outer.origin), outer.size);
            window.paint_quad(quad(sh, rad, transparent_black(), px(w), shadow, BorderStyle::Solid));
            window.paint_quad(quad(outer, rad, transparent_black(), px(w), col, BorderStyle::Solid));
        }
        Shape::FilledRect(r) => {
            let b = Bounds::from_corners(map(pt(r.x0, r.y0)), map(pt(r.x1, r.y1)));
            window.paint_quad(quad(b, px(a.width.min(6.0) * z), col, px(0.), transparent_black(), BorderStyle::Solid));
        }
        Shape::Ellipse(r) => {
            let c = map(pt((r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0));
            let (rx, ry) = (r.w() * z / 2.0, r.h() * z / 2.0);
            let n = ((rx + ry) / 2.0).clamp(24.0, 160.0) as usize;
            let pts: Vec<Point<Pixels>> = (0..=n)
                .map(|i| {
                    let t = i as f32 / n as f32 * std::f32::consts::TAU;
                    c + point(px(rx * t.cos()), px(ry * t.sin()))
                })
                .collect();
            if let Some(path) = stroke_path(&pts.iter().map(|p| shift(*p)).collect::<Vec<_>>(), w, false) {
                window.paint_path(path, shadow);
            }
            if let Some(path) = stroke_path(&pts, w, false) {
                window.paint_path(path, col);
            }
        }
        Shape::Pencil(ps) => {
            let pts: Vec<Point<Pixels>> = ps.iter().map(|p| map(*p)).collect();
            if let Some(path) = stroke_path(&pts.iter().map(|p| shift(*p)).collect::<Vec<_>>(), w, false) {
                window.paint_path(path, shadow);
            }
            if let Some(path) = stroke_path(&pts, w, false) {
                window.paint_path(path, col);
            }
        }
        Shape::Highlighter(ps) => {
            let pts: Vec<Point<Pixels>> = ps.iter().map(|p| map(*p)).collect();
            let mut c = col;
            c.a = 0.43;
            if let Some(path) = stroke_path(&pts, a.highlighter_width() * z, true) {
                window.paint_path(path, c);
            }
        }
        Shape::Blur(r) | Shape::Pixelate(r) => {
            // committed effects are baked into the display image; show the frame while drawing
            let b = Bounds::from_corners(map(pt(r.x0, r.y0)), map(pt(r.x1, r.y1)));
            window.paint_quad(quad(b, px(2.), transparent_black(), px(1.), rgba(0xFFFFFF40), BorderStyle::Dashed));
        }
        Shape::Spotlight(_) | Shape::Text { .. } | Shape::Counter { .. } => {}
    }
}
