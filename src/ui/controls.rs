use super::assets::{icon, Icon};
use super::motion;
use super::theme::{color, font_size, radius};
use gpui::prelude::*;
use gpui::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub fn icon_button(id: impl Into<ElementId>, path: Icon, active: bool) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(32.))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .rounded(radius::MD)
        .cursor_pointer()
        .when(active, |d| d.bg(color::selected()))
        .when(!active, |d| d.hover(|s| s.bg(color::hover())).active(|s| s.bg(color::control_pressed())))
        .child(icon(path, px(18.), if active { color::text() } else { color::text_dim() }))
}

/// A 28 px tool icon inside a `control` track; active is accent with a white icon.
pub fn tool_button(id: impl Into<ElementId>, path: Icon, active: bool) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(28.))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .rounded(radius::SM)
        .cursor_pointer()
        .when(active, |d| d.bg(color::accent()))
        .when(!active, |d| d.hover(|s| s.bg(color::control_hover())).active(|s| s.bg(color::control_pressed())))
        .child(icon(path, px(16.), if active { color::on_accent() } else { color::text_dim() }))
}

pub fn pill_button(id: impl Into<ElementId>, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(14.))
        .h(px(26.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(color::light_pill())
        .text_color(color::light_pill_text())
        .text_size(font_size::BODY)
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(|s| s.bg(color::on_accent()))
        .active(|s| s.opacity(0.8))
        .child(label)
}

pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>, primary: bool) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(12.))
        .h(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(radius::MD)
        .text_size(font_size::BODY)
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .active(|s| s.opacity(0.8))
        .when(primary, |d| d.bg(color::accent()).text_color(color::on_accent()).hover(|s| s.bg(color::accent_hover())))
        .when(!primary, |d| {
            d.bg(color::control()).text_color(color::text()).hover(|s| s.bg(color::control_hover()))
        })
        .child(label.into())
}

pub fn toggle(id: impl Into<ElementId>, on: bool) -> Stateful<Div> {
    const TRAVEL: f32 = 16.;
    let id = id.into();
    let knob = div().size(px(18.)).rounded_full().bg(color::on_accent()).shadow_sm();
    let knob = if motion::just_changed(&id, on) {
        knob.with_animation(if on { "on" } else { "off" }, Animation::new(motion::FAST).with_easing(ease_in_out), move |k, t| {
            k.ml(px(TRAVEL * if on { t } else { 1.0 - t }))
        })
        .into_any_element()
    } else {
        knob.ml(px(if on { TRAVEL } else { 0. })).into_any_element()
    };
    div()
        .id(id)
        .w(px(38.))
        .h(px(22.))
        .flex_none()
        .rounded_full()
        .p(px(2.))
        .cursor_pointer()
        .bg(if on { color::accent() } else { color::control_hover() })
        .flex()
        .child(knob)
}

pub fn segment(id: impl Into<ElementId>, label: impl Into<SharedString>, active: bool) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(10.))
        .h(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(radius::SM)
        .text_size(font_size::SMALL)
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .text_color(if active { color::text() } else { color::text_dim() })
        .when(active, |d| d.bg(color::selected()).shadow_sm())
        .when(!active, |d| d.hover(|s| s.bg(color::hover())))
        .child(label.into())
}

pub fn segmented() -> Div {
    div().flex().gap(px(2.)).p(px(2.)).rounded(radius::MD).bg(color::control())
}

/// 0..1 slider. `on_change(value, fresh)` fires on press and while dragging;
/// `fresh` is true only for the press so views can snapshot undo once per gesture.
/// The drag flag lives in keyed element state because every render rebuilds the
/// element; a plain Rc<Cell> would be replaced before the next frame painted.
pub fn slider(id: impl Into<ElementId>, value: f32, on_change: impl Fn(f32, bool, &mut Window, &mut App) + 'static) -> Stateful<Div> {
    let value = value.clamp(0.0, 1.0);
    let track: Rc<Cell<Bounds<Pixels>>> = Rc::new(Cell::new(Bounds::default()));
    let drag_slot: Rc<RefCell<Option<Entity<bool>>>> = Rc::new(RefCell::new(None));
    let on_change = Rc::new(on_change);
    let frac = {
        let track = track.clone();
        Rc::new(move |pos: Point<Pixels>| {
            let b = track.get();
            let w = f32::from(b.size.width);
            if w <= 0.0 { 0.0 } else { (f32::from(pos.x - b.origin.x) / w).clamp(0.0, 1.0) }
        })
    };
    let set_drag = {
        let drag_slot = drag_slot.clone();
        Rc::new(move |cx: &mut App, v: bool| {
            if let Some(d) = drag_slot.borrow().clone() {
                d.update(cx, |d, _| *d = v);
            }
        })
    };
    let hit = {
        let track = track.clone();
        let drag_slot = drag_slot.clone();
        let on_change = on_change.clone();
        let frac = frac.clone();
        canvas(
            move |b, _, _| track.set(b),
            move |_, _, window, cx| {
                let drag = window.use_keyed_state("sld", cx, |_, _| false);
                *drag_slot.borrow_mut() = Some(drag.clone());
                if *drag.read(cx) {
                    let on_change = on_change.clone();
                    let frac = frac.clone();
                    window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Capture && e.pressed_button == Some(MouseButton::Left) {
                            on_change(frac(e.position), false, window, cx);
                        }
                    });
                    window.on_mouse_event(move |_: &MouseUpEvent, phase, window, cx| {
                        if phase == DispatchPhase::Capture {
                            drag.update(cx, |d, _| *d = false);
                            window.refresh();
                        }
                    });
                }
            },
        )
        .absolute()
        .size_full()
        .into_any_element()
    };
    div()
        .id(id)
        .relative()
        .h(px(16.))
        .flex_none()
        .w_full()
        .flex()
        .items_center()
        .cursor_pointer()
        .child(hit)
        .child(div().h(px(4.)).w_full().rounded_full().bg(color::control()))
        .child(div().absolute().top(px(6.)).left_0().h(px(4.)).w(relative(value)).rounded_full().bg(color::accent()))
        .child(div().absolute().top_0().left(relative(value)).ml(px(-8.)).size(px(16.)).rounded_full().bg(color::on_accent()).shadow_md())
        .on_mouse_down(MouseButton::Left, {
            let set_drag = set_drag.clone();
            let on_change = on_change.clone();
            let frac = frac.clone();
            move |e, window, cx| {
                set_drag(cx, true);
                on_change(frac(e.position), true, window, cx);
            }
        })
        .on_mouse_up(MouseButton::Left, {
            let set_drag = set_drag.clone();
            move |_, window, cx| {
                set_drag(cx, false);
                window.refresh();
            }
        })
        .on_mouse_up_out(MouseButton::Left, move |_, window, cx| {
            set_drag(cx, false);
            window.refresh();
        })
}

/// Text or number field frame; callers size it and attach handlers.
pub fn field(id: impl Into<ElementId>, active: bool) -> Stateful<Div> {
    div()
        .id(id)
        .rounded(radius::MD)
        .bg(color::field())
        .border_1()
        .border_color(if active { color::accent() } else { color::field() })
}

/// Ghost button whose `face` sits left of a chevron; `open` shows the active fill.
pub fn dropdown(id: impl Into<ElementId>, face: impl IntoElement, open: bool) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(28.))
        .px(px(8.))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(4.))
        .rounded(radius::MD)
        .cursor_pointer()
        .when(open, |d| d.bg(color::selected()))
        .when(!open, |d| d.hover(|s| s.bg(color::hover())))
        .child(face)
        .child(icon(Icon::ChevronDown, px(12.), color::text_dim()))
}

/// HUD popover frame. Render inside `deferred(anchored()...)`.
/// Needs a hitbox that swallows its own clicks, or an occluding dismiss layer
/// beneath it receives the mouse-down and closes the menu before rows can click.
pub fn menu(id: impl Into<ElementId>) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_col()
        .min_w(px(120.))
        .p(px(4.))
        .rounded(radius::LG)
        .bg(color::hud())
        .border_1()
        .border_color(color::hud_border())
        .shadow_lg()
        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
        .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_mouse_up(MouseButton::Right, |_, _, cx| cx.stop_propagation())
}

pub fn menu_row(id: impl Into<ElementId>, label: impl Into<SharedString>, active: bool) -> Stateful<Div> {
    div()
        .id(id)
        .h(px(28.))
        .px(px(8.))
        .flex()
        .items_center()
        .gap(px(6.))
        .rounded(radius::SM)
        .text_size(font_size::BODY)
        .text_color(color::text())
        .cursor_pointer()
        .hover(|s| s.bg(color::accent()).text_color(color::on_accent()))
        .child(
            div()
                .w(px(14.))
                .flex_none()
                .when(active, |d| d.child(icon(Icon::Check, px(14.), color::text()))),
        )
        .child(label.into())
}
