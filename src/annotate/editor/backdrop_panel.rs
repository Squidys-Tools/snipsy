use crate::annotate::doc::*;
use crate::ui::{self, color, font_size, radius};
use gpui::prelude::*;
use gpui::*;
use super::{Editor, Menu};

const MAX_PADDING: f32 = 0.24;
const MAX_RADIUS: f32 = 28.0;

impl Editor {
    pub(super) fn bg_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let bd = self.doc.backdrop;
        let heading = |t: &'static str| div().text_size(font_size::SMALL).text_color(color::text_dim()).mt(px(12.)).mb(px(2.)).child(t);
        let mut grid = div().flex().flex_wrap().gap(px(8.));
        for (i, (angle, c0, c1)) in BACKGROUNDS.iter().copied().enumerate() {
            let active = bd.preset == i && bd.enabled;
            grid = grid.child(
                div()
                    .id(("bgp", i))
                    .size(px(44.))
                    .rounded(radius::MD)
                    .cursor_pointer()
                    .border_2()
                    .border_color(if active { color::accent().into() } else { color::border() })
                    .bg(linear_gradient(angle, linear_color_stop(ui::hex_hsla(c0), 0.0), linear_color_stop(ui::hex_hsla(c1), 1.0)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.push_undo();
                        this.doc.backdrop.preset = i;
                        this.doc.backdrop.enabled = true;
                        cx.notify();
                    })),
            );
        }
        let e = cx.entity();
        let padding = ui::slider("pad", (bd.padding / MAX_PADDING).clamp(0.0, 1.0), move |v, fresh, _, cx| {
            e.update(cx, |this, cx| {
                if fresh {
                    this.push_undo();
                }
                this.doc.backdrop.padding = v * MAX_PADDING;
                cx.notify();
            });
        });
        let e = cx.entity();
        let radius_ = ui::slider("rad", (bd.radius / MAX_RADIUS).clamp(0.0, 1.0), move |v, fresh, _, cx| {
            e.update(cx, |this, cx| {
                if fresh {
                    this.push_undo();
                }
                this.doc.backdrop.radius = v * MAX_RADIUS;
                cx.notify();
            });
        });
        let row = |l: &'static str, control: Stateful<Div>| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .mt(px(10.))
                .child(div().text_size(font_size::BODY).text_color(color::text()).child(l))
                .child(control)
        };
        let open = self.menu == Some(Menu::Aspect);
        let aspect = div()
            .relative()
            .mt(px(10.))
            .child(
                ui::dropdown("bg-aspect", div().text_size(font_size::BODY).text_color(color::text()).child(bd.aspect.label()), open)
                    .w_full()
                    .justify_between()
                    .bg(color::control())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.menu = (!open).then_some(Menu::Aspect);
                        cx.notify();
                    })),
            )
            .when(open, |d| {
                d.child(deferred(
                    anchored()
                        .anchor(Corner::TopLeft)
                        .offset(point(px(0.), px(34.)))
                        .child(ui::enter("bg-aspect-menu", ui::menu("bg-aspect-list").w_full().children(Aspect::ALL.iter().copied().enumerate().map(|(i, a)| {
                            ui::menu_row(("asp", i), a.label(), bd.aspect == a).on_click(cx.listener(move |this, _, _, cx| {
                                this.push_undo();
                                this.doc.backdrop.aspect = a;
                                this.menu = None;
                                cx.notify();
                            }))
                        })), 0., -4.)),
                ))
            });
        div()
            .id("bg-panel")
            .w(px(280.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(16.))
            .overflow_y_scroll()
            .border_r_1()
            .border_color(color::separator())
            .bg(color::sidebar_bg())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_size(font_size::BODY).font_weight(FontWeight::SEMIBOLD).text_color(color::text()).child("Background"))
                    .child(ui::toggle("bg-on", bd.enabled).on_click(cx.listener(|this, _, _, cx| {
                        this.push_undo();
                        this.doc.backdrop.enabled = !this.doc.backdrop.enabled;
                        cx.notify();
                    }))),
            )
            .child(grid)
            .child(heading("Padding"))
            .child(padding)
            .child(heading("Corner Radius"))
            .child(radius_)
            .child(heading("Aspect Ratio"))
            .child(aspect)
            .child(row("Shadow", ui::toggle("bg-shadow", bd.shadow).on_click(cx.listener(|this, _, _, cx| {
                this.push_undo();
                this.doc.backdrop.shadow = !this.doc.backdrop.shadow;
                cx.notify();
            }))))
            .child(row(
                "Auto Balance",
                ui::toggle("bg-balance", bd.balance).on_click(cx.listener(|this, _, _, cx| {
                    this.push_undo();
                    this.doc.backdrop.balance = !this.doc.backdrop.balance;
                    cx.notify();
                })),
            ))
            .child(div().text_size(font_size::CAPTION).text_color(color::text_faint()).child("Auto Balance trims uneven solid borders so the content sits centred."))
    }
}
