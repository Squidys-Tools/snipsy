use crate::annotate::doc::*;
use crate::annotate::doc::{Rgba, TextStyle};
use crate::ui::{self, color, font_size, icon, radius, Icon};
use gpui::prelude::*;
use gpui::*;
use super::{Editor, Menu, STROKES};

impl Editor {
    pub(super) fn top_bar(&self, cx: &mut Context<Self>) -> (AnyElement, AnyElement, AnyElement) {
        let chip = |b: Stateful<Div>| div().p(px(2.)).rounded(radius::MD).bg(color::control()).child(b);
        let left = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(
                chip(ui::icon_button("crop", Icon::Crop, self.tool == Tool::Crop)
                    .tooltip(ui::tip(Tool::Crop.label()))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.set_tool(Tool::Crop);
                        window.focus(&this.focus);
                        cx.notify();
                    }))),
            )
            .child(
                chip(ui::icon_button("resize", Icon::Resize, self.show_resize)
                    .tooltip(ui::tip("Resize"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_resize = !this.show_resize;
                        cx.notify();
                    }))),
            )
            .child(
                chip(ui::icon_button("bg", Icon::Background, self.show_bg)
                    .tooltip(ui::tip("Background tool"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_bg = !this.show_bg;
                        if this.show_bg && !this.doc.backdrop.enabled {
                            this.push_undo();
                            this.doc.backdrop.enabled = true;
                        }
                        cx.notify();
                    }))),
            );

        let mut track = div().flex().items_center().gap(px(2.)).p(px(2.)).rounded(radius::MD).bg(color::control());
        for (i, t) in Tool::TOOLBAR.iter().copied().filter(|t| *t != Tool::Crop).enumerate() {
            if i == 1 || i == 9 {
                track = track.child(hairline());
            }
            track = track.child(
                ui::tool_button(("tool", i), t.icon(), self.tool == t)
                    .tooltip(ui::tip(t.label()))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.set_tool(t);
                        window.focus(&this.focus);
                        cx.notify();
                    })),
            );
        }
        track = track
            .child(hairline())
            .child(
                ui::tool_button("undo", Icon::Undo, false)
                    .tooltip(ui::tip("Undo"))
                    .when(self.undo.is_empty(), |d| d.opacity(0.35))
                    .on_click(cx.listener(|this, _, w, cx| {
                        this.do_undo();
                        this.refresh_display(w);
                        cx.notify();
                    })),
            )
            .child(
                ui::tool_button("redo", Icon::Redo, false)
                    .tooltip(ui::tip("Redo"))
                    .when(self.redo.is_empty(), |d| d.opacity(0.35))
                    .on_click(cx.listener(|this, _, w, cx| {
                        this.do_redo();
                        this.refresh_display(w);
                        cx.notify();
                    })),
            );

        let mut center = div().flex().items_center().gap(px(8.)).child(track);
        if self.tool.uses_color() || self.selected.is_some() {
            center = center.child(self.color_dropdown(cx)).child(self.stroke_dropdown(cx));
        }
        let text_selected = matches!(
            self.doc.annotations.iter().find(|a| Some(a.id) == self.selected),
            Some(Annotation { shape: Shape::Text { .. }, .. })
        );
        if self.tool == Tool::Text || text_selected {
            center = center.child(self.style_dropdown(cx));
        }

        let right = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .child(ui::button("copy", "Copy", false).on_click(cx.listener(|this, _, _, cx| this.export_copy(cx))))
            .child(ui::button("save", "Save", true).on_click(cx.listener(|this, _, _, cx| this.export_save(cx))));

        (left.into_any_element(), center.into_any_element(), right.into_any_element())
    }

    fn dropdown(&self, id: &'static str, which: Menu, face: impl IntoElement, menu_el: Stateful<Div>, cx: &mut Context<Self>) -> Div {
        let open = self.menu == Some(which);
        div()
            .relative()
            .child(
                ui::dropdown(id, face, open)
                    .h(px(28.))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.menu = (!open).then_some(which);
                        cx.notify();
                    })),
            )
            .when(open, |d| {
                d.child(deferred(
                    anchored()
                        .anchor(Corner::TopLeft)
                        .offset(point(px(0.), px(34.)))
                        .child(ui::enter(SharedString::from(format!("{id}-menu")), menu_el, 0., -4.)),
                ))
            })
    }

    fn color_dropdown(&self, cx: &mut Context<Self>) -> Div {
        let face = div()
            .size(px(16.))
            .rounded(radius::SM)
            .bg(ui::hsla_of(self.color))
            .border_1()
            .border_color(color::border());
        let swatch = |id: (&'static str, usize), c: Rgba, active: bool, cx: &mut Context<Self>| {
            div()
                .id(id)
                .size(px(22.))
                .flex_none()
                .rounded_full()
                .p(px(2.))
                .border_2()
                .border_color(if active { color::text().into() } else { transparent_black() })
                .cursor_pointer()
                .child(div().size_full().rounded_full().bg(ui::hsla_of(c)).border_1().border_color(color::border()))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.set_color(c);
                    this.menu = None;
                    cx.notify();
                }))
        };
        let mut grid = div().flex().flex_wrap().gap(px(6.)).p(px(4.)).max_w(px(200.));
        for (i, c) in PALETTE.iter().copied().enumerate() {
            grid = grid.child(swatch(("color", i), c, self.color == c, cx));
        }
        let mut m = ui::menu("color-menu").child(grid);
        if !self.custom_colors.is_empty() {
            let mut custom = div().flex().flex_wrap().gap(px(6.)).p(px(4.));
            for (i, c) in self.custom_colors.iter().copied().enumerate() {
                custom = custom.child(swatch(("custom", i), c, self.color == c, cx));
            }
            m = m.child(div().mx(px(4.)).h(px(1.)).bg(color::separator())).child(custom);
        }
        m = m.child(
            div()
                .id("eyedropper")
                .group("eyedropper")
                .h(px(28.))
                .mx(px(4.))
                .mb(px(4.))
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(8.))
                .rounded(radius::SM)
                .text_size(font_size::BODY)
                .text_color(color::text())
                .cursor_pointer()
                .hover(|s| s.bg(color::accent()).text_color(color::on_accent()))
                .child(icon(Icon::Picker, px(14.), color::text_dim()).group_hover("eyedropper", |s| s.text_color(color::on_accent())))
                .child("Pick from image")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.eyedropper = true;
                    this.menu = None;
                    cx.notify();
                })),
        );
        self.dropdown("color", Menu::Color, face, m, cx)
    }

    fn stroke_dropdown(&self, cx: &mut Context<Self>) -> Div {
        let face = div()
            .w(px(18.))
            .h(px((self.width * 0.55).max(1.5)))
            .rounded_full()
            .bg(color::text());
        let mut m = ui::menu("stroke-menu");
        for (i, w) in STROKES.iter().copied().enumerate() {
            let active = (self.width - w).abs() < 0.1;
            let group = SharedString::from(format!("stroke-{i}"));
            m = m.child(
                div()
                    .id(("stroke", i))
                    .group(group.clone())
                    .h(px(28.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .rounded(radius::SM)
                    .cursor_pointer()
                    .when(!active, |d| d.hover(|s| s.bg(color::accent())))
                    .when(active, |d| d.bg(color::selected()))
                    .child(
                        div()
                            .w(px(24.))
                            .h(px((w * 0.55).max(1.5)))
                            .rounded_full()
                            .bg(color::text())
                            .group_hover(group, |s| s.bg(color::on_accent())),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_width(w);
                        this.menu = None;
                        cx.notify();
                    })),
            );
        }
        self.dropdown("stroke", Menu::Stroke, face, m, cx)
    }

    fn style_dropdown(&self, cx: &mut Context<Self>) -> Div {
        let face = div().text_size(font_size::BODY).text_color(color::text()).child(self.text_style.label());
        let m = ui::menu("style-menu").children(TextStyle::ALL.iter().copied().enumerate().map(|(i, s)| {
            ui::menu_row(("tstyle", i), s.label(), self.text_style == s).on_click(cx.listener(move |this, _, _, cx| {
                this.text_style = s;
                this.menu = None;
                let needs = matches!(this.selected_mut(), Some(Annotation { shape: Shape::Text { style, .. }, .. }) if *style != s);
                if needs {
                    this.push_undo_restyle();
                    if let Some(Annotation { shape: Shape::Text { style, .. }, .. }) = this.selected_mut() {
                        *style = s;
                    }
                }
                cx.notify();
            }))
        }));
        self.dropdown("tstyle", Menu::TextStyle, face, m, cx)
    }

    pub(super) fn drag_chip(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        div()
            .id("drag-me")
            .h(px(28.))
            .px(px(10.))
            .flex()
            .items_center()
            .gap(px(6.))
            .rounded(radius::MD)
            .bg(color::control())
            .text_size(font_size::SMALL)
            .text_color(color::text_dim())
            .cursor(CursorStyle::OpenHand)
            .tooltip(ui::tip("Drag into any app"))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.export_drag(window, cx)))
            .child(icon(Icon::Drag, px(14.), color::text_dim()))
            .child("Drag me")
            .child(icon(Icon::Drag, px(14.), color::text_dim()))
    }
}

fn hairline() -> Div {
    div().w(px(1.)).h(px(18.)).mx(px(3.)).bg(color::separator())
}
