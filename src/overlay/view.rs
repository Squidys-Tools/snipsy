use super::geometry::DragKind;
use super::{close_all, OverlayView, MAG_CELL, MAG_CELLS};
use crate::app::Mode;
use crate::capture::{self, WindowInfo};
use crate::ui::{self, color, font_size, radius};
use gpui::prelude::*;
use gpui::*;
use image::RgbaImage;

fn pill(text: String) -> Div {
    div()
        .px(px(7.))
        .py(px(2.))
        .rounded(radius::SM)
        .bg(color::hud())
        .border_1()
        .border_color(color::hud_border())
        .text_color(color::text())
        .text_size(font_size::CAPTION)
        .font_family(ui::FONT)
        .whitespace_nowrap()
        .child(text)
}

// No text_shadow in gpui: a dark copy offset one pixel serves as the shadow.
fn coord_text(text: String) -> Div {
    div()
        .relative()
        .whitespace_nowrap()
        .text_size(font_size::CAPTION)
        .font_family(ui::FONT)
        .child(div().absolute().left(px(1.)).top(px(1.)).text_color(color::scrim()).child(text.clone()))
        .child(div().text_color(color::text()).child(text))
}

fn paint_selection(window: &mut Window, full: Bounds<Pixels>, s: Bounds<Pixels>, with_handles: bool) {
    let dim = color::scrim();
    let (x0, y0) = (s.origin.x, s.origin.y);
    let (x1, y1) = (x0 + s.size.width, y0 + s.size.height);
    let r = |a: Point<Pixels>, b: Point<Pixels>| Bounds::from_corners(a, b);
    for b in [
        r(full.origin, point(full.size.width, y0)),
        r(point(px(0.), y1), point(full.size.width, full.size.height)),
        r(point(px(0.), y0), point(x0, y1)),
        r(point(x1, y0), point(full.size.width, y1)),
    ] {
        if b.size.width > px(0.) && b.size.height > px(0.) {
            window.paint_quad(fill(b, dim));
        }
    }
    window.paint_quad(outline(s.dilate(px(1.)), color::scrim(), BorderStyle::Solid));
    window.paint_quad(outline(s, color::text(), BorderStyle::Solid));
    if with_handles {
        let (mx, my) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
        for (hx, hy) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1), (mx, y0), (mx, y1), (x0, my), (x1, my)] {
            let hb = Bounds::new(point(hx - px(4.), hy - px(4.)), gpui::size(px(8.), px(8.)));
            window.paint_quad(quad(hb, px(4.), color::on_accent(), px(1.), color::scrim(), BorderStyle::Solid));
        }
    }
}

fn paint_window_highlight(window: &mut Window, w: Bounds<Pixels>) {
    let mut tint = color::accent();
    tint.a = 0.2;
    window.paint_quad(fill(w, tint));
    window.paint_quad(quad(w, px(0.), transparent_black(), px(3.), color::accent(), BorderStyle::Solid));
}

fn paint_crosshair(window: &mut Window, full: Bounds<Pixels>, m: Point<Pixels>) {
    let line = color::scrim();
    let line2 = color::text_faint();
    window.paint_quad(fill(Bounds::new(point(px(0.), m.y), gpui::size(full.size.width, px(1.))), line));
    window.paint_quad(fill(Bounds::new(point(m.x, px(0.)), gpui::size(px(1.), full.size.height)), line));
    window.paint_quad(fill(Bounds::new(point(px(0.), m.y + px(1.)), gpui::size(full.size.width, px(1.))), line2));
    window.paint_quad(fill(Bounds::new(point(m.x + px(1.), px(0.)), gpui::size(px(1.), full.size.height)), line2));
}

fn window_title_label(w: Bounds<Pixels>, info: &WindowInfo) -> AnyElement {
    let title = if info.title.len() > 48 { format!("{}…", &info.title.chars().take(48).collect::<String>()) } else { info.title.clone() };
    div()
        .absolute()
        .left(w.origin.x)
        .top(w.origin.y)
        .w(w.size.width)
        .h(w.size.height)
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(4.))
                .px(px(14.))
                .py(px(8.))
                .rounded(radius::LG)
                .bg(color::hud())
                .border_1()
                .border_color(color::hud_border())
                .shadow_lg()
                .text_color(color::text())
                .font_family(ui::FONT)
                .child(div().text_size(font_size::BODY).font_weight(FontWeight::SEMIBOLD).child(title))
                .child(div().text_size(font_size::CAPTION).text_color(color::text_dim()).child(format!("{} × {}", info.rect.w, info.rect.h))),
        )
        .into_any_element()
}

fn magnifier_panel(mag: std::sync::Arc<RenderImage>, m: Point<Pixels>, size: Size<Pixels>) -> AnyElement {
    let side = px(MAG_CELLS as f32 * MAG_CELL);
    let mut ox = m.x + px(24.);
    let mut oy = m.y + px(30.);
    if ox + side > size.width {
        ox = m.x - side - px(24.);
    }
    if oy + side > size.height {
        oy = m.y - side - px(30.);
    }
    let c = px((MAG_CELLS / 2) as f32 * MAG_CELL);
    let band = color::scrim();
    div()
        .absolute()
        .left(ox)
        .top(oy)
        .size(side)
        .rounded(radius::LG)
        .shadow_lg()
        .child(img(mag).absolute().size(side).rounded(radius::LG))
        .child(div().absolute().left(c).top_0().w(px(MAG_CELL)).h(c).bg(band))
        .child(div().absolute().left(c).top(c + px(MAG_CELL)).w(px(MAG_CELL)).h(c).bg(band))
        .child(div().absolute().top(c).left_0().h(px(MAG_CELL)).w(c).bg(band))
        .child(div().absolute().top(c).left(c + px(MAG_CELL)).h(px(MAG_CELL)).w(c).bg(band))
        .child(div().absolute().left(c).top(c).size(px(MAG_CELL)).border_1().border_color(color::on_accent()))
        .child(div().absolute().size(side).rounded(radius::LG).border_1().border_color(color::scrim()))
        .into_any_element()
}

impl OverlayView {
    pub(super) fn update_magnifier(&mut self, window: &mut Window) {
        if !self.show_mag {
            return;
        }
        let Some(m) = self.mouse else { return };
        let cx0 = (f32::from(m.x) * self.scale) as i32;
        let cy0 = (f32::from(m.y) * self.scale) as i32;
        let cell = (MAG_CELL * self.scale).round().max(1.0) as u32;
        let n = MAG_CELLS;
        let side = n * cell;
        let mut out = RgbaImage::new(side, side);
        let half = (n / 2) as i32;
        for gy in 0..n {
            for gx in 0..n {
                let sx = cx0 + gx as i32 - half;
                let sy = cy0 + gy as i32 - half;
                let p = if sx >= 0 && sy >= 0 && (sx as u32) < self.image.width() && (sy as u32) < self.image.height() {
                    *self.image.get_pixel(sx as u32, sy as u32)
                } else {
                    image::Rgba([0, 0, 0, 255])
                };
                for yy in 0..cell {
                    for xx in 0..cell {
                        out.put_pixel(gx * cell + xx, gy * cell + yy, p);
                    }
                }
            }
        }
        if let Some(old) = self.magnifier.replace(ui::to_render_image(&out)) {
            let _ = window.drop_image(old);
        }
    }

    fn paint_canvas(
        &self,
        selection: Option<Bounds<Pixels>>,
        window_rect: Option<Bounds<Pixels>>,
        mouse: Option<Point<Pixels>>,
        show_cross: bool,
        dragging_new: bool,
    ) -> impl IntoElement {
        let all_in_one = self.mode == Mode::AllInOne;
        let dim_all = self.mode != Mode::Window && selection.is_none() && all_in_one;
        canvas(
            |_, _, _| {},
            move |full, _, window, _| {
                if let Some(s) = selection {
                    paint_selection(window, full, s, all_in_one && !dragging_new);
                } else if dim_all {
                    window.paint_quad(fill(full, color::scrim()));
                }
                if let Some(w) = window_rect {
                    paint_window_highlight(window, w);
                }
                if show_cross {
                    if let Some(m) = mouse {
                        paint_crosshair(window, full, m);
                    }
                }
            },
        )
        .absolute()
        .size_full()
    }

    fn size_label(&self, s: Bounds<Pixels>, size: Size<Pixels>) -> AnyElement {
        let r = self.to_image_rect(s);
        let below = s.origin.y + s.size.height + px(8.);
        let y = if below + px(24.) > size.height { s.origin.y - px(26.) } else { below };
        div()
            .absolute()
            .left(s.origin.x + s.size.width / 2.0 - px(50.))
            .top(y)
            .w(px(100.))
            .flex()
            .justify_center()
            .child(pill(format!("{} × {}", r.w, r.h)))
            .into_any_element()
    }

    fn cursor_labels(&self, m: Point<Pixels>, size: Size<Pixels>, labels: &mut Vec<AnyElement>) {
        let (px_, py_) = ((f32::from(m.x) * self.scale) as i32, (f32::from(m.y) * self.scale) as i32);
        if self.mode != Mode::Window {
            labels.push(
                div()
                    .absolute()
                    .left(m.x + px(8.))
                    .top(m.y + px(6.))
                    .flex()
                    .flex_col()
                    .child(coord_text(format!("{px_} · {py_}")))
                    .into_any_element(),
            );
        }
        if self.show_mag && self.mode != Mode::Window {
            if let Some(mag) = self.magnifier.clone() {
                labels.push(magnifier_panel(mag, m, size));
            }
        }
    }

    fn mode_hint(&self) -> &'static str {
        match self.mode {
            Mode::Area => "Drag to capture an area  ·  Click a window  ·  Space: window mode  ·  Esc: cancel",
            Mode::Window if self.all_in_one => "Click a window to capture it  ·  Space or Esc: back to All-In-One",
            Mode::Window => "Click a window to capture it  ·  Space: area mode  ·  Esc: cancel",
            Mode::Ocr if self.all_in_one => "Select an area to copy its text  ·  Esc: back to All-In-One",
            Mode::Ocr => "Select an area to copy its text  ·  Esc: cancel",
            Mode::AllInOne => "Drag to select  ·  Enter or double-click to capture  ·  Arrows nudge  ·  Esc: cancel",
        }
    }

    fn hint_banner(&self) -> Div {
        div().absolute().top(px(28.)).left_0().right_0().flex().justify_center().child(
            div()
                .px(px(14.))
                .py(px(7.))
                .rounded(radius::LG)
                .bg(color::hud())
                .border_1()
                .border_color(color::hud_border())
                .text_size(font_size::SMALL)
                .font_family(ui::FONT)
                .text_color(color::text_dim())
                .child(self.mode_hint()),
        )
    }
}

impl Render for OverlayView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let size = window.bounds().size;
        let (gx, gy) = capture::cursor_pos();
        let cursor_here = self.monitor.rect.contains(gx, gy);
        let mouse = self.mouse.filter(|_| cursor_here);
        let selection = self.selection;
        let dragging_new = matches!(self.drag, Some(DragKind::New { .. }));
        let window_rect = if self.mode == Mode::Window {
            self.hovered_window.as_ref().and_then(|w| self.local_from_global(w.rect))
        } else {
            None
        };
        let show_cross = self.show_cross && self.mode != Mode::Window && (selection.is_none() || dragging_new);

        let geometry = self.paint_canvas(selection, window_rect, mouse, show_cross, dragging_new);

        let mut labels: Vec<AnyElement> = vec![];
        if let Some(s) = selection {
            labels.push(self.size_label(s, size));
        }
        if let (Some(w), Some(info)) = (window_rect, self.hovered_window.as_ref()) {
            labels.push(window_title_label(w, info));
        }
        if let Some(m) = mouse.filter(|_| show_cross || self.mode == Mode::Window) {
            self.cursor_labels(m, size, &mut labels);
        }
        let show_hint = cursor_here && self.drag.is_none() && selection.is_none();

        div()
            .size_full()
            .relative()
            .track_focus(&self.focus)
            .cursor(if self.mode == Mode::Window { CursorStyle::PointingHand } else { CursorStyle::Crosshair })
            .on_key_down(cx.listener(Self::on_key))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_down))
            .on_mouse_move(cx.listener(Self::on_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_up))
            .on_mouse_down(MouseButton::Right, cx.listener(|_, _, _, cx| close_all(cx, |_| {})))
            .child(img(self.render.clone()).absolute().size_full())
            .child(geometry)
            .children(labels)
            .when(show_hint, |d| d.child(self.hint_banner()))
            // keyed on how the overlay was opened, so the HUD survives the Window / OCR sub-modes
            .when(self.all_in_one && self.drag.is_none() && (cursor_here || selection.is_some()), |d| {
                d.child(self.hud(cx))
            })
            .when(self.aspect_menu, |d| {
                d.child(
                    div()
                        .id("menu-dismiss")
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .occlude()
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| {
                            this.aspect_menu = false;
                            cx.stop_propagation();
                            cx.notify();
                        }))
                        .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_mouse_down(MouseButton::Right, cx.listener(|this, _, _, cx| {
                            this.aspect_menu = false;
                            cx.stop_propagation();
                            cx.notify();
                        })),
                )
            })
    }
}
