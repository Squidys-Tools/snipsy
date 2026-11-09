//! Full-screen capture overlay (one per monitor): frozen screen, crosshair,
//! pixel magnifier, area / window selection and the All-In-One HUD.

use crate::app::{self, state, state_mut, Command, Mode};
use crate::capture::{self, MonitorInfo, PRect, WindowInfo};
use crate::history::CaptureKind;
use crate::ui::{self, color, icon};
use crate::{platform, quick, toast};
use gpui::prelude::*;
use gpui::*;
use image::RgbaImage;
use std::sync::Arc;

const MAG_CELLS: u32 = 15;
const MAG_CELL: f32 = 8.0;
const HANDLE: f32 = 8.0;

pub fn start(mode: Mode, cx: &mut App) {
    if state(cx).capturing {
        return;
    }
    state_mut(cx).capturing = true;
    // A pending self-timer would fire into the middle of this session: abort it.
    let had_timer = toast::cancel_timer(cx);
    // Hide our own floating UI so it is not part of the frozen frame.
    quick::set_visible(false, cx);
    let had_toast = toast::close_all(cx);
    // removed / hidden windows need a moment before DWM stops compositing them
    let busy = state(cx).quick.is_some() || had_toast || had_timer;
    let wait = app::delay(cx, if busy { 60 } else { 0 });
    cx.spawn(async move |cx: &mut AsyncApp| {
        wait.await;
        let _ = cx.update(|cx| open_overlays(mode, cx));
    })
    .detach();
}

fn open_overlays(mode: Mode, cx: &mut App) {
    let monitors = capture::monitors();
    let windows = capture::list_windows();
    let displays = cx.displays();
    let last_area = state(cx).settings.last_area;
    let (show_mag, show_cross) = (state(cx).settings.show_magnifier, state(cx).settings.show_crosshair);
    let mut handles = vec![];
    // Only the overlay under the cursor takes keyboard focus; activating every
    // overlay in turn would leave the last monitor's one in the foreground.
    let (cur_x, cur_y) = capture::cursor_pos();
    let focus_idx = monitors.iter().position(|m| m.rect.contains(cur_x, cur_y)).unwrap_or(0);
    for (i, mon) in monitors.iter().enumerate() {
        let focus_me = i == focus_idx;
        let Ok(img) = capture::capture_rect(mon.rect) else { continue };
        let img = Arc::new(img);
        let display = displays.get(i).cloned();
        let bounds = display.as_ref().map(|d| d.bounds()).unwrap_or_else(|| {
            Bounds::new(
                point(px(mon.rect.x as f32 / mon.scale), px(mon.rect.y as f32 / mon.scale)),
                size(px(mon.rect.w as f32 / mon.scale), px(mon.rect.h as f32 / mon.scale)),
            )
        });
        let mon = mon.clone();
        let windows = windows.clone();
        let opts = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            focus: true,
            show: true,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            display_id: display.as_ref().map(|d| d.id()),
            window_background: WindowBackgroundAppearance::Opaque,
            ..Default::default()
        };
        let res = cx.open_window(opts, move |window, cx| {
            if let Some(hwnd) = platform::hwnd_of(window) {
                platform::place_window(hwnd, mon.rect, true);
                if focus_me {
                    platform::force_foreground(hwnd);
                }
            }
            // Alt+F4 (or any other WM_CLOSE) must end the whole session, not
            // just this window, or `capturing` would stay stuck.
            window.on_window_should_close(cx, |_, cx| {
                close_all(cx, |_| {});
                false
            });
            cx.new(|cx| {
                let focus = cx.focus_handle();
                window.focus(&focus);
                let scale = mon.scale;
                let mut v = OverlayView {
                    mode,
                    all_in_one: mode == Mode::AllInOne,
                    aspect: Aspect::Free,
                    editing: None,
                    monitor: mon,
                    render: ui::to_render_image(&img),
                    image: img,
                    windows,
                    focus,
                    mouse: None,
                    drag: None,
                    selection: None,
                    saved_selection: None,
                    hovered_window: None,
                    magnifier: None,
                    scale,
                    show_mag,
                    show_cross,
                };
                if mode == Mode::AllInOne {
                    if let Some((x, y, w, h)) = last_area {
                        v.selection = v.local_from_global(PRect { x, y, w, h });
                    }
                }
                v
            })
        });
        if let Ok(h) = res {
            handles.push(h.into());
        }
    }
    if handles.is_empty() {
        state_mut(cx).capturing = false;
        quick::set_visible(true, cx);
    }
    state_mut(cx).overlays = handles;
}

/// Close every overlay window and run `then` once they are gone.
fn close_all(cx: &mut App, then: impl FnOnce(&mut App) + 'static) {
    cx.defer(move |cx| {
        let handles = std::mem::take(&mut state_mut(cx).overlays);
        for h in handles {
            let _ = h.update(cx, |_, window, _| window.remove_window());
        }
        state_mut(cx).capturing = false;
        quick::set_visible(true, cx);
        then(cx);
    });
}

#[derive(Clone, Copy, PartialEq)]
enum Handle {
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
enum DragKind {
    New { start: Point<Pixels> },
    Move { offset: Point<Pixels> },
    Resize { handle: Handle, orig: Bounds<Pixels> },
}

/// All-In-One aspect-ratio presets for the selection.
#[derive(Clone, Copy, PartialEq)]
enum Aspect {
    Free,
    Square,
    Classic,
    Wide,
}

impl Aspect {
    const ALL: [Aspect; 4] = [Aspect::Free, Aspect::Square, Aspect::Classic, Aspect::Wide];

    fn ratio(self) -> Option<f32> {
        match self {
            Aspect::Free => None,
            Aspect::Square => Some(1.0),
            Aspect::Classic => Some(4.0 / 3.0),
            Aspect::Wide => Some(16.0 / 9.0),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Aspect::Free => "Free",
            Aspect::Square => "1:1",
            Aspect::Classic => "4:3",
            Aspect::Wide => "16:9",
        }
    }
}

/// Which HUD size box is being typed into.
#[derive(Clone, Copy, PartialEq)]
enum Dim {
    W,
    H,
}

/// The corner opposite `a` for a drag towards `p` locked to width/height = `r`,
/// growing along the dominant axis and shrunk to stay inside `full`.
fn ratio_corner(a: Point<Pixels>, p: Point<Pixels>, r: f32, full: Size<Pixels>) -> Point<Pixels> {
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
fn clamp_into(mut b: Bounds<Pixels>, full: Size<Pixels>) -> Bounds<Pixels> {
    b.size.width = b.size.width.min(full.width);
    b.size.height = b.size.height.min(full.height);
    b.origin.x = b.origin.x.max(px(0.)).min((full.width - b.size.width).max(px(0.)));
    b.origin.y = b.origin.y.max(px(0.)).min((full.height - b.size.height).max(px(0.)));
    b
}

/// Run `f` on the overlay (on any monitor) that holds a selection, or
/// `otherwise` if none does. Deferred, so it may be called from inside an
/// overlay's own update.
fn with_selection_holder(
    cx: &mut App,
    f: impl FnOnce(&mut OverlayView, &mut Window, &mut Context<OverlayView>) + 'static,
    otherwise: impl FnOnce(&mut App) + 'static,
) {
    cx.defer(move |cx| {
        let mut f = Some(f);
        for h in state(cx).overlays.clone() {
            let Some(h) = h.downcast::<OverlayView>() else { continue };
            let _ = h.update(cx, |v, window, cx| {
                if v.selection.is_some() {
                    if let Some(f) = f.take() {
                        f(v, window, cx);
                        cx.notify();
                    }
                }
            });
        }
        if f.is_some() {
            otherwise(cx);
        }
    });
}

pub struct OverlayView {
    mode: Mode,
    /// Opened as All-In-One: Window / OCR are sub-modes and the HUD stays.
    all_in_one: bool,
    aspect: Aspect,
    /// HUD size box being edited and the digits typed so far.
    editing: Option<(Dim, String)>,
    monitor: MonitorInfo,
    image: Arc<RgbaImage>,
    render: Arc<RenderImage>,
    windows: Vec<WindowInfo>,
    focus: FocusHandle,
    mouse: Option<Point<Pixels>>,
    drag: Option<DragKind>,
    selection: Option<Bounds<Pixels>>,
    /// All-In-One selection put aside while in the Window sub-mode, restored
    /// on returning to All-In-One.
    saved_selection: Option<Bounds<Pixels>>,
    hovered_window: Option<WindowInfo>,
    magnifier: Option<Arc<RenderImage>>,
    scale: f32,
    show_mag: bool,
    show_cross: bool,
}

fn norm(a: Point<Pixels>, b: Point<Pixels>) -> Bounds<Pixels> {
    Bounds::from_corners(point(a.x.min(b.x), a.y.min(b.y)), point(a.x.max(b.x), a.y.max(b.y)))
}

impl OverlayView {
    fn local_from_global(&self, r: PRect) -> Option<Bounds<Pixels>> {
        let r = r.intersect(&self.monitor.rect)?;
        let s = self.scale;
        Some(Bounds::new(
            point(px((r.x - self.monitor.rect.x) as f32 / s), px((r.y - self.monitor.rect.y) as f32 / s)),
            size(px(r.w as f32 / s), px(r.h as f32 / s)),
        ))
    }

    /// Selection (logical) -> physical rect relative to this monitor's image.
    fn to_image_rect(&self, b: Bounds<Pixels>) -> PRect {
        let s = self.scale;
        let x0 = (f32::from(b.origin.x) * s).round().max(0.0) as i32;
        let y0 = (f32::from(b.origin.y) * s).round().max(0.0) as i32;
        let x1 = (f32::from(b.origin.x + b.size.width) * s).round().min(self.image.width() as f32) as i32;
        let y1 = (f32::from(b.origin.y + b.size.height) * s).round().min(self.image.height() as f32) as i32;
        PRect { x: x0, y: y0, w: (x1 - x0).max(0), h: (y1 - y0).max(0) }
    }

    fn global_point(&self, p: Point<Pixels>) -> (i32, i32) {
        (
            self.monitor.rect.x + (f32::from(p.x) * self.scale) as i32,
            self.monitor.rect.y + (f32::from(p.y) * self.scale) as i32,
        )
    }

    /// Copy this overlay's mode and aspect preset to the overlays on the other monitors.
    fn sync_peers(&self, cx: &mut Context<Self>) {
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
    fn clear_peer_selections(&self, cx: &mut Context<Self>) {
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

    /// Switch mode, putting the selection aside while in the Window sub-mode and
    /// bringing it back on return to All-In-One.
    fn set_mode(&mut self, mode: Mode) {
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

    /// Apply a typed size to the overlay holding the selection (on any
    /// monitor); only when none has one, create it here.
    fn apply_size_routed(&mut self, dim: Dim, typed: String, full: Size<Pixels>, cx: &mut Context<Self>) {
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

    /// Apply a typed width or height (physical px) to the selection, creating a
    /// centred one if there is none. Keeps the aspect preset and the centre.
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

    /// Pick an aspect preset and reshape the current selection to it.
    /// (A peer holding the selection reshapes its own in `sync_peers`.)
    fn set_aspect(&mut self, aspect: Aspect, full: Size<Pixels>, cx: &mut Context<Self>) {
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

    /// Shift the selection by (dx, dy), keeping it on screen.
    fn nudge(&mut self, dx: Pixels, dy: Pixels, full: Size<Pixels>) {
        if let Some(sel) = self.selection.as_mut() {
            sel.origin.x += dx;
            sel.origin.y += dy;
            *sel = clamp_into(*sel, full);
        }
    }

    fn update_hover(&mut self) {
        if let Some(m) = self.mouse {
            let (gx, gy) = self.global_point(m);
            self.hovered_window = capture::root_window_at(gx, gy, &self.windows);
        }
    }

    fn update_magnifier(&mut self, window: &mut Window) {
        if !self.show_mag {
            return;
        }
        let Some(m) = self.mouse else { return };
        let cx0 = (f32::from(m.x) * self.scale) as i32;
        let cy0 = (f32::from(m.y) * self.scale) as i32;
        let cell = (MAG_CELL * self.scale).round().max(1.0) as u32;
        let n = MAG_CELLS;
        let side = n * cell;
        let mut out = RgbaImage::new(side, side);
        let half = (n / 2) as i32;
        for gy in 0..n {
            for gx in 0..n {
                let sx = cx0 + gx as i32 - half;
                let sy = cy0 + gy as i32 - half;
                let p = if sx >= 0 && sy >= 0 && (sx as u32) < self.image.width() && (sy as u32) < self.image.height() {
                    *self.image.get_pixel(sx as u32, sy as u32)
                } else {
                    image::Rgba([0, 0, 0, 255])
                };
                for yy in 0..cell {
                    for xx in 0..cell {
                        out.put_pixel(gx * cell + xx, gy * cell + yy, p);
                    }
                }
            }
        }
        if let Some(old) = self.magnifier.replace(ui::to_render_image(&out)) {
            let _ = window.drop_image(old);
        }
    }

    fn handle_at(&self, p: Point<Pixels>) -> Option<Handle> {
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

    fn on_down(&mut self, e: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>) {
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

    fn on_move(&mut self, e: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
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

    fn on_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
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

    fn on_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = e.keystroke.key.as_str();
        let step = if e.keystroke.modifiers.shift { px(10.) } else { px(1.) };
        let full = window.bounds().size;
        // typing into a HUD size box
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
                    // leave the Window / OCR sub-mode
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

    fn crop(&self, b: Bounds<Pixels>) -> Option<(RgbaImage, PRect)> {
        let r = self.to_image_rect(b);
        if r.w < 1 || r.h < 1 {
            return None;
        }
        let img = image::imageops::crop_imm(&*self.image, r.x as u32, r.y as u32, r.w as u32, r.h as u32).to_image();
        let global = PRect { x: r.x + self.monitor.rect.x, y: r.y + self.monitor.rect.y, w: r.w, h: r.h };
        Some((img, global))
    }

    fn capture_selection(&mut self, kind: CaptureKind, cx: &mut Context<Self>) {
        let Some(sel) = self.selection else { return };
        let Some((img, global)) = self.crop(sel) else { return };
        state_mut(cx).settings.last_area = Some((global.x, global.y, global.w, global.h));
        state(cx).settings.save();
        let ocr = self.mode == Mode::Ocr;
        close_all(cx, move |cx| {
            if ocr {
                app::recognize_and_copy(Arc::new(img), cx);
            } else {
                app::finish(img, kind, cx);
            }
        });
    }

    fn capture_fullscreen(&mut self, cx: &mut Context<Self>) {
        let img = (*self.image).clone();
        close_all(cx, move |cx| app::finish(img, CaptureKind::Fullscreen, cx));
    }

    fn capture_window(&mut self, cx: &mut Context<Self>) {
        let Some(info) = self.hovered_window.clone() else { return };
        let scale = self.scale;
        let fallback = self.local_from_global(info.rect).and_then(|b| self.crop(b)).map(|c| c.0);
        let (shadow, background) = (state(cx).settings.window_shadow, state(cx).settings.window_background);
        // window_padding is in logical px around the window
        let pad_px = state(cx).settings.window_padding as f32 * scale;
        close_all(cx, move |cx| {
            let raw = capture::capture_window(&info).ok().or(fallback);
            let Some(mut img) = raw else {
                toast::show(cx, "Capture failed", "Could not capture that window");
                return;
            };
            capture::round_corners(&mut img, 8.0 * scale);
            let img = if background {
                // Backdrop padding is a fraction of the shorter image side.
                let padding = pad_px / img.width().min(img.height()).max(1) as f32;
                let mut doc = crate::annotate::doc::Doc::new(Arc::new(img));
                doc.backdrop.enabled = true;
                doc.backdrop.radius = 0.0;
                doc.backdrop.shadow = true;
                doc.backdrop.padding = padding;
                crate::annotate::render::render(&doc)
            } else if shadow {
                crate::annotate::render::window_shadow(&img, scale)
            } else {
                img
            };
            app::finish(img, CaptureKind::Window, cx);
        });
    }

    fn hud(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let item = |id: &'static str, icon_path: &'static str, label: &'static str, active: bool| {
            div()
                .id(id)
                .w(px(68.))
                .h(px(54.))
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(4.))
                .rounded(px(9.))
                .cursor_pointer()
                .when(active, |d| d.bg(color::selected()))
                .when(!active, |d| d.hover(|s| s.bg(color::hover())))
                .child(icon(icon_path, px(20.), color::text()))
                .child(div().text_size(px(12.)).text_color(color::text_dim()).child(label))
        };
        let sel = self.selection.map(|s| self.to_image_rect(s));
        let has_sel = sel.is_some();
        let editing = self.editing.clone();
        // click a box to type an exact size (physical px); Enter applies, Tab switches
        let size_box = |id: &'static str, dim: Dim, v: String| {
            let typing = editing.as_ref().filter(|(d, _)| *d == dim).map(|(_, b)| b.clone());
            let active = typing.is_some();
            let (text, faint) = match typing {
                Some(b) if b.is_empty() => (v, true),
                Some(b) => (format!("{b}|"), false),
                None => (v, false),
            };
            div()
                .id(id)
                .w(px(58.))
                .h(px(28.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(7.))
                .bg(rgba(0xFFFFFF14))
                .border_1()
                .border_color(if active { color::accent().into() } else { rgba(0xFFFFFF14) })
                .text_size(px(14.))
                .text_color(if faint { color::text_faint() } else { color::text() })
                .cursor(CursorStyle::IBeam)
                .child(text)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.editing = Some((dim, String::new()));
                    window.focus(&this.focus);
                    cx.notify();
                }))
        };
        let aspect_btn = |a: Aspect| {
            let active = self.aspect == a;
            div()
                .id(("hud-aspect", a as usize))
                .h(px(28.))
                .px(px(8.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(7.))
                .text_size(px(12.))
                .text_color(if active { color::text() } else { color::text_dim() })
                .cursor_pointer()
                .when(active, |d| d.bg(color::selected()))
                .when(!active, |d| d.hover(|s| s.bg(color::hover())))
                .child(a.label())
                .on_click(cx.listener(move |this, _, window, cx| this.set_aspect(a, window.bounds().size, cx)))
        };
        // Panels (not the full-width row) swallow presses so they never start a
        // selection drag nor eat the mouse-up that ends one.
        let panel = || {
            div()
                .flex()
                .items_center()
                .p(px(6.))
                .gap(px(2.))
                .rounded(px(14.))
                .bg(color::hud())
                .border_1()
                .border_color(color::hud_border())
                .shadow_lg()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        };
        let sep = || div().w(px(1.)).h(px(32.)).mx(px(4.)).bg(color::separator());
        div()
            .absolute()
            .bottom(px(56.))
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .gap(px(12.))
            .font_family(ui::FONT)
            .child(
                panel()
                    .child(item("hud-area", "icons/area.svg", "Area", self.mode == Mode::AllInOne).on_click(cx.listener(|this, _, _, cx| {
                        if this.selection.is_some() && this.mode == Mode::AllInOne {
                            this.capture_selection(CaptureKind::Area, cx);
                        } else {
                            this.set_mode(Mode::AllInOne);
                            this.sync_peers(cx);
                            cx.notify();
                        }
                    })))
                    .child(item("hud-full", "icons/fullscreen.svg", "Fullscreen", false).on_click(cx.listener(|this, _, _, cx| this.capture_fullscreen(cx))))
                    .child(item("hud-window", "icons/window.svg", "Window", self.mode == Mode::Window).on_click(cx.listener(|this, _, _, cx| {
                        this.set_mode(Mode::Window);
                        this.update_hover();
                        this.sync_peers(cx);
                        cx.notify();
                    })))
                    .child(item("hud-timer", "icons/timer.svg", "Timer", false).on_click(cx.listener(|this, _, _, cx| {
                        let area = this.selection.and_then(|s| this.crop(s)).map(|c| c.1);
                        close_all(cx, move |cx| app::run(Command::SelfTimer(area), cx));
                    })))
                    .child(sep())
                    .child(item("hud-ocr", "icons/ocr.svg", "OCR", self.mode == Mode::Ocr).on_click(cx.listener(|this, _, _, cx| {
                        if this.selection.is_some() {
                            this.mode = Mode::Ocr;
                            this.capture_selection(CaptureKind::Area, cx);
                        } else {
                            this.mode = Mode::Ocr;
                            this.sync_peers(cx);
                            cx.notify();
                        }
                    }))),
            )
            .child(
                panel()
                    .px(px(14.))
                    .gap(px(8.))
                    .children(Aspect::ALL.map(aspect_btn))
                    .child(sep())
                    .child(size_box("hud-w", Dim::W, sel.map(|r| r.w.to_string()).unwrap_or("—".into())))
                    .child(div().text_size(px(12.)).text_color(color::text_faint()).child("×"))
                    .child(size_box("hud-h", Dim::H, sel.map(|r| r.h.to_string()).unwrap_or("—".into())))
                    .child(sep())
                    .child(
                        ui::button("hud-capture", "Capture", true)
                            .when(!has_sel, |d| d.opacity(0.4))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.selection.is_some() {
                                    this.capture_selection(CaptureKind::Area, cx);
                                } else {
                                    with_selection_holder(cx, |v, _, cx| v.capture_selection(CaptureKind::Area, cx), |_| {});
                                }
                            })),
                    )
                    .child(ui::button("hud-cancel", "Cancel", false).on_click(cx.listener(|_, _, _, cx| close_all(cx, |_| {})))),
            )
    }
}

impl Render for OverlayView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let size = window.bounds().size;
        let (gx, gy) = capture::cursor_pos();
        let cursor_here = self.monitor.rect.contains(gx, gy);
        let mouse = self.mouse.filter(|_| cursor_here);
        let selection = self.selection;
        let dragging_new = matches!(self.drag, Some(DragKind::New { .. }));
        let window_rect = if self.mode == Mode::Window {
            self.hovered_window.as_ref().and_then(|w| self.local_from_global(w.rect))
        } else {
            None
        };
        let all_in_one = self.mode == Mode::AllInOne;
        let show_cross = self.show_cross && self.mode != Mode::Window && (selection.is_none() || dragging_new);
        let dim_all = self.mode != Mode::Window && selection.is_none() && all_in_one;

        let geometry = canvas(
            |_, _, _| {},
            move |bounds, _, window, _| {
                let dim = rgba(0x00000066);
                let full = bounds;
                if let Some(s) = selection {
                    // dim outside the selection
                    let (x0, y0) = (s.origin.x, s.origin.y);
                    let (x1, y1) = (x0 + s.size.width, y0 + s.size.height);
                    let r = |a: Point<Pixels>, b: Point<Pixels>| Bounds::from_corners(a, b);
                    for b in [
                        r(full.origin, point(full.size.width, y0)),
                        r(point(px(0.), y1), point(full.size.width, full.size.height)),
                        r(point(px(0.), y0), point(x0, y1)),
                        r(point(x1, y0), point(full.size.width, y1)),
                    ] {
                        if b.size.width > px(0.) && b.size.height > px(0.) {
                            window.paint_quad(fill(b, dim));
                        }
                    }
                    window.paint_quad(outline(s.dilate(px(1.)), rgba(0x00000066), BorderStyle::Solid));
                    window.paint_quad(outline(s, rgba(0xFFFFFFE6), BorderStyle::Solid));
                    if all_in_one && !dragging_new {
                        let (mx, my) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                        for (hx, hy) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1), (mx, y0), (mx, y1), (x0, my), (x1, my)] {
                            let hb = Bounds::new(point(hx - px(4.), hy - px(4.)), gpui::size(px(8.), px(8.)));
                            window.paint_quad(
                                quad(hb, px(4.), rgb(0xFFFFFF), px(1.), rgba(0x00000080), BorderStyle::Solid),
                            );
                        }
                    }
                } else if dim_all {
                    window.paint_quad(fill(full, rgba(0x00000033)));
                }
                if let Some(w) = window_rect {
                    window.paint_quad(fill(w, rgba(0x0A84FF33)));
                    window.paint_quad(quad(w, px(0.), transparent_black(), px(3.), color::accent(), BorderStyle::Solid));
                }
                if show_cross {
                    if let Some(m) = mouse {
                        let line = rgba(0x00000073);
                        let line2 = rgba(0xFFFFFF59);
                        window.paint_quad(fill(Bounds::new(point(px(0.), m.y), gpui::size(full.size.width, px(1.))), line));
                        window.paint_quad(fill(Bounds::new(point(m.x, px(0.)), gpui::size(px(1.), full.size.height)), line));
                        window.paint_quad(fill(Bounds::new(point(px(0.), m.y + px(1.)), gpui::size(full.size.width, px(1.))), line2));
                        window.paint_quad(fill(Bounds::new(point(m.x + px(1.), px(0.)), gpui::size(px(1.), full.size.height)), line2));
                    }
                }
            },
        )
        .absolute()
        .size_full();

        // floating labels
        let mut labels: Vec<AnyElement> = vec![];
        let pill = |text: String| {
            div()
                .px(px(7.))
                .py(px(2.))
                .rounded(px(6.))
                .bg(rgba(0x000000B3))
                .text_color(rgb(0xFFFFFF))
                .text_size(px(11.))
                .font_family(ui::FONT)
                .whitespace_nowrap()
                .child(text)
        };
        if let Some(s) = selection {
            let r = self.to_image_rect(s);
            let below = s.origin.y + s.size.height + px(8.);
            let y = if below + px(24.) > size.height { s.origin.y - px(26.) } else { below };
            labels.push(
                div()
                    .absolute()
                    .left(s.origin.x + s.size.width / 2.0 - px(50.))
                    .top(y)
                    .w(px(100.))
                    .flex()
                    .justify_center()
                    .child(pill(format!("{} × {}", r.w, r.h)))
                    .into_any_element(),
            );
        }
        if let (Some(w), Some(info)) = (window_rect, self.hovered_window.as_ref()) {
            let title = if info.title.len() > 48 { format!("{}…", &info.title.chars().take(48).collect::<String>()) } else { info.title.clone() };
            labels.push(
                div()
                    .absolute()
                    .left(w.origin.x)
                    .top(w.origin.y)
                    .w(w.size.width)
                    .h(w.size.height)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap(px(4.))
                            .px(px(14.))
                            .py(px(8.))
                            .rounded(px(10.))
                            .bg(rgba(0x000000A6))
                            .text_color(rgb(0xFFFFFF))
                            .font_family(ui::FONT)
                            .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).child(title))
                            .child(div().text_size(px(11.)).text_color(color::text_dim()).child(format!("{} × {}", info.rect.w, info.rect.h))),
                    )
                    .into_any_element(),
            );
        }
        if let Some(m) = mouse.filter(|_| show_cross || self.mode == Mode::Window) {
            let (px_, py_) = ((f32::from(m.x) * self.scale) as i32, (f32::from(m.y) * self.scale) as i32);
            if self.mode != Mode::Window {
                labels.push(
                    div()
                        .absolute()
                        .left(m.x + px(8.))
                        .top(m.y + px(6.))
                        .flex()
                        .flex_col()
                        .child(pill(format!("{px_}\n{py_}").replace('\n', " · ")))
                        .into_any_element(),
                );
            }
            if self.show_mag && self.mode != Mode::Window {
                if let Some(mag) = self.magnifier.clone() {
                    let side = px(MAG_CELLS as f32 * MAG_CELL);
                    let mut ox = m.x + px(24.);
                    let mut oy = m.y + px(30.);
                    if ox + side > size.width {
                        ox = m.x - side - px(24.);
                    }
                    if oy + side > size.height {
                        oy = m.y - side - px(30.);
                    }
                    let c = px((MAG_CELLS / 2) as f32 * MAG_CELL);
                    let band = rgba(0x00000059);
                    labels.push(
                        div()
                            .absolute()
                            .left(ox)
                            .top(oy)
                            .size(side)
                            .rounded(px(12.))
                            .shadow_lg()
                            .child(img(mag).absolute().size(side).rounded(px(12.)))
                            .child(div().absolute().left(c).top_0().w(px(MAG_CELL)).h(c).bg(band))
                            .child(div().absolute().left(c).top(c + px(MAG_CELL)).w(px(MAG_CELL)).h(c).bg(band))
                            .child(div().absolute().top(c).left_0().h(px(MAG_CELL)).w(c).bg(band))
                            .child(div().absolute().top(c).left(c + px(MAG_CELL)).h(px(MAG_CELL)).w(c).bg(band))
                            .child(div().absolute().left(c).top(c).size(px(MAG_CELL)).border_1().border_color(rgb(0xFFFFFF)))
                            .child(div().absolute().size(side).rounded(px(12.)).border_2().border_color(rgba(0xFFFFFFCC)))
                            .into_any_element(),
                    );
                }
            }
        }
        // mode hint at the top
        let hint = match self.mode {
            Mode::Area => "Drag to capture an area  ·  Click a window  ·  Space: window mode  ·  Esc: cancel",
            Mode::Window if self.all_in_one => "Click a window to capture it  ·  Space or Esc: back to All-In-One",
            Mode::Window => "Click a window to capture it  ·  Space: area mode  ·  Esc: cancel",
            Mode::Ocr if self.all_in_one => "Select an area to copy its text  ·  Esc: back to All-In-One",
            Mode::Ocr => "Select an area to copy its text  ·  Esc: cancel",
            Mode::AllInOne => "Drag to select  ·  Enter or double-click to capture  ·  Arrows nudge  ·  Esc: cancel",
        };
        let show_hint = cursor_here && self.drag.is_none() && selection.is_none();

        div()
            .size_full()
            .relative()
            .track_focus(&self.focus)
            .cursor(if self.mode == Mode::Window { CursorStyle::PointingHand } else { CursorStyle::Crosshair })
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_down))
            .on_mouse_move(cx.listener(Self::on_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_up))
            .on_mouse_down(MouseButton::Right, cx.listener(|_, _, _, cx| close_all(cx, |_| {})))
            .child(img(self.render.clone()).absolute().size_full())
            .child(geometry)
            .children(labels)
            .when(show_hint, |d| {
                d.child(
                    div().absolute().top(px(28.)).left_0().right_0().flex().justify_center().child(
                        div()
                            .px(px(14.))
                            .py(px(7.))
                            .rounded(px(10.))
                            .bg(color::hud())
                            .border_1()
                            .border_color(color::hud_border())
                            .text_size(px(12.))
                            .font_family(ui::FONT)
                            .text_color(color::text_dim())
                            .child(hint),
                    ),
                )
            })
            // keyed on how the overlay was opened, so the HUD survives the Window / OCR sub-modes
            .when(self.all_in_one && self.drag.is_none() && (cursor_here || selection.is_some()), |d| {
                d.child(self.hud(cx))
            })
    }
}
