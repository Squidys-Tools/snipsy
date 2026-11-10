//! Floating Screenshots: always-on-top pinned captures.
//! Drag to move · scroll to resize · Ctrl+scroll for opacity · arrows nudge ·
//! L = Lock Mode (click-through) · Ctrl+C copy · Esc close.

use crate::app::{self, state, state_mut, Capture};
use crate::capture::{self, PRect};
use crate::ui::{self, color, icon, Icon};
use crate::{annotate, platform};
use gpui::prelude::*;
use gpui::*;

const BORDER: f32 = 1.0;

pub struct PinView {
    capture: Capture,
    full: std::sync::Arc<RenderImage>,
    zoom: f32,
    opacity: f32,
    locked: bool,
    hovered: bool,
    focus: FocusHandle,
    hwnd: isize,
}

pub fn open(capture: Capture, cx: &mut App) {
    let (x, y) = capture::cursor_pos();
    let Some(m) = capture::monitor_at(x, y) else { return };
    let (iw, ih) = (capture.image.width() as f32, capture.image.height() as f32);
    // fit within 80% of the work area at most
    let fit = ((m.work.w as f32 * 0.8) / iw).min((m.work.h as f32 * 0.8) / ih).min(1.0);
    let (w, h) = (((iw * fit) as i32).max(40), ((ih * fit) as i32).max(30));
    let rect = PRect { x: m.work.x + (m.work.w - w) / 2, y: m.work.y + (m.work.h - h) / 2, w, h };
    let scale = m.scale;
    let bounds = Bounds::new(
        point(px(rect.x as f32 / scale), px(rect.y as f32 / scale)),
        size(px(rect.w as f32 / scale), px(rect.h as f32 / scale)),
    );
    let full = ui::to_render_image(&capture.image);
    let display_id = cx.displays().into_iter().find(|d| d.bounds().contains(&bounds.center())).map(|d| d.id());
    let res = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            focus: true,
            show: true,
            kind: WindowKind::PopUp,
            is_movable: true,
            is_resizable: false,
            is_minimizable: false,
            display_id,
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        },
        move |window, cx| {
            let hwnd = platform::hwnd_of(window).map(|h| h.0 as isize).unwrap_or_default();
            if hwnd != 0 {
                let h = windows::Win32::Foundation::HWND(hwnd as _);
                platform::set_hud_frame(h);
                platform::set_tool_window(h);
                platform::place_window(h, rect, true);
                // again, in case the first move crossed into another DPI and
                // gpui's WM_DPICHANGED handler resized the window
                platform::place_window(h, rect, true);
                platform::force_foreground(h);
            }
            // Alt+F4 etc. close the window without PinView::close: prune the handle too
            window.on_window_should_close(cx, |window, cx| {
                let me = window.window_handle();
                state_mut(cx).pins.retain(|p| AnyWindowHandle::from(*p) != me);
                true
            });
            cx.new(|cx| {
                let focus = cx.focus_handle();
                window.focus(&focus);
                PinView { capture, full, zoom: fit, opacity: 1.0, locked: false, hovered: false, focus, hwnd }
            })
        },
    );
    if let Ok(h) = res {
        state_mut(cx).pins.push(h);
    }
}

pub fn unlock_all(cx: &mut App) {
    let pins = state(cx).pins.clone();
    let mut alive = vec![];
    for p in pins {
        let ok = p
            .update(cx, |v, _, cx| {
                v.set_locked(false);
                cx.notify();
            })
            .is_ok();
        if ok {
            alive.push(p);
        }
    }
    state_mut(cx).pins = alive;
}

impl PinView {
    fn hwnd(&self) -> windows::Win32::Foundation::HWND {
        windows::Win32::Foundation::HWND(self.hwnd as _)
    }

    fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
        platform::set_click_through(self.hwnd(), locked);
    }

    /// Current zoom, derived from the real window size: gpui's WM_DPICHANGED
    /// handler resizes the window when it is dragged to a monitor with another DPI.
    fn current_zoom(&self) -> f32 {
        let r = platform::window_rect(self.hwnd());
        let iw = self.capture.image.width().max(1) as f32;
        if r.w > 0 { r.w as f32 / iw } else { self.zoom }
    }

    fn zoom_by(&mut self, factor: f32) {
        let z = self.current_zoom() * factor;
        self.set_zoom(z);
    }

    fn set_zoom(&mut self, z: f32) {
        let r = platform::window_rect(self.hwnd());
        let (iw, ih) = (self.capture.image.width().max(1) as f32, self.capture.image.height().max(1) as f32);
        // clamp the zoom (not the sides) so the minimum size keeps the aspect ratio
        let zmin = (40.0 / iw).max(30.0 / ih).max(0.1).min(1.0);
        let z = z.clamp(zmin, 4.0f32.max(zmin));
        // tiny / strip images (zmin capped at 1): still keep the window at least
        // 40x30 so it can be grabbed and its toolbar fits
        let (w, h) = (((iw * z).round() as i32).max(40), ((ih * z).round() as i32).max(30));
        // zoom around the window centre
        let nr = PRect { x: r.x + (r.w - w) / 2, y: r.y + (r.h - h) / 2, w, h };
        self.zoom = z;
        platform::place_window(self.hwnd(), nr, true);
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let me = window.window_handle();
        state_mut(cx).pins.retain(|p| AnyWindowHandle::from(*p) != me);
        window.remove_window();
    }

    fn on_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ks = &e.keystroke;
        let step = if ks.modifiers.shift { 10 } else { 1 };
        match ks.key.as_str() {
            "escape" => self.close(window, cx),
            "left" => platform::move_window_by(self.hwnd(), -step, 0),
            "right" => platform::move_window_by(self.hwnd(), step, 0),
            "up" => platform::move_window_by(self.hwnd(), 0, -step),
            "down" => platform::move_window_by(self.hwnd(), 0, step),
            "c" if ks.modifiers.control => app::copy_capture(&self.capture.clone(), cx),
            "l" => self.set_locked(!self.locked),
            "=" | "+" => self.zoom_by(1.1),
            "-" => self.zoom_by(1.0 / 1.1),
            "0" => self.set_zoom(1.0),
            _ => {}
        }
        cx.notify();
    }
}

impl Render for PinView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let hovered = self.hovered && !self.locked;
        // label from the actual window size (it can change behind our back on a DPI switch)
        let iw = self.capture.image.width().max(1) as f32;
        let zoom = f32::from(window.bounds().size.width) * window.scale_factor() / iw;
        let tb = |id: &'static str, path: Icon| {
            div()
                .id(id)
                .size(px(24.))
                .rounded(px(6.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .hover(|s| s.bg(color::hover()))
                .child(icon(path, px(14.), rgb(0xFFFFFF)))
        };
        div()
            .id("pin")
            .size_full()
            .relative()
            .track_focus(&self.focus)
            .font_family(ui::FONT)
            .opacity(self.opacity)
            .on_key_down(cx.listener(Self::on_key))
            .on_hover(cx.listener(|this, h: &bool, _, cx| {
                this.hovered = *h;
                cx.notify();
            }))
            .on_mouse_down(MouseButton::Left, |_, window, _| window.start_window_move())
            .on_scroll_wheel(cx.listener(|this, e: &ScrollWheelEvent, _, cx| {
                let dy = match e.delta {
                    ScrollDelta::Lines(l) => l.y,
                    ScrollDelta::Pixels(p) => f32::from(p.y) / 40.0,
                };
                if e.modifiers.control {
                    this.opacity = (this.opacity + dy * 0.05).clamp(0.2, 1.0);
                } else {
                    this.zoom_by(1.0 + dy * 0.08);
                }
                cx.notify();
            }))
            .child(img(self.full.clone()).absolute().size_full())
            .child(
                div()
                    .absolute()
                    .size_full()
                    .border(px(BORDER))
                    .border_color(if hovered { color::accent().into() } else { rgba(0xFFFFFF40) }),
            )
            .when(hovered, |d| {
                d.child(
                    div()
                        .absolute()
                        .top(px(8.))
                        .right(px(8.))
                        .flex()
                        .gap(px(2.))
                        .p(px(3.))
                        .rounded(px(9.))
                        .bg(color::hud())
                        .border_1()
                        .border_color(color::hud_border())
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(tb("p-copy", Icon::Copy).on_click(cx.listener(|this, _, _, cx| app::copy_capture(&this.capture.clone(), cx))))
                        .child(tb("p-edit", Icon::Edit).on_click(cx.listener(|this, _, window, cx| {
                            let c = this.capture.clone();
                            this.close(window, cx);
                            cx.defer(move |cx| annotate::open(c, cx));
                        })))
                        .child(tb("p-opacity", Icon::Eye).on_click(cx.listener(|this, _, _, cx| {
                            this.opacity = if this.opacity > 0.9 { 0.7 } else if this.opacity > 0.6 { 0.45 } else { 1.0 };
                            cx.notify();
                        })))
                        .child(tb("p-lock", Icon::Lock).on_click(cx.listener(|this, _, _, cx| {
                            this.set_locked(true);
                            cx.notify();
                        })))
                        .child(tb("p-close", Icon::Close).on_click(cx.listener(|this, _, window, cx| this.close(window, cx)))),
                )
                .child(
                    div()
                        .absolute()
                        .bottom(px(8.))
                        .left(px(8.))
                        .px(px(7.))
                        .py(px(2.))
                        .rounded(px(6.))
                        .bg(color::hud())
                        .text_size(px(11.))
                        .text_color(color::text_dim())
                        .child(format!("{:.0}% · opacity {:.0}%", zoom * 100.0, self.opacity * 100.0)),
                )
            })
    }
}
