//! Quick Access Overlay: a stack of capture thumbnails in a screen corner.
//! Hover a card for Copy / Save and corner actions (pin, close, annotate, OCR);
//! drag a card into any app to drop the PNG file; click to annotate.

use crate::app::{self, state, state_mut, Capture};
use crate::capture::{self, PRect};
use crate::settings::OverlayCorner;
use crate::ui::{self, color, icon, Icon};
use crate::{annotate, pin, platform, toast};
use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;

const MAX_CARDS: usize = 5;
const CARD_W: f32 = 236.0;
const GAP: f32 = 12.0;
const MARGIN: f32 = 18.0;
const PAD: f32 = 14.0;
/// How many dismissed captures "Restore Recently Closed" keeps (each holds a full image).
const MAX_RECENT: usize = 10;

struct Card {
    capture: Capture,
    _auto_close: Option<Task<()>>,
}

pub struct QuickAccess {
    cards: Vec<Card>,
    hovered: Option<u64>,
    press: Option<(u64, Point<Pixels>)>,
    visible: bool,
    /// Monitor the stack lives on (the one holding the cursor at the last capture).
    monitor: Option<capture::MonitorInfo>,
}

fn remember_closed(c: Capture, cx: &mut App) {
    let rc = &mut state_mut(cx).recently_closed;
    rc.push(c);
    if rc.len() > MAX_RECENT {
        let n = rc.len() - MAX_RECENT;
        rc.drain(..n);
    }
}

/// The monitor under the cursor (where the capture was just taken), else the primary one.
fn target_monitor() -> Option<capture::MonitorInfo> {
    let (x, y) = capture::cursor_pos();
    capture::monitor_at(x, y).or_else(capture::primary_monitor)
}

fn ui_scale(cx: &App) -> f32 {
    state(cx).settings.overlay_scale
}

fn card_h(c: &Capture, k: f32) -> f32 {
    let (w, h) = (c.image.width() as f32, c.image.height() as f32);
    (CARD_W * h / w.max(1.0)).clamp(96.0, 176.0) * k
}

pub fn push(capture: Capture, cx: &mut App) {
    let Some(m) = target_monitor() else { return };
    // a handle whose window is gone (closed behind our back) is useless: forget it
    if let Some(h) = state(cx).quick {
        if h.update(cx, |_, _, _| ()).is_err() {
            state_mut(cx).quick = None;
        }
    }
    let handle = match state(cx).quick {
        Some(h) => h,
        None => {
            let rect = PRect { x: m.work.x, y: m.work.y + m.work.h - 10, w: 10, h: 10 };
            let mon = m.clone();
            let Some(h) = toast::open_hud(cx, rect, m.scale, move |_, _| QuickAccess {
                cards: vec![],
                hovered: None,
                press: None,
                visible: true,
                monitor: Some(mon),
            }) else {
                return;
            };
            state_mut(cx).quick = Some(h);
            h
        }
    };
    let auto = state(cx).settings.overlay_auto_close.then(|| state(cx).settings.overlay_auto_close_secs);
    let id = capture.id;
    let _ = handle.update(cx, |q, window, cx| {
        let task = auto.map(|secs| {
            cx.spawn_in(window, async move |this, cx| {
                loop {
                    cx.background_executor().timer(std::time::Duration::from_secs(secs as u64)).await;
                    let keep = this
                        .update(cx, |q, cx| {
                            if q.hovered == Some(id) {
                                return true;
                            }
                            q.dismiss(id, cx);
                            false
                        })
                        .unwrap_or(false);
                    if !keep {
                        break;
                    }
                }
            })
        });
        q.cards.push(Card { capture, _auto_close: task });
        while q.cards.len() > MAX_CARDS {
            let old = q.cards.remove(0);
            remember_closed(old.capture, cx);
        }
        q.monitor = Some(m);
        q.visible = true;
        q.relayout(window, cx);
        cx.notify();
    });
}

pub fn set_file(id: u64, file: PathBuf, cx: &mut App) {
    if let Some(h) = state(cx).quick {
        let _ = h.update(cx, |q, _, _| {
            if let Some(c) = q.cards.iter_mut().find(|c| c.capture.id == id) {
                c.capture.file = Some(file);
            }
        });
    }
}

pub fn set_visible(visible: bool, cx: &mut App) {
    let hidden_by_user = state(cx).overlays_hidden;
    if let Some(h) = state(cx).quick {
        let res = h.update(cx, |q, window, _| {
            if q.cards.is_empty() {
                // nothing to show: close it instead of leaving a hidden HUD around
                window.remove_window();
                return false;
            }
            if let Some(hwnd) = platform::hwnd_of(window) {
                if visible && !hidden_by_user && q.visible {
                    platform::show_no_activate(hwnd);
                } else {
                    platform::hide_window(hwnd);
                }
            }
            true
        });
        // window gone (or just closed because it was empty): drop the handle
        if !res.unwrap_or(false) {
            state_mut(cx).quick = None;
        }
    }
}

pub fn toggle_hidden(cx: &mut App) {
    let st = state_mut(cx);
    st.overlays_hidden = !st.overlays_hidden;
    set_visible(true, cx);
}

impl QuickAccess {
    fn relayout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(hwnd) = platform::hwnd_of(window) else { return };
        if self.cards.is_empty() {
            return;
        }
        let k = ui_scale(cx);
        let corner = state(cx).settings.overlay_corner;
        let Some(m) = self.monitor.clone().or_else(capture::primary_monitor) else { return };
        let s = m.scale;
        let h_logical: f32 = self.cards.iter().map(|c| card_h(&c.capture, k)).sum::<f32>() + GAP * (self.cards.len() as f32 - 1.0) + PAD * 2.0;
        let w_logical = CARD_W * k + PAD * 2.0;
        let (w, h) = ((w_logical * s) as i32, (h_logical * s) as i32);
        let mg = ((MARGIN - PAD) * s) as i32;
        let x = match corner {
            OverlayCorner::BottomLeft | OverlayCorner::TopLeft => m.work.x + mg,
            _ => m.work.x + m.work.w - w - mg,
        };
        let y = match corner {
            OverlayCorner::TopLeft | OverlayCorner::TopRight => m.work.y + mg,
            _ => m.work.y + m.work.h - h - mg,
        };
        let r = PRect { x, y, w, h };
        let cur = platform::window_rect(hwnd);
        let same_monitor = capture::monitor_at(cur.x + cur.w / 2, cur.y + cur.h / 2).is_some_and(|c| c.rect == m.rect);
        let hide = state(cx).overlays_hidden || state(cx).capturing || toast::timer_running(cx);
        platform::place_window_ex(hwnd, r, true, !hide);
        if !same_monitor {
            // Crossing to a monitor with another DPI makes gpui apply the
            // system-suggested rect (WM_DPICHANGED); now that the window is on
            // the target monitor, set the exact rect again.
            platform::place_window_ex(hwnd, r, true, !hide);
        }
        if hide {
            platform::hide_window(hwnd);
        }
    }

    fn dismiss(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(i) = self.cards.iter().position(|c| c.capture.id == id) {
            let card = self.cards.remove(i);
            remember_closed(card.capture, cx);
        }
        self.after_change(cx);
    }

    /// Remove without adding to "recently closed" (e.g. moved into the editor).
    fn take(&mut self, id: u64, cx: &mut Context<Self>) -> Option<Capture> {
        let i = self.cards.iter().position(|c| c.capture.id == id)?;
        let c = self.cards.remove(i).capture;
        self.after_change(cx);
        Some(c)
    }

    fn after_change(&mut self, cx: &mut Context<Self>) {
        self.hovered = None;
        if self.cards.is_empty() {
            if let Some(h) = state_mut(cx).quick.take() {
                cx.defer(move |cx| {
                    let _ = h.update(cx, |_, window, _| window.remove_window());
                });
            }
        } else {
            cx.defer(|cx| {
                if let Some(h) = state(cx).quick {
                    let _ = h.update(cx, |q, window, cx| q.relayout(window, cx));
                }
            });
        }
        cx.notify();
    }

    fn capture(&self, id: u64) -> Option<Capture> {
        self.cards.iter().find(|c| c.capture.id == id).map(|c| c.capture.clone())
    }

    fn start_drag(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let Some(c) = self.capture(id) else { return };
        let prefix = state(cx).settings.file_prefix.clone();
        let Some(hwnd) = platform::hwnd_of(window) else { return };
        let hwnd_raw = hwnd.0 as isize;
        self.press = None;
        // Run the modal OLE drag loop outside of GPUI's event dispatch.
        cx.spawn(async move |_, _| {
            if let Some(path) = app::temp_file_for(&c, &prefix) {
                let _ = platform::drag_file_out(windows::Win32::Foundation::HWND(hwnd_raw as _), &path);
            }
        })
        .detach();
    }
}

fn corner_button(id: (&'static str, u64), path: Icon) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(26.))
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba(0x1E1E20D0))
        .border_1()
        .border_color(rgba(0xFFFFFF26))
        .cursor_pointer()
        .hover(|s| s.bg(rgba(0x3A3A3CF0)))
        .child(icon(path, px(13.), rgb(0xFFFFFF)))
}

impl Render for QuickAccess {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let k = ui_scale(cx);
        let top_anchor = matches!(state(cx).settings.overlay_corner, OverlayCorner::TopLeft | OverlayCorner::TopRight);
        let mut cards: Vec<AnyElement> = vec![];
        let order: Vec<usize> = if top_anchor { (0..self.cards.len()).rev().collect() } else { (0..self.cards.len()).collect() };
        for i in order {
            let c = &self.cards[i].capture;
            let id = c.id;
            let hovered = self.hovered == Some(id);
            let h = card_h(c, k);
            let card = div()
                .id(("card", id))
                .w(px(CARD_W * k))
                .h(px(h))
                .flex_none()
                .relative()
                .rounded(px(12.))
                .overflow_hidden()
                .border_1()
                .border_color(rgba(0xFFFFFF33))
                .bg(color::window_bg())
                .shadow_lg()
                .cursor_pointer()
                .on_hover(cx.listener(move |this, hov: &bool, _, cx| {
                    if *hov {
                        this.hovered = Some(id);
                    } else if this.hovered == Some(id) {
                        this.hovered = None;
                    }
                    cx.notify();
                }))
                .on_mouse_down(MouseButton::Left, cx.listener(move |this, e: &MouseDownEvent, _, _| {
                    this.press = Some((id, e.position));
                }))
                .on_mouse_move(cx.listener(move |this, e: &MouseMoveEvent, window, cx| {
                    if let Some((pid, p)) = this.press {
                        if pid == id && e.pressed_button == Some(MouseButton::Left) {
                            let d = e.position - p;
                            if f32::from(d.x).abs() + f32::from(d.y).abs() > 6.0 {
                                this.start_drag(id, window, cx);
                            }
                        }
                    }
                }))
                .on_mouse_up(MouseButton::Right, cx.listener(move |this, _, _, cx| this.dismiss(id, cx)))
                .on_click(cx.listener(move |this, e: &ClickEvent, _, cx| {
                    if this.press.take().is_some() || e.standard_click() {
                        if let Some(c) = this.take(id, cx) {
                            cx.defer(move |cx| annotate::open(c, cx));
                        }
                    }
                }))
                .child(img(c.preview.clone()).absolute().size_full().object_fit(ObjectFit::Cover))
                .when(hovered, |d| {
                    d.child(
                        div()
                            .absolute()
                            .size_full()
                            .bg(rgba(0x0000004D))
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(px(8.))
                            .child(ui::pill_button(("copy", id), "Copy").on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                if let Some(c) = this.capture(id) {
                                    this.take(id, cx);
                                    cx.defer(move |cx| app::copy_capture(&c, cx));
                                }
                            })))
                            .child(ui::pill_button(("save", id), "Save").on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                if let Some(c) = this.capture(id) {
                                    this.take(id, cx);
                                    cx.defer(move |cx| app::save_to_export(&c, cx));
                                }
                            })))
                            .child(
                                corner_button(("close", id), Icon::Close)
                                    .absolute()
                                    .top(px(8.))
                                    .right(px(8.))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.dismiss(id, cx);
                                    })),
                            )
                            .child(
                                corner_button(("pin", id), Icon::Pin)
                                    .absolute()
                                    .top(px(8.))
                                    .left(px(8.))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        if let Some(c) = this.take(id, cx) {
                                            cx.defer(move |cx| pin::open(c, cx));
                                        }
                                    })),
                            )
                            .child(
                                corner_button(("edit", id), Icon::Edit)
                                    .absolute()
                                    .bottom(px(8.))
                                    .left(px(8.))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        if let Some(c) = this.take(id, cx) {
                                            cx.defer(move |cx| annotate::open(c, cx));
                                        }
                                    })),
                            )
                            .child(
                                corner_button(("ocr", id), Icon::Ocr)
                                    .absolute()
                                    .bottom(px(8.))
                                    .right(px(8.))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        if let Some(c) = this.capture(id) {
                                            cx.defer(move |cx| app::recognize_and_copy(c.image.clone(), cx));
                                        }
                                    })),
                            )
                            .child(
                                div()
                                    .absolute()
                                    .bottom(px(13.))
                                    .left(px(40.))
                                    .right(px(40.))
                                    .flex()
                                    .justify_center()
                                    .text_size(px(10.))
                                    .text_color(rgba(0xFFFFFFB3))
                                    .child(format!("{} × {}", c.image.width(), c.image.height())),
                            ),
                    )
                })
                .into_any_element();
            cards.push(card);
        }
        div()
            .size_full()
            .p(px(PAD))
            .flex()
            .flex_col()
            .gap(px(GAP))
            .font_family(ui::FONT)
            .children(cards)
    }
}
