use super::assets::icon;
use super::theme::color;
use gpui::prelude::*;
use gpui::*;

/// Square icon button used in HUD toolbars.
pub fn icon_button(id: impl Into<ElementId>, path: &'static str, active: bool) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(32.))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .rounded(px(7.))
        .cursor_pointer()
        .when(active, |d| d.bg(color::selected()))
        .when(!active, |d| d.hover(|s| s.bg(color::hover())))
        .child(icon(path, px(18.), if active { color::text() } else { rgba(0xEBEBF5D0) }))
}

/// CleanShot style translucent pill button ("Copy" / "Save" on overlay cards).
pub fn pill_button(id: impl Into<ElementId>, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(14.))
        .h(px(26.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(rgba(0xF2F2F7E6))
        .text_color(rgb(0x1C1C1E))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(|s| s.bg(rgb(0xFFFFFF)))
        .child(label)
}

/// Standard text button.
pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>, primary: bool) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(12.))
        .h(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(7.))
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .when(primary, |d| d.bg(color::accent()).text_color(rgb(0xFFFFFF)).hover(|s| s.bg(color::accent_hover())))
        .when(!primary, |d| {
            d.bg(rgba(0xFFFFFF1F)).text_color(color::text()).hover(|s| s.bg(rgba(0xFFFFFF2E)))
        })
        .child(label.into())
}

/// macOS style toggle switch.
pub fn toggle(id: impl Into<ElementId>, on: bool) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(38.))
        .h(px(22.))
        .flex_none()
        .rounded_full()
        .p(px(2.))
        .cursor_pointer()
        .bg(if on { color::accent() } else { rgba(0xFFFFFF2E) })
        .flex()
        .when(on, |d| d.justify_end())
        .child(div().size(px(18.)).rounded_full().bg(rgb(0xFFFFFF)).shadow_sm())
}

/// Segmented control item.
pub fn segment(id: impl Into<ElementId>, label: impl Into<SharedString>, active: bool) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(10.))
        .h(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .text_size(px(12.))
        .cursor_pointer()
        .text_color(if active { color::text() } else { color::text_dim() })
        .when(active, |d| d.bg(rgba(0xFFFFFF33)).shadow_sm())
        .when(!active, |d| d.hover(|s| s.bg(color::hover())))
        .child(label.into())
}

pub fn segmented() -> Div {
    div().flex().gap(px(2.)).p(px(2.)).rounded(px(8.)).bg(rgba(0x00000040))
}
