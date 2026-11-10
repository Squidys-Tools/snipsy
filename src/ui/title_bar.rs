use super::assets::icon;
use super::theme::color;
use gpui::prelude::*;
use gpui::*;

/// Custom window chrome: drag area plus Windows-style min / max / close.
pub fn title_bar(title: impl Into<SharedString>, center: Option<AnyElement>, right: Option<AnyElement>) -> Div {
    let caption = |id: &'static str, icon_path: &'static str, area: WindowControlArea, danger: bool| {
        div()
            .id(id)
            .w(px(46.))
            .flex_none()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .window_control_area(area)
            .when(danger, |d| d.hover(|s| s.bg(rgb(0xC42B1C))))
            .when(!danger, |d| d.hover(|s| s.bg(color::hover())))
            .child(icon(icon_path, px(14.), color::text_dim()))
    };
    div()
        .h(px(44.))
        .w_full()
        .flex_none()
        .flex()
        .items_center()
        .border_b_1()
        .border_color(color::separator())
        .child(
            div()
                .id("drag")
                .flex_1()
                .min_w(px(60.))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .h_full()
                .flex()
                .items_center()
                .px(px(14.))
                .gap(px(8.))
                .window_control_area(WindowControlArea::Drag)
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color::text_dim())
                .child(title.into()),
        )
        .children(center.map(|c| div().flex_none().child(c)))
        .child(div().flex_1().min_w(px(12.)).h_full().window_control_area(WindowControlArea::Drag))
        .children(right.map(|r| div().flex_none().child(r)))
        .child(caption("min", "icons/minus.svg", WindowControlArea::Min, false))
        .child(caption("max", "icons/maximize.svg", WindowControlArea::Max, false))
        .child(caption("close", "icons/close.svg", WindowControlArea::Close, true))
}
