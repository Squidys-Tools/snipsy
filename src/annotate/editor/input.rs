use crate::annotate::doc::*;
use crate::annotate::doc::Rgba;
use crate::app::{state, state_mut};
use gpui::prelude::*;
use gpui::*;
use super::{Editor, Drag, STROKES};

impl Editor {
    pub(super) fn set_color(&mut self, c: Rgba) {
        self.color = c;
        if self.selected.is_some() {
            self.push_undo_restyle();
            if let Some(a) = self.selected_mut() {
                a.color = c;
            }
        }
    }

    pub(super) fn set_width(&mut self, w: f32) {
        self.width = w;
        if self.selected.is_some() {
            self.push_undo_restyle();
            if let Some(a) = self.selected_mut() {
                a.width = w;
            }
        }
    }

    pub(super) fn set_tool(&mut self, t: Tool) {
        self.commit_text();
        if self.tool == Tool::Crop && t != Tool::Crop {
            self.crop = None;
        }
        self.tool = t;
        self.eyedropper = false;
        if t != Tool::Select {
            self.selected = None;
        }
        self.reset_crop();
    }

    pub(super) fn commit_text(&mut self) {
        self.ime_marked = 0;
        if let Some(id) = self.editing_text.take() {
            self.doc.annotations.retain(|a| !(a.id == id && matches!(&a.shape, Shape::Text { text, .. } if text.trim().is_empty())));
            // abandoned empty text / edit without changes: no undo step
            self.drop_noop_undo();
        }
    }

    /// Start editing an existing text annotation (snapshotting for undo) and
    /// sync the toolbar's text style from it.
    fn begin_edit_text(&mut self, id: u64) {
        self.commit_text();
        self.push_undo_tentative();
        self.editing_text = Some(id);
        self.selected = Some(id);
        self.ime_marked = 0;
        if let Some(Annotation { shape: Shape::Text { style, .. }, .. }) = self.doc.annotations.iter().find(|a| a.id == id) {
            self.text_style = *style;
        }
    }

    fn delete_selected(&mut self) {
        if let Some(id) = self.selected.take() {
            self.push_undo();
            self.doc.annotations.retain(|a| a.id != id);
        }
    }

    pub(super) fn on_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        if self.drag.is_some() {
            // a release was missed somehow: finish the previous gesture first
            self.finish_drag(window, cx);
        }
        let p = self.to_img(e.position);
        if self.eyedropper {
            let (w, h) = (self.doc.base.width(), self.doc.base.height());
            if p.x >= 0.0 && p.y >= 0.0 && (p.x as u32) < w && (p.y as u32) < h {
                let c = self.doc.base.get_pixel(p.x as u32, p.y as u32).0;
                let c = [c[0], c[1], c[2], 255];
                if !self.custom_colors.contains(&c) {
                    self.custom_colors.insert(0, c);
                    self.custom_colors.truncate(6);
                    state_mut(cx).settings.custom_colors = self.custom_colors.clone();
                    state(cx).settings.save();
                }
                self.set_color(c);
            }
            self.eyedropper = false;
            cx.notify();
            return;
        }
        match self.tool {
            Tool::Select => {
                self.commit_text();
                let hit = self.doc.annotations.iter().rev().find(|a| a.hit(p)).map(|a| a.id);
                self.selected = hit;
                if let Some(id) = hit {
                    if let Some(a) = self.doc.annotations.iter().find(|a| a.id == id) {
                        self.color = a.color;
                        self.width = a.width;
                    }
                    let is_text = matches!(self.doc.annotations.iter().find(|a| a.id == id), Some(Annotation { shape: Shape::Text { .. }, .. }));
                    if e.click_count >= 2 && is_text {
                        // snapshot covers both a possible move and the edit
                        self.begin_edit_text(id);
                    } else {
                        self.push_undo_tentative();
                    }
                    self.drag = Some(Drag::Move { last: p });
                }
            }
            Tool::Crop => self.drag = Some(Drag::Crop { start: p }),
            Tool::Text => {
                // commit first so a just-abandoned empty text cannot be hit
                self.commit_text();
                let hit = self
                    .doc
                    .annotations
                    .iter()
                    .rev()
                    .find(|a| matches!(a.shape, Shape::Text { .. }) && a.bounds().contains(p))
                    .map(|a| a.id);
                if let Some(id) = hit {
                    self.begin_edit_text(id);
                } else {
                    self.push_undo_tentative();
                    let id = self.new_id();
                    let size = (self.width * 8.0).max(18.0);
                    self.doc.annotations.push(Annotation {
                        id,
                        shape: Shape::Text { pos: pt(p.x, p.y - size * TEXT_LINE_HEIGHT / 2.0), text: String::new(), style: self.text_style },
                        color: self.color,
                        width: self.width,
                        floor_scale: 1.0,
                    });
                    self.editing_text = Some(id);
                    self.selected = Some(id);
                    self.ime_marked = 0;
                }
            }
            Tool::Counter => {
                self.push_undo();
                let id = self.new_id();
                let n = self.doc.next_counter();
                self.doc.annotations.push(Annotation { id, shape: Shape::Counter { c: p, n }, color: self.color, width: self.width, floor_scale: 1.0 });
            }
            t => {
                self.commit_text();
                let id = self.new_id();
                let shape = match t {
                    Tool::Arrow => Shape::Arrow { a: p, b: p },
                    Tool::Line => Shape::Line { a: p, b: p },
                    Tool::Rect => Shape::Rect(Rect::from_pts(p, p)),
                    Tool::FilledRect => Shape::FilledRect(Rect::from_pts(p, p)),
                    Tool::Ellipse => Shape::Ellipse(Rect::from_pts(p, p)),
                    Tool::Pencil => Shape::Pencil(vec![p]),
                    Tool::Highlighter => Shape::Highlighter(vec![p]),
                    Tool::Blur => Shape::Blur(Rect::from_pts(p, p)),
                    Tool::Pixelate => Shape::Pixelate(Rect::from_pts(p, p)),
                    _ => Shape::Spotlight(Rect::from_pts(p, p)),
                };
                self.drawing = Some(Annotation { id, shape, color: self.color, width: self.width, floor_scale: 1.0 });
                self.drag = Some(Drag::Draw { start: p });
            }
        }
        cx.notify();
    }

    pub(super) fn on_move(&mut self, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = self.drag else { return };
        if e.pressed_button != Some(MouseButton::Left) {
            return;
        }
        let mut p = self.to_img(e.position);
        let shift = e.modifiers.shift;
        match drag {
            Drag::Draw { start } => {
                if let Some(a) = self.drawing.as_mut() {
                    match &mut a.shape {
                        Shape::Arrow { b, .. } | Shape::Line { b, .. } => {
                            if shift {
                                let (dx, dy) = (p.x - start.x, p.y - start.y);
                                let ang = dy.atan2(dx);
                                let snap = (ang / std::f32::consts::FRAC_PI_4).round() * std::f32::consts::FRAC_PI_4;
                                let len = (dx * dx + dy * dy).sqrt();
                                p = pt(start.x + len * snap.cos(), start.y + len * snap.sin());
                            }
                            *b = p;
                        }
                        Shape::Rect(r) | Shape::FilledRect(r) | Shape::Ellipse(r) | Shape::Blur(r) | Shape::Pixelate(r) | Shape::Spotlight(r) => {
                            if shift {
                                let s = (p.x - start.x).abs().max((p.y - start.y).abs());
                                p = pt(start.x + s * (p.x - start.x).signum(), start.y + s * (p.y - start.y).signum());
                            }
                            *r = Rect::from_pts(start, p);
                        }
                        Shape::Pencil(pts) => pts.push(p),
                        Shape::Highlighter(pts) => {
                            if shift {
                                pts.truncate(1);
                            }
                            // book-style: keep the stroke horizontal while Shift is held
                            if shift {
                                p.y = pts[0].y;
                            }
                            pts.push(p);
                        }
                        _ => {}
                    }
                }
            }
            Drag::Move { last } => {
                let (dx, dy) = (p.x - last.x, p.y - last.y);
                if let Some(a) = self.selected_mut() {
                    a.translate(dx, dy);
                }
                self.drag = Some(Drag::Move { last: p });
            }
            Drag::Crop { start } => {
                let (w, h) = (self.doc.base.width() as f32, self.doc.base.height() as f32);
                let snap = |v: f32, max: f32| if v.abs() < 8.0 { 0.0 } else if (v - max).abs() < 8.0 { max } else { v };
                let mut q = pt(snap(p.x, w), snap(p.y, h));
                let s = pt(snap(start.x.clamp(0.0, w), w), snap(start.y.clamp(0.0, h), h));
                let ratio = self.crop_aspect.ratio().or(if shift { Some(1.0) } else { None });
                if let Some(r) = ratio {
                    let (dx, dy) = (q.x - s.x, q.y - s.y);
                    let max_w = if dx >= 0.0 { w - s.x } else { s.x };
                    let max_h = if dy >= 0.0 { h - s.y } else { s.y };
                    let (aw, ah) = (dx.abs(), dy.abs());
                    let (mut aw, mut ah) = if aw / ah.max(1.0) > r { (ah.max(1.0) * r, ah.max(1.0)) } else { (aw, aw / r) };
                    if aw > max_w {
                        (aw, ah) = (max_w, max_w / r);
                    }
                    if ah > max_h {
                        (aw, ah) = (max_h * r, max_h);
                    }
                    q = pt(s.x + aw.copysign(dx), s.y + ah.copysign(dy));
                }
                self.crop = Some(Rect::from_pts(s, q).clamp_to(w, h));
            }
        }
        cx.notify();
    }

    /// Mouse released outside the canvas: still finish the gesture.
    pub(super) fn on_up_out(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.drag.is_some() {
            self.finish_drag(window, cx);
        }
    }

    pub(super) fn on_up(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_drag(window, cx);
    }

    fn finish_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let drag = self.drag.take();
        if let (Some(Drag::Draw { .. }), Some(mut a)) = (drag, self.drawing.take()) {
            let keep = match &mut a.shape {
                Shape::Arrow { a: p0, b } | Shape::Line { a: p0, b } => p0.dist(*b) > 3.0,
                Shape::Rect(r) | Shape::FilledRect(r) | Shape::Ellipse(r) | Shape::Blur(r) | Shape::Pixelate(r) | Shape::Spotlight(r) => r.w() > 3.0 && r.h() > 3.0,
                Shape::Pencil(pts) => {
                    *pts = smooth(pts, 2);
                    !pts.is_empty()
                }
                Shape::Highlighter(pts) => {
                    *pts = smooth(pts, 1);
                    !pts.is_empty()
                }
                _ => true,
            };
            if keep {
                self.push_undo();
                if a.is_effect() {
                    // effects sit beneath vector annotations
                    let pos = self.doc.annotations.iter().position(|x| !x.is_effect()).unwrap_or(self.doc.annotations.len());
                    self.doc.annotations.insert(pos, a);
                } else {
                    self.doc.annotations.push(a);
                }
            }
        }
        if matches!(drag, Some(Drag::Move { .. })) && self.editing_text.is_none() {
            self.drop_noop_undo(); // nothing moved
        }
        self.refresh_display(window);
        cx.notify();
    }

    pub(super) fn on_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ks = &e.keystroke;
        let ctrl = ks.modifiers.control;
        let key = ks.key.as_str();

        if let Some(id) = self.editing_text {
            let exists = self.doc.annotations.iter().any(|a| a.id == id && matches!(a.shape, Shape::Text { .. }));
            if !exists {
                // stale edit target: drop it so keys reach normal handling
                self.editing_text = None;
                self.ime_marked = 0;
            } else if !ctrl {
                let mut handled = true;
                let mut changed = false;
                if let Some(Annotation { shape: Shape::Text { text, .. }, .. }) = self.doc.annotations.iter_mut().find(|a| a.id == id) {
                    match key {
                        "escape" => handled = false,
                        "enter" if !ks.modifiers.shift => handled = false,
                        "enter" => {
                            text.push('\n');
                            changed = true;
                        }
                        "backspace" => {
                            changed = text.pop().is_some();
                        }
                        // Printable characters (and IME / Unicode input) arrive through
                        // the platform input handler (WM_CHAR) — let the key propagate.
                        _ => return,
                    }
                }
                cx.stop_propagation();
                self.ime_marked = 0;
                if changed {
                    self.dirty = true;
                }
                if !handled {
                    self.commit_text();
                    self.selected = None;
                }
                cx.notify();
                return;
            }
        }

        match (ctrl, ks.modifiers.shift, key) {
            (true, false, "z") => self.do_undo(),
            (true, true, "z") | (true, _, "y") => self.do_redo(),
            (true, _, "c") => self.export_copy(cx),
            (true, false, "s") => self.export_save(cx),
            (true, true, "s") => self.export_save_as(window, cx),
            (true, _, "p") => self.export_pin(cx),
            (false, _, "delete") | (false, _, "backspace") => self.delete_selected(),
            (false, _, "enter") if self.tool == Tool::Crop => self.apply_crop(),
            (false, _, "escape") => {
                if self.tool == Tool::Crop {
                    self.crop = None;
                    self.tool = Tool::Arrow;
                } else if self.eyedropper {
                    self.eyedropper = false;
                } else {
                    self.selected = None;
                }
            }
            (false, false, k) => {
                if let Some(t) = Tool::shortcut(k) {
                    self.set_tool(t);
                } else if let Some(i) = "12345".find(k) {
                    self.set_width(STROKES[i]);
                }
            }
            _ => {}
        }
        self.refresh_display(window);
        cx.notify();
    }
}
