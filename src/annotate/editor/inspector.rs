use crate::annotate::doc::*;
use crate::ui::{self, color, font_size, Icon};
use gpui::prelude::*;
use gpui::*;
use super::{Editor, RESIZE_FIT, RESIZE_PRESETS};

impl Editor {
    pub(super) fn bottom_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let (_, z) = self.view.get();
        let center = if self.tool == Tool::Crop {
            self.crop_controls(cx).into_any_element()
        } else if self.show_resize {
            self.resize_controls(cx).into_any_element()
        } else {
            self.drag_chip(cx).into_any_element()
        };
        div()
            .h(px(44.))
            .flex_none()
            .flex()
            .items_center()
            .px(px(14.))
            .border_t_1()
            .border_color(color::separator())
            .bg(color::toolbar_bg())
            .child(
                div()
                    .flex_none()
                    .text_size(font_size::CAPTION)
                    .text_color(color::text_faint())
                    .child(format!("{} × {} · {:.0}%", self.doc.base.width(), self.doc.base.height(), z * 100.0)),
            )
            .child(div().flex_1().min_w_0().h_full().flex().items_center().justify_center().child(center))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(2.))
                    .child(ui::icon_button("rot-l", Icon::RotateLeft, false).tooltip(ui::tip("Rotate left")).on_click(cx.listener(|this, _, w, cx| {
                        this.rotate(false);
                        this.refresh_display(w);
                        cx.notify();
                    })))
                    .child(ui::icon_button("rot-r", Icon::RotateRight, false).tooltip(ui::tip("Rotate right")).on_click(cx.listener(|this, _, w, cx| {
                        this.rotate(true);
                        this.refresh_display(w);
                        cx.notify();
                    })))
                    .child(ui::icon_button("flip-h", Icon::FlipH, false).tooltip(ui::tip("Flip horizontally")).on_click(cx.listener(|this, _, w, cx| {
                        this.flip(true);
                        this.refresh_display(w);
                        cx.notify();
                    })))
                    .child(ui::icon_button("flip-v", Icon::FlipV, false).tooltip(ui::tip("Flip vertically")).on_click(cx.listener(|this, _, w, cx| {
                        this.flip(false);
                        this.refresh_display(w);
                        cx.notify();
                    })))
                    .child(ui::icon_button("pin", Icon::Pin, false).tooltip(ui::tip("Pin to screen (Ctrl+P)")).on_click(cx.listener(|this, _, _, cx| this.export_pin(cx)))),
            )
    }

    fn crop_controls(&self, cx: &mut Context<Self>) -> Div {
        let mut ratios = ui::segmented();
        for (i, a) in Aspect::ALL.iter().copied().enumerate() {
            let label = if a == Aspect::Auto { "Free" } else { a.label() };
            ratios = ratios.child(ui::segment(("crop-asp", i), label, self.crop_aspect == a).on_click(cx.listener(move |this, _, _, cx| {
                this.set_crop_aspect(a);
                cx.notify();
            })));
        }
        div()
            .flex()
            .gap(px(6.))
            .items_center()
            .child(ratios)
            .child(div().text_size(font_size::SMALL).text_color(color::text_dim()).child(
                self.crop.map(|r| format!("{} × {}", r.w() as u32, r.h() as u32)).unwrap_or_default(),
            ))
            .child(ui::button("crop-cancel", "Cancel", false).on_click(cx.listener(|this, _, _, cx| {
                this.crop = None;
                this.tool = Tool::Arrow;
                cx.notify();
            })))
            .child(ui::button("crop-apply", "Apply Crop", true).on_click(cx.listener(|this, _, w, cx| {
                this.apply_crop();
                this.refresh_display(w);
                cx.notify();
            })))
    }

    fn resize_controls(&self, cx: &mut Context<Self>) -> Div {
        let (bw, bh) = (self.doc.base.width(), self.doc.base.height());
        let mut seg = ui::segmented();
        for (i, (l, f)) in RESIZE_PRESETS.iter().copied().enumerate() {
            seg = seg.child(ui::segment(("resize-pct", i), l, false).on_click(cx.listener(move |this, _, w, cx| {
                this.resize_by(f);
                this.refresh_display(w);
                cx.notify();
            })));
        }
        let mut fit = ui::segmented();
        for (i, m) in RESIZE_FIT.iter().copied().enumerate() {
            let enabled = bw.max(bh) > m;
            fit = fit.child(
                ui::segment(("resize-fit", i), format!("Max {m}"), false)
                    .when(!enabled, |d| d.opacity(0.35))
                    .on_click(cx.listener(move |this, _, w, cx| {
                        this.resize_fit(m);
                        this.refresh_display(w);
                        cx.notify();
                    })),
            );
        }
        div()
            .flex()
            .gap(px(6.))
            .items_center()
            .child(seg)
            .child(fit)
            .child(ui::button("resize-done", "Done", false).on_click(cx.listener(|this, _, _, cx| {
                this.show_resize = false;
                cx.notify();
            })))
    }
}
