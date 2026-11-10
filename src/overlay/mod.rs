//! Full-screen capture overlay, one window per monitor.

mod geometry;
mod hud;
mod input;
mod selection;
mod shoot;
mod view;

use crate::app::{self, state, state_mut, Mode};
use crate::capture::{self, MonitorInfo, PRect, WindowInfo};
use crate::{platform, quick, toast, ui};
use geometry::{Aspect, Dim, DragKind};
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
}
