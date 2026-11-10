use super::geometry::{Aspect, Dim};
use super::{close_all, with_selection_holder, OverlayView};
use crate::app::{self, Command, Mode};
use crate::history::CaptureKind;
use crate::ui::{self, color, icon, Icon};
use gpui::prelude::*;
use gpui::*;

fn item(id: &'static str, icon_path: Icon, label: &'static str, active: bool) -> Stateful<Div> {
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
}

// Panels (not the full-width row) swallow presses so they never start a
// selection drag nor eat the mouse-up that ends one.
fn panel() -> Div {
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
}

fn sep() -> Div {
    div().w(px(1.)).h(px(32.)).mx(px(4.)).bg(color::separator())
}

impl OverlayView {
    pub(super) fn hud(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .absolute()
            .bottom(px(56.))
            .left_0()
            .right_0()
            .flex()
            .justify_center()
            .gap(px(12.))
            .font_family(ui::FONT)
            .child(self.hud_modes(cx))
            .child(self.hud_options(cx))
    }

    fn hud_modes(&self, cx: &mut Context<Self>) -> Div {
        panel()
            .child(item("hud-area", Icon::Area, "Area", self.mode == Mode::AllInOne).on_click(cx.listener(|this, _, _, cx| {
                if this.selection.is_some() && this.mode == Mode::AllInOne {
                    this.capture_selection(CaptureKind::Area, cx);
                } else {
                    this.set_mode(Mode::AllInOne);
                    this.sync_peers(cx);
                    cx.notify();
                }
            })))
            .child(item("hud-full", Icon::Fullscreen, "Fullscreen", false).on_click(cx.listener(|this, _, _, cx| this.capture_fullscreen(cx))))
            .child(item("hud-window", Icon::Window, "Window", self.mode == Mode::Window).on_click(cx.listener(|this, _, _, cx| {
                this.set_mode(Mode::Window);
                this.update_hover();
                this.sync_peers(cx);
                cx.notify();
            })))
            .child(item("hud-timer", Icon::Timer, "Timer", false).on_click(cx.listener(|this, _, _, cx| {
                let area = this.selection.and_then(|s| this.crop(s)).map(|c| c.1);
                close_all(cx, move |cx| app::run(Command::SelfTimer(area), cx));
            })))
            .child(sep())
            .child(item("hud-ocr", Icon::Ocr, "OCR", self.mode == Mode::Ocr).on_click(cx.listener(|this, _, _, cx| {
                if this.selection.is_some() {
                    this.mode = Mode::Ocr;
                    this.capture_selection(CaptureKind::Area, cx);
                } else {
                    this.mode = Mode::Ocr;
                    this.sync_peers(cx);
                    cx.notify();
                }
            })))
    }

    fn hud_options(&self, cx: &mut Context<Self>) -> Div {
        panel()
            .px(px(14.))
            .gap(px(8.))
            .children(Aspect::ALL.map(|a| self.aspect_btn(a, cx)))
            .child(sep())
            .children(self.size_fields(cx))
            .child(sep())
            .child(self.capture_btn(cx))
            .child(ui::button("hud-cancel", "Cancel", false).on_click(cx.listener(|_, _, _, cx| close_all(cx, |_| {}))))
    }

    fn aspect_btn(&self, a: Aspect, cx: &mut Context<Self>) -> Stateful<Div> {
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
    }

    fn size_fields(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let sel = self.selection.map(|s| self.to_image_rect(s));
        vec![
            self.size_box("hud-w", Dim::W, sel.map(|r| r.w.to_string()).unwrap_or("—".into()), cx).into_any_element(),
            div().text_size(px(12.)).text_color(color::text_faint()).child("×").into_any_element(),
            self.size_box("hud-h", Dim::H, sel.map(|r| r.h.to_string()).unwrap_or("—".into()), cx).into_any_element(),
        ]
    }

    // click a box to type an exact size (physical px); Enter applies, Tab switches
    fn size_box(&self, id: &'static str, dim: Dim, v: String, cx: &mut Context<Self>) -> Stateful<Div> {
        let typing = self.editing.as_ref().filter(|(d, _)| *d == dim).map(|(_, b)| b.clone());
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
    }

    fn capture_btn(&self, cx: &mut Context<Self>) -> impl IntoElement {
        ui::button("hud-capture", "Capture", true)
            .when(self.selection.is_none(), |d| d.opacity(0.4))
            .on_click(cx.listener(|this, _, _, cx| {
                if this.selection.is_some() {
                    this.capture_selection(CaptureKind::Area, cx);
                } else {
                    with_selection_holder(cx, |v, _, cx| v.capture_selection(CaptureKind::Area, cx), |_| {});
                }
            }))
    }
}
