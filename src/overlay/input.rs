use super::geometry::{norm, ratio_corner, Dim, DragKind, Handle};
use super::{close_all, with_selection_holder, OverlayView};
use crate::app::Mode;
use crate::history::CaptureKind;
use gpui::*;

impl OverlayView {
    pub(super) fn on_down(&mut self, e: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let p = e.position;
        self.mouse = Some(p);
        self.editing = None;
        match self.mode {
            Mode::Window => {}
            Mode::AllInOne => {
                if let Some(h) = self.handle_at(p) {
                    self.drag = Some(DragKind::Resize { handle: h, orig: self.selection.unwrap() });
                } else if let Some(sel) = self.selection.filter(|s| s.contains(&p)) {
                    if e.click_count >= 2 {
                        self.capture_selection(CaptureKind::Area, cx);
                        return;
                    }
                    self.drag = Some(DragKind::Move { offset: p - sel.origin });
                } else {
                    self.drag = Some(DragKind::New { start: p });
                    self.selection = None;
                    self.clear_peer_selections(cx);
                }
            }
            Mode::Area | Mode::Ocr => {
                self.drag = Some(DragKind::New { start: p });
                self.selection = None;
                self.clear_peer_selections(cx);
            }
        }
        cx.notify();
    }

    pub(super) fn on_move(&mut self, e: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.mouse = Some(e.position);
        let full = window.bounds().size;
        // gpui captures the mouse during a drag, so positions can lie outside the window
        let p = point(e.position.x.max(px(0.)).min(full.width), e.position.y.max(px(0.)).min(full.height));
        // Shift locks to a square; otherwise the chosen aspect preset applies
        let ratio = if e.modifiers.shift { Some(1.0) } else { self.aspect.ratio() };
        match self.drag {
            Some(DragKind::New { start }) => {
                let b = match ratio {
                    Some(r) => norm(start, ratio_corner(start, p, r, full)),
                    None => norm(start, p),
                };
                self.selection = Some(b);
            }
            Some(DragKind::Move { offset }) => {
                if let Some(sel) = self.selection.as_mut() {
                    let mut o = p - offset;
                    let maxx = (full.width - sel.size.width).max(px(0.));
                    let maxy = (full.height - sel.size.height).max(px(0.));
                    o.x = o.x.max(px(0.)).min(maxx);
                    o.y = o.y.max(px(0.)).min(maxy);
                    sel.origin = o;
                }
            }
            Some(DragKind::Resize { handle, orig }) if ratio.is_some() => {
                let r = ratio.unwrap_or(1.0);
                let (x0, y0) = (orig.origin.x, orig.origin.y);
                let (x1, y1) = (x0 + orig.size.width, y0 + orig.size.height);
                // anchor = the fixed opposite corner / edge; edges grow the other side from it
                let (anchor, q) = match handle {
                    Handle::NW => (point(x1, y1), p),
                    Handle::NE => (point(x0, y1), p),
                    Handle::SW => (point(x1, y0), p),
                    Handle::SE => (point(x0, y0), p),
                    Handle::E => (point(x0, y0), point(p.x, y0)),
                    Handle::W => (point(x1, y0), point(p.x, y0)),
                    Handle::N => (point(x0, y1), point(x0, p.y)),
                    Handle::S => (point(x0, y0), point(x0, p.y)),
                };
                self.selection = Some(norm(anchor, ratio_corner(anchor, q, r, full)));
            }
            Some(DragKind::Resize { handle, orig }) => {
                let (mut x0, mut y0) = (orig.origin.x, orig.origin.y);
                let (mut x1, mut y1) = (x0 + orig.size.width, y0 + orig.size.height);
                match handle {
                    Handle::N => y0 = p.y,
                    Handle::S => y1 = p.y,
                    Handle::W => x0 = p.x,
                    Handle::E => x1 = p.x,
                    Handle::NW => (x0, y0) = (p.x, p.y),
                    Handle::NE => (x1, y0) = (p.x, p.y),
                    Handle::SW => (x0, y1) = (p.x, p.y),
                    Handle::SE => (x1, y1) = (p.x, p.y),
                }
                self.selection = Some(norm(point(x0, y0), point(x1, y1)));
            }
            None => {}
        }
        self.update_hover();
        self.update_magnifier(window);
        cx.notify();
    }

    pub(super) fn on_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        let drag = self.drag.take();
        match self.mode {
            Mode::Window => self.capture_window(cx),
            Mode::Area | Mode::Ocr => {
                let ok = self.selection.is_some_and(|s| s.size.width > px(3.) && s.size.height > px(3.));
                if ok {
                    let kind = CaptureKind::Area;
                    self.capture_selection(kind, cx);
                } else if matches!(drag, Some(DragKind::New { .. })) && self.mode == Mode::Area {
                    // a plain click captures the window under the cursor (like CleanShot's space toggle)
                    self.selection = None;
                    if self.hovered_window.is_some() {
                        self.capture_window(cx);
                    } else {
                        // desktop / taskbar are not listed as windows: take this monitor
                        self.capture_fullscreen(cx);
                    }
                }
            }
            Mode::AllInOne => {
                if self.selection.is_some_and(|s| s.size.width < px(3.) || s.size.height < px(3.)) {
                    self.selection = None;
                }
            }
        }
        cx.notify();
    }

    pub(super) fn on_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = e.keystroke.key.as_str();
        let step = if e.keystroke.modifiers.shift { px(10.) } else { px(1.) };
        let full = window.bounds().size;
        if let Some((dim, buf)) = self.editing.as_mut() {
            let dim = *dim;
            match k {
                "escape" => self.editing = None,
                "backspace" => {
                    buf.pop();
                }
                "enter" | "tab" => {
                    let typed = std::mem::take(buf);
                    self.editing = None;
                    self.apply_size_routed(dim, typed, full, cx);
                    if k == "tab" {
                        self.editing = Some((if dim == Dim::W { Dim::H } else { Dim::W }, String::new()));
                    }
                }
                _ if !k.is_empty() && k.len() <= 1 && k.chars().all(|c| c.is_ascii_digit()) => {
                    if buf.len() < 5 {
                        buf.push_str(k);
                    }
                }
                _ => {}
            }
            cx.notify();
            return;
        }
        match k {
            "escape" => {
                if self.all_in_one && self.mode != Mode::AllInOne && self.drag.is_none() {
                    self.set_mode(Mode::AllInOne);
                    self.sync_peers(cx);
                } else if self.mode == Mode::AllInOne && self.selection.is_some() && self.drag.is_none() {
                    self.selection = None;
                } else if self.mode == Mode::AllInOne && self.drag.is_none() {
                    // the selection may live on another monitor
                    with_selection_holder(
                        cx,
                        |v, _, _| v.selection = None,
                        |cx| close_all(cx, |_| {}),
                    );
                } else {
                    close_all(cx, |_| {});
                }
            }
            "space" => {
                let mode = match (self.mode, self.all_in_one) {
                    (Mode::Window, true) => Mode::AllInOne,
                    (_, true) => Mode::Window,
                    (Mode::Window, false) => Mode::Area,
                    (Mode::Area, false) => Mode::Window,
                    (m, false) => m,
                };
                self.set_mode(mode);
                self.update_hover();
                self.sync_peers(cx);
            }
            "enter" => {
                if self.selection.is_some() {
                    self.capture_selection(CaptureKind::Area, cx);
                } else if self.mode == Mode::Window {
                    self.capture_window(cx);
                } else {
                    // the selection may live on another monitor
                    with_selection_holder(cx, |v, _, cx| v.capture_selection(CaptureKind::Area, cx), |_| {});
                }
            }
            "left" | "right" | "up" | "down" => {
                let (dx, dy) = match k {
                    "left" => (-step, px(0.)),
                    "right" => (step, px(0.)),
                    "up" => (px(0.), -step),
                    _ => (px(0.), step),
                };
                if self.selection.is_some() {
                    self.nudge(dx, dy, full);
                } else {
                    // the selection may live on another monitor
                    with_selection_holder(cx, move |v, w, _| v.nudge(dx, dy, w.bounds().size), |_| {});
                }
            }
            "f" => self.capture_fullscreen(cx),
            "w" => {
                self.set_mode(Mode::Window);
                self.update_hover();
                self.sync_peers(cx);
            }
            _ => {}
        }
        cx.notify();
    }
}
