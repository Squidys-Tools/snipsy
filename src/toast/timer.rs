//! Self-timer countdown; the HUD is a `ToastView` in countdown mode.

use super::{close_all, open_toast, Toasts};
use crate::app::{self, state};
use crate::capture::{self, PRect};
use crate::history::CaptureKind;
use gpui::*;
use std::time::Duration;

/// Whether a self-timer countdown (or its pending capture) is in progress.
pub fn timer_running(cx: &App) -> bool {
    cx.try_global::<Toasts>().is_some_and(|t| t.timer_running)
}

/// Abort a running self-timer and remove its HUD. Returns whether one was running.
pub fn cancel_timer(cx: &mut App) -> bool {
    let t = cx.default_global::<Toasts>();
    let was = t.timer_running;
    t.timer_running = false;
    t.timer_gen += 1;
    if let Some(h) = t.countdown.take() {
        let _ = h.update(cx, |_, window, _| window.remove_window());
    }
    was
}

fn timer_alive(gen: u64, cx: &mut App) -> bool {
    let t = cx.default_global::<Toasts>();
    t.timer_running && t.timer_gen == gen
}

/// End timer `gen` early (if it is still the current one): drop its HUD and
/// bring Quick Access back.
fn end_timer(gen: u64, cx: &mut App) {
    if !timer_alive(gen, cx) {
        return;
    }
    cancel_timer(cx);
    if !state(cx).capturing {
        crate::quick::set_visible(true, cx);
    }
}

/// Self-timer: big countdown HUD, then capture `area` (or the cursor's display).
pub fn start_timer(area: Option<PRect>, cx: &mut App) {
    if state(cx).capturing || timer_running(cx) {
        return;
    }
    let secs = state(cx).settings.self_timer_secs.max(1);
    let Some(handle) = open_toast(cx, "".into(), "".into(), None, Some(secs)) else { return };
    let t = cx.default_global::<Toasts>();
    t.timer_gen += 1;
    t.timer_running = true;
    t.countdown = Some(handle.into());
    let gen = t.timer_gen;
    // keep Quick Access out of the timed shot (like overlay::start does)
    crate::quick::set_visible(false, cx);
    cx.spawn(async move |cx: &mut AsyncApp| {
        for left in (1..=secs).rev() {
            let alive = cx.update(|cx| timer_alive(gen, cx)).unwrap_or(false)
                && handle
                    .update(cx, |v, _, cx| {
                        v.countdown = Some(left);
                        cx.notify();
                    })
                    .is_ok();
            if !alive {
                let _ = cx.update(|cx| end_timer(gen, cx));
                return;
            }
            cx.background_executor().timer(Duration::from_secs(1)).await;
        }
        let go = cx
            .update(|cx| {
                if !timer_alive(gen, cx) {
                    return false;
                }
                if let Some(h) = cx.default_global::<Toasts>().countdown.take() {
                    let _ = h.update(cx, |_, window, _| window.remove_window());
                }
                close_all(cx);
                true
            })
            .unwrap_or(false);
        if !go {
            return;
        }
        // give DWM a frame to remove the HUD before grabbing pixels
        cx.background_executor().timer(Duration::from_millis(120)).await;
        let _ = cx.update(|cx| {
            if !timer_alive(gen, cx) {
                return;
            }
            cancel_timer(cx);
            if state(cx).capturing {
                return;
            }
            let rect = area.or_else(|| {
                let (x, y) = capture::cursor_pos();
                capture::monitor_at(x, y).map(|m| m.rect)
            });
            if let Some(r) = rect {
                app::capture_rect_now(r, CaptureKind::Timer, cx);
            }
            crate::quick::set_visible(true, cx);
        });
    })
    .detach();
}
