use super::geometry::{clamp_into, norm, ratio_corner, Aspect, Dim, Handle};
use super::{with_selection_holder, OverlayView, HANDLE};
use crate::app::{state, Mode};
use crate::capture;
use gpui::*;

impl OverlayView {
    pub(super) fn sync_peers(&self, cx: &mut Context<Self>) {
        let (me, mode, aspect) = (self.monitor.rect, self.mode, self.aspect);
        cx.defer(move |cx| {
            for h in state(cx).overlays.clone() {
                let Some(h) = h.downcast::<OverlayView>() else { continue };
                let _ = h.update(cx, |v, window, cx| {
                    if v.monitor.rect == me || (v.mode == mode && v.aspect == aspect) {
                        return;
                    }
                    v.set_mode(mode);
                    if v.aspect != aspect {
                        v.aspect = aspect;
                        v.reshape_to_aspect(window.bounds().size);
                    }
                    v.update_hover();
                    cx.notify();
                });
            }
        });
    }

    /// A new selection was started here: drop the ones on other monitors so
    /// Enter / Esc / Capture act on exactly one.
    pub(super) fn clear_peer_selections(&self, cx: &mut Context<Self>) {
        let me = self.monitor.rect;
        cx.defer(move |cx| {
            for h in state(cx).overlays.clone() {
                let Some(h) = h.downcast::<OverlayView>() else { continue };
                let _ = h.update(cx, |v, _, cx| {
                    if v.monitor.rect != me && (v.selection.is_some() || v.editing.is_some()) {
                        v.selection = None;
                        v.editing = None;
                        cx.notify();
                    }
                });
            }
        });
    }

    pub(super) fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
        if mode == Mode::Window {
            if self.selection.is_some() {
                self.saved_selection = self.selection.take();
            }
            self.editing = None;
        } else if mode == Mode::AllInOne && self.selection.is_none() {
            self.selection = self.saved_selection.take();
        }
    }

    pub(super) fn apply_size_routed(&mut self, dim: Dim, typed: String, full: Size<Pixels>, cx: &mut Context<Self>) {
        if self.selection.is_some() {
            self.apply_size(dim, &typed, full);
            return;
        }
        let me = cx.weak_entity();
        let typed2 = typed.clone();
        with_selection_holder(
            cx,
            move |v, w, _| v.apply_size(dim, &typed, w.bounds().size),
            move |cx| {
                let _ = me.update(cx, |v, cx| {
                    v.apply_size(dim, &typed2, full);
                    v.clear_peer_selections(cx);
                    cx.notify();
                });
            },
        );
    }

    fn apply_size(&mut self, dim: Dim, typed: &str, full: Size<Pixels>) {
        let Ok(v) = typed.parse::<u32>() else { return };
        if v == 0 {
            return;
        }
        let s = self.scale;
        let (fw, fh) = (f32::from(full.width), f32::from(full.height));
        let cur = self
            .selection
            .unwrap_or_else(|| Bounds::new(point(px(fw / 4.0), px(fh / 4.0)), size(px(fw / 2.0), px(fh / 2.0))));
        let (mut w, mut h) = (f32::from(cur.size.width), f32::from(cur.size.height));
        let val = v as f32 / s;
        let ratio = self.aspect.ratio();
        match dim {
            Dim::W => {
                w = val;
                if let Some(r) = ratio {
                    h = w / r;
                }
            }
            Dim::H => {
                h = val;
                if let Some(r) = ratio {
                    w = h * r;
                }
            }
        }
        if ratio.is_some() && (w > fw || h > fh) {
            let k = (fw / w).min(fh / h);
            w *= k;
            h *= k;
        }
        let (w, h) = (w.min(fw), h.min(fh));
        let c = cur.center();
        // snap the origin to the physical pixel grid so the crop is exactly the typed size
        let snap = |v: f32| (v * s).round() / s;
        let x = snap((f32::from(c.x) - w / 2.0).clamp(0.0, (fw - w).max(0.0)));
        let y = snap((f32::from(c.y) - h / 2.0).clamp(0.0, (fh - h).max(0.0)));
        self.selection = Some(clamp_into(Bounds::new(point(px(x), px(y)), size(px(w), px(h))), full));
    }

    /// A peer holding the selection reshapes its own in `sync_peers`.
    pub(super) fn set_aspect(&mut self, aspect: Aspect, full: Size<Pixels>, cx: &mut Context<Self>) {
        self.aspect = aspect;
        self.reshape_to_aspect(full);
        self.sync_peers(cx);
        cx.notify();
    }

    fn reshape_to_aspect(&mut self, full: Size<Pixels>) {
        if let (Some(r), Some(sel)) = (self.aspect.ratio(), self.selection) {
            let corner = ratio_corner(sel.origin, point(sel.origin.x + sel.size.width, sel.origin.y), r, full);
            self.selection = Some(norm(sel.origin, corner));
        }
    }

    pub(super) fn nudge(&mut self, dx: Pixels, dy: Pixels, full: Size<Pixels>) {
        if let Some(sel) = self.selection.as_mut() {
            sel.origin.x += dx;
            sel.origin.y += dy;
            *sel = clamp_into(*sel, full);
        }
    }

    pub(super) fn update_hover(&mut self) {
        if let Some(m) = self.mouse {
            let (gx, gy) = self.global_point(m);
            self.hovered_window = capture::root_window_at(gx, gy, &self.windows);
        }
    }

    pub(super) fn handle_at(&self, p: Point<Pixels>) -> Option<Handle> {
        let b = self.selection?;
        let (x0, y0) = (f32::from(b.origin.x), f32::from(b.origin.y));
        let (x1, y1) = (x0 + f32::from(b.size.width), y0 + f32::from(b.size.height));
        let (mx, my) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        let (px_, py_) = (f32::from(p.x), f32::from(p.y));
        let near = |ax: f32, ay: f32| (px_ - ax).abs() <= HANDLE && (py_ - ay).abs() <= HANDLE;
        [
            (x0, y0, Handle::NW),
            (x1, y0, Handle::NE),
            (x0, y1, Handle::SW),
            (x1, y1, Handle::SE),
            (mx, y0, Handle::N),
            (mx, y1, Handle::S),
            (x0, my, Handle::W),
            (x1, my, Handle::E),
        ]
        .into_iter()
        .find(|(ax, ay, _)| near(*ax, *ay))
        .map(|(_, _, h)| h)
    }
}
