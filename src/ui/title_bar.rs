use super::assets::{icon, Icon};
use super::theme::{color, font_size};
use gpui::prelude::*;
use gpui::*;

pub fn title_bar(title: impl Into<SharedString>, center: Option<AnyElement>, right: Option<AnyElement>) -> Div {
    let title = title.into();
    let leading = div()
        .id("drag")
        .flex_shrink()
        .min_w(px(60.))
        .h_full()
        .flex()
        .items_center()
        .px(px(14.))
        .gap(px(8.))
        .window_control_area(WindowControlArea::Drag)
        .text_size(font_size::TITLE)
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color::text())
        .child(div().min_w_0().truncate().child(title.clone()))
        .into_any_element();
    chrome(title, px(44.), color::window_bg().into(), leading, center, right)
}

/// 52 px `toolbar_bg` bar for the editor: control clusters instead of a title.
pub fn tool_bar(left: impl IntoElement, center: impl IntoElement, right: impl IntoElement) -> Div {
    let leading = div()
        .h_full()
        .flex_none()
        .flex()
        .items_center()
        .px(px(10.))
        .child(left)
        .into_any_element();
    let right = div().h_full().flex_none().flex().items_center().pr(px(10.)).child(right).into_any_element();
    chrome("editor".into(), px(52.), color::toolbar_bg().into(), leading, Some(center.into_any_element()), Some(right))
}

// group names are app-global, so caption groups get a per-bar prefix
fn chrome(ns: SharedString, height: Pixels, bg: Hsla, leading: AnyElement, center: Option<AnyElement>, right: Option<AnyElement>) -> Div {
    div()
        .h(height)
        .w_full()
        .flex_none()
        .flex()
        .items_center()
        .bg(bg)
        .border_b_1()
        .border_color(color::separator())
        .child(leading)
        .child(
            div()
                .flex_1()
                .min_w(px(12.))
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .window_control_area(WindowControlArea::Drag)
                .children(center),
        )
        .children(right)
        .child(caption(ns.clone(), "min", Icon::Minus, WindowControlArea::Min, false))
        .child(caption(ns.clone(), "max", Icon::Maximize, WindowControlArea::Max, false))
        .child(caption(ns, "close", Icon::Close, WindowControlArea::Close, true))
}

fn caption(ns: SharedString, id: &'static str, icon_path: Icon, area: WindowControlArea, danger: bool) -> Stateful<Div> {
    let group = SharedString::from(format!("{ns}-{id}"));
    div()
        .id(id)
        .group(group.clone())
        .w(px(46.))
        .flex_none()
        .h_full()
        .flex()
        .items_center()
        .justify_center()
        .window_control_area(area)
        .when(danger, |d| d.hover(|s| s.bg(color::close_hover()).text_color(color::on_accent())))
        .when(!danger, |d| d.hover(|s| s.bg(color::hover())))
        .child(icon(icon_path, px(14.), color::text_dim()).group_hover(group, move |s| {
            s.text_color(if danger { color::on_accent() } else { color::text() })
        }))
}
