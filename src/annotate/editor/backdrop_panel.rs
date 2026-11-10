use crate::annotate::doc::*;
use crate::ui::{self, color};
use gpui::prelude::*;
use gpui::*;
use super::{Editor, PADDINGS, RADII};

impl Editor {
    pub(super) fn bg_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let bd = self.doc.backdrop;
        let label = |t: &'static str| div().text_size(px(11.)).font_weight(FontWeight::SEMIBOLD).text_color(color::text_faint()).mt(px(10.)).child(t);
        let mut grid = div().flex().flex_wrap().gap(px(8.));
        for (i, (angle, c0, c1)) in BACKGROUNDS.iter().copied().enumerate() {
            let active = bd.preset == i && bd.enabled;
            grid = grid.child(
                div()
                    .id(("bgp", i))
                    .size(px(44.))
                    .rounded(px(9.))
                    .cursor_pointer()
                    .border_2()
                    .border_color(if active { color::accent().into() } else { rgba(0xFFFFFF1A) })
                    .bg(linear_gradient(angle, linear_color_stop(ui::hex_hsla(c0), 0.0), linear_color_stop(ui::hex_hsla(c1), 1.0)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.push_undo();
                        this.doc.backdrop.preset = i;
                        this.doc.backdrop.enabled = true;
                        cx.notify();
                    })),
            );
        }
        let mut pads = ui::segmented();
        for (i, (l, v)) in PADDINGS.iter().copied().enumerate() {
            pads = pads.child(ui::segment(("pad", i), l, (bd.padding - v).abs() < 0.001).on_click(cx.listener(move |this, _, _, cx| {
                this.push_undo();
                this.doc.backdrop.padding = v;
                cx.notify();
            })));
        }
        let mut radii = ui::segmented();
        for (i, (l, v)) in RADII.iter().copied().enumerate() {
            radii = radii.child(ui::segment(("rad", i), l, (bd.radius - v).abs() < 0.1).on_click(cx.listener(move |this, _, _, cx| {
                this.push_undo();
                this.doc.backdrop.radius = v;
                cx.notify();
            })));
        }
        let mut aspects = ui::segmented().flex_wrap();
        for (i, a) in Aspect::ALL.iter().copied().enumerate() {
            aspects = aspects.child(ui::segment(("asp", i), a.label(), bd.aspect == a).on_click(cx.listener(move |this, _, _, cx| {
                this.push_undo();
                this.doc.backdrop.aspect = a;
                cx.notify();
            })));
        }
        let row = |l: &'static str, control: Stateful<Div>| {
            div()
                .flex()
                .items_center()
                .justify_between()
                .mt(px(10.))
                .child(div().text_size(px(13.)).text_color(color::text()).child(l))
                .child(control)
        };
        div()
            .w(px(260.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(16.))
            .border_l_1()
            .border_color(color::separator())
            .bg(color::sidebar_bg())
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).text_color(color::text()).child("Background"))
                    .child(ui::toggle("bg-on", bd.enabled).on_click(cx.listener(|this, _, _, cx| {
                        this.push_undo();
                        this.doc.backdrop.enabled = !this.doc.backdrop.enabled;
                        cx.notify();
                    }))),
            )
            .child(grid)
            .child(label("PADDING"))
            .child(pads)
            .child(label("CORNER RADIUS"))
            .child(radii)
            .child(label("ASPECT RATIO"))
            .child(aspects)
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
            .child(div().text_size(px(11.)).text_color(color::text_faint()).child("Auto Balance trims uneven solid borders so the content sits centred."))
    }
}
