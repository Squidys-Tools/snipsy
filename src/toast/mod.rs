//! Small HUD notifications ("Copied", "Saved") and the shared HUD window helper.

mod timer;

use crate::app;
use crate::capture::{self, PRect};
use crate::platform;
use crate::ui::{self, color, icon};
use gpui::prelude::*;
use gpui::*;
use std::path::PathBuf;

pub use timer::{cancel_timer, start_timer, timer_running};

#[derive(Default)]
struct Toasts {
    windows: Vec<AnyWindowHandle>,
    /// The self-timer countdown HUD (kept apart so other toasts do not close it).
    countdown: Option<AnyWindowHandle>,
    timer_running: bool,
    /// Bumped whenever a timer starts or is cancelled; a stale task bails out.
    timer_gen: u64,
}
impl Global for Toasts {}

pub struct ToastView {
    title: SharedString,
    body: SharedString,
    path: Option<PathBuf>,
    countdown: Option<u32>,
}

/// Open a non-activating, transparent, topmost HUD window at a physical rect.
pub fn open_hud<V: Render + 'static>(
    cx: &mut App,
    rect: PRect,
    scale: f32,
    build: impl FnOnce(&mut Window, &mut Context<V>) -> V + 'static,
) -> Option<WindowHandle<V>> {
    let bounds = Bounds::new(
        point(px(rect.x as f32 / scale), px(rect.y as f32 / scale)),
        size(px(rect.w as f32 / scale), px(rect.h as f32 / scale)),
    );
    let display_id = cx
        .displays()
        .into_iter()
        .find(|d| {
            let b = d.bounds();
            b.contains(&bounds.center())
        })
        .map(|d| d.id());
    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            focus: false,
            show: false,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            display_id,
            window_background: WindowBackgroundAppearance::Transparent,
            ..Default::default()
        },
        move |window, cx| {
            if let Some(hwnd) = platform::hwnd_of(window) {
                platform::set_hud_frame(hwnd);
                platform::set_no_activate(hwnd);
                platform::place_window(hwnd, rect, true);
            }
            cx.new(|cx| build(window, cx))
        },
    )
    .ok()
}

fn toast_rect(w: f32, h: f32) -> Option<(PRect, f32)> {
    let m = capture::monitor_at(capture::cursor_pos().0, capture::cursor_pos().1).or_else(capture::primary_monitor)?;
    let (pw, ph) = ((w * m.scale) as i32, (h * m.scale) as i32);
    Some((
        PRect { x: m.work.x + (m.work.w - pw) / 2, y: m.work.y + m.work.h - ph - (64.0 * m.scale) as i32, w: pw, h: ph },
        m.scale,
    ))
}

pub fn show(cx: &mut App, title: impl Into<SharedString>, body: impl Into<SharedString>) {
    open_toast(cx, title.into(), body.into(), None, None);
}

pub fn show_with_path(cx: &mut App, title: impl Into<SharedString>, body: impl Into<SharedString>, path: PathBuf) {
    open_toast(cx, title.into(), body.into(), Some(path), None);
}

fn open_toast(cx: &mut App, title: SharedString, body: SharedString, path: Option<PathBuf>, countdown: Option<u32>) -> Option<WindowHandle<ToastView>> {
    if countdown.is_none() {
        close_all(cx);
    }
    let (w, h) = if countdown.is_some() { (148.0, 148.0) } else { (420.0, 72.0) };
    let (rect, scale) = toast_rect(w, h)?;
    let rect = if countdown.is_some() {
        let m = capture::monitor_at(capture::cursor_pos().0, capture::cursor_pos().1)?;
        PRect { x: m.rect.x + (m.rect.w - rect.w) / 2, y: m.rect.y + (m.rect.h - rect.h) / 2, ..rect }
    } else {
        rect
    };
    let handle = open_hud(cx, rect, scale, move |_, _| ToastView { title, body, path, countdown })?;
    if countdown.is_none() {
        cx.default_global::<Toasts>().windows.push(handle.into());
        let t = app::delay(cx, 2600);
        cx.spawn(async move |cx: &mut AsyncApp| {
            t.await;
            let _ = handle.update(cx, |_, window, _| window.remove_window());
            let _ = cx.update(|cx| forget(handle.into(), cx));
        })
        .detach();
    }
    Some(handle)
}

/// Drop a (removed) toast's handle from the list.
fn forget(handle: AnyWindowHandle, cx: &mut App) {
    cx.default_global::<Toasts>().windows.retain(|w| *w != handle);
}

/// Close every toast (not the self-timer countdown). Returns whether any
/// toast window was still open, i.e. whether DWM needs a moment to drop it.
pub fn close_all(cx: &mut App) -> bool {
    let wins = std::mem::take(&mut cx.default_global::<Toasts>().windows);
    let mut any = false;
    for w in wins {
        any |= w.update(cx, |_, window, _| window.remove_window()).is_ok();
    }
    any
}

impl Render for ToastView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if let Some(n) = self.countdown {
            return div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .font_family(ui::FONT)
                .child(
                    div()
                        .size(px(132.))
                        .rounded(px(28.))
                        .bg(color::hud())
                        .border_1()
                        .border_color(color::hud_border())
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(2.))
                        .child(div().text_size(px(56.)).font_weight(FontWeight::BOLD).text_color(color::text()).child(n.to_string()))
                        .child(div().text_size(px(12.)).text_color(color::text_dim()).child("Self-Timer")),
                )
                .into_any_element();
        }
        let path = self.path.clone();
        div()
            .size_full()
            .p(px(6.))
            .font_family(ui::FONT)
            .child(
                div()
                    .size_full()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .px(px(14.))
                    .rounded(px(14.))
                    .bg(color::hud())
                    .border_1()
                    .border_color(color::hud_border())
                    .shadow_lg()
                    .child(
                        div()
                            .size(px(32.))
                            .flex_none()
                            .rounded_full()
                            .bg(color::accent())
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon("icons/check.svg", px(18.), rgb(0xFFFFFF))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .child(div().text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(color::text()).child(self.title.clone()))
                            .child(div().text_size(px(12.)).text_color(color::text_dim()).truncate().child(self.body.clone())),
                    )
                    .when_some(path, |d, path| {
                        d.child(ui::button("reveal", "Show", false).on_click(cx.listener(move |_, _, window, cx| {
                            platform::reveal_in_explorer(&path);
                            forget(window.window_handle(), cx);
                            window.remove_window();
                        })))
                    }),
            )
            .into_any_element()
    }
}
