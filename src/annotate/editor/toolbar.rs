use crate::annotate::doc::*;
use crate::ui::{self, color, icon, Icon};
use gpui::prelude::*;
use gpui::*;
use super::Editor;

impl Editor {
    pub(super) fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut tools = div().flex().items_center().gap(px(2.));
        for (i, t) in Tool::TOOLBAR.iter().copied().enumerate() {
            if i == 2 || i == 10 {
                tools = tools.child(div().w(px(1.)).h(px(20.)).mx(px(4.)).bg(color::separator()));
            }
            tools = tools.child(
                ui::icon_button(("tool", i), t.icon(), self.tool == t)
                    .tooltip(ui::tip(t.label()))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.set_tool(t);
                        window.focus(&this.focus);
                        cx.notify();
                    })),
            );
        }
        tools
            .child(div().w(px(1.)).h(px(20.)).mx(px(4.)).bg(color::separator()))
            .child(ui::icon_button("undo", Icon::Undo, false).when(self.undo.is_empty(), |d| d.opacity(0.35)).on_click(cx.listener(|this, _, w, cx| {
                this.do_undo();
                this.refresh_display(w);
                cx.notify();
            })))
            .child(ui::icon_button("redo", Icon::Redo, false).when(self.redo.is_empty(), |d| d.opacity(0.35)).on_click(cx.listener(|this, _, w, cx| {
                this.do_redo();
                this.refresh_display(w);
                cx.notify();
            })))
    }

    pub(super) fn actions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(8.))
            .child(
                ui::icon_button("bg", Icon::Background, self.show_bg)
                    .tooltip(ui::tip("Background tool"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_bg = !this.show_bg;
                        if this.show_bg && !this.doc.backdrop.enabled {
                            this.push_undo();
                            this.doc.backdrop.enabled = true;
                        }
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("drag-me")
                    .h(px(28.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .rounded(px(7.))
                    .bg(color::field())
                    .border_1()
                    .border_color(color::control())
                    .text_size(px(12.))
                    .text_color(color::text_dim())
                    .cursor(CursorStyle::OpenHand)
                    .tooltip(ui::tip("Drag into any app"))
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.export_drag(window, cx)))
                    .child(icon(Icon::Drag, px(14.), color::text_dim()))
                    .child("Drag me"),
            )
            .child(ui::icon_button("pin", Icon::Pin, false).tooltip(ui::tip("Pin to screen (Ctrl+P)")).on_click(cx.listener(|this, _, _, cx| this.export_pin(cx))))
            .child(ui::button("copy", "Copy", false).on_click(cx.listener(|this, _, _, cx| this.export_copy(cx))))
            .child(ui::button("save", "Save", true).on_click(cx.listener(|this, _, _, cx| this.export_save(cx))))
    }
}
