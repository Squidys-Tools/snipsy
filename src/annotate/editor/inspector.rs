use crate::annotate::doc::*;
use crate::annotate::doc::{Rgba, TextStyle};
use crate::ui::{self, color};
use gpui::prelude::*;
use gpui::*;
use super::toolbar::Tip;
use super::{Editor, RESIZE_FIT, RESIZE_PRESETS, STROKES};

impl Editor {
    pub(super) fn bottom_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = self.color_swatches(cx);
        let strokes = self.stroke_widths(cx);
        let text_styles = self.text_style_controls(cx);
        let crop_controls = self.crop_controls(cx);
        let transform = self.transform_buttons(cx);
        let resize_controls = self.resize_controls(cx);

        let (_, z) = self.view.get();
        let sf = 1.0; // zoom shown relative to image pixels
        div()
            .h(px(48.))
            .flex_none()
            .flex()
            .items_center()
            .gap(px(14.))
            .px(px(14.))
            .border_t_1()
            .border_color(color::separator())
            .bg(color::sidebar_bg())
            .when(!self.show_resize && (self.tool.uses_color() || self.selected.is_some()), |d| d.child(colors).child(strokes))
            .when(!self.show_resize, |d| d.children(text_styles).children(crop_controls))
            .children(resize_controls)
            .child(div().flex_1())
            .child(transform)
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(color::text_faint())
                    .child(format!("{} × {}  ·  {:.0}%", self.doc.base.width(), self.doc.base.height(), z * 100.0 * sf)),
            )
    }

    fn color_swatches(&self, cx: &mut Context<Self>) -> Div {
        let swatch = |id: (&'static str, usize), c: Rgba, active: bool| {
            div()
                .id(id)
                .size(px(22.))
                .flex_none()
                .rounded_full()
                .p(px(2.))
                .border_2()
                .border_color(if active { color::text().into() } else { transparent_black() })
                .cursor_pointer()
                .child(div().size_full().rounded_full().bg(ui::hsla_of(c)).border_1().border_color(rgba(0xFFFFFF33)))
        };
        let mut colors = div().flex().items_center().gap(px(4.));
        for (i, c) in PALETTE.iter().copied().enumerate() {
            colors = colors.child(swatch(("color", i), c, self.color == c).on_click(cx.listener(move |this, _, _, cx| {
                this.set_color(c);
                cx.notify();
            })));
        }
        if !self.custom_colors.is_empty() {
            colors = colors.child(div().w(px(1.)).h(px(18.)).mx(px(4.)).bg(color::separator()));
            for (i, c) in self.custom_colors.iter().copied().enumerate() {
                colors = colors.child(swatch(("custom", i), c, self.color == c).on_click(cx.listener(move |this, _, _, cx| {
                    this.set_color(c);
                    cx.notify();
                })));
            }
        }
        colors = colors.child(
            div()
                .id("eyedropper")
                .ml(px(4.))
                .h(px(24.))
                .px(px(8.))
                .flex()
                .items_center()
                .rounded(px(6.))
                .text_size(px(12.))
                .cursor_pointer()
                .text_color(if self.eyedropper { color::accent() } else { color::text_dim() })
                .when(self.eyedropper, |d| d.bg(color::selected()))
                .hover(|s| s.bg(color::hover()))
                .child("Pick color")
                .on_click(cx.listener(|this, _, _, cx| {
                    this.eyedropper = !this.eyedropper;
                    cx.notify();
                })),
        );
        colors
    }

    fn stroke_widths(&self, cx: &mut Context<Self>) -> Div {
        let mut strokes = ui::segmented();
        for (i, w) in STROKES.iter().copied().enumerate() {
            let active = (self.width - w).abs() < 0.1;
            strokes = strokes.child(
                div()
                    .id(("stroke", i))
                    .w(px(30.))
                    .h(px(24.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(active, |d| d.bg(rgba(0xFFFFFF33)))
                    .when(!active, |d| d.hover(|s| s.bg(color::hover())))
                    .child(div().w(px(16.)).h(px((w * 0.55).max(1.5))).rounded_full().bg(color::text()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_width(w);
                        cx.notify();
                    })),
            );
        }
        strokes
    }

    fn text_style_controls(&self, cx: &mut Context<Self>) -> Option<Div> {
        (self.tool == Tool::Text).then(|| {
            let mut seg = ui::segmented();
            for (i, s) in TextStyle::ALL.iter().copied().enumerate() {
                seg = seg.child(ui::segment(("tstyle", i), s.label(), self.text_style == s).on_click(cx.listener(move |this, _, _, cx| {
                    this.text_style = s;
                    let needs = matches!(this.selected_mut(), Some(Annotation { shape: Shape::Text { style, .. }, .. }) if *style != s);
                    if needs {
                        this.push_undo_restyle();
                        if let Some(Annotation { shape: Shape::Text { style, .. }, .. }) = this.selected_mut() {
                            *style = s;
                        }
                    }
                    cx.notify();
                })));
            }
            seg
        })
    }

    fn crop_controls(&self, cx: &mut Context<Self>) -> Option<Div> {
        (self.tool == Tool::Crop).then(|| {
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
                .child(div().text_size(px(12.)).text_color(color::text_dim()).child(
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
        })
    }

    fn transform_buttons(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .gap(px(2.))
            .child(ui::icon_button("rot-l", "icons/undo.svg", false).tooltip(|_, cx| cx.new(|_| Tip("Rotate left".into())).into()).on_click(cx.listener(|this, _, w, cx| {
                this.rotate(false);
                this.refresh_display(w);
                cx.notify();
            })))
            .child(ui::icon_button("rot-r", "icons/redo.svg", false).tooltip(|_, cx| cx.new(|_| Tip("Rotate right".into())).into()).on_click(cx.listener(|this, _, w, cx| {
                this.rotate(true);
                this.refresh_display(w);
                cx.notify();
            })))
            .child(ui::segment("flip-h", "Flip H", false).on_click(cx.listener(|this, _, w, cx| {
                this.flip(true);
                this.refresh_display(w);
                cx.notify();
            })))
            .child(ui::segment("flip-v", "Flip V", false).on_click(cx.listener(|this, _, w, cx| {
                this.flip(false);
                this.refresh_display(w);
                cx.notify();
            })))
            .child(ui::segment("resize", "Resize", self.show_resize).on_click(cx.listener(|this, _, _, cx| {
                this.show_resize = !this.show_resize;
                cx.notify();
            })))
    }

    fn resize_controls(&self, cx: &mut Context<Self>) -> Option<Div> {
        self.show_resize.then(|| {
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
                .child(div().text_size(px(12.)).text_color(color::text_dim()).child("Resize"))
                .child(seg)
                .child(fit)
                .child(ui::button("resize-done", "Done", false).on_click(cx.listener(|this, _, _, cx| {
                    this.show_resize = false;
                    cx.notify();
                })))
        })
    }
}
