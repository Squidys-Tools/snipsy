//! The Annotate window: CleanShot-style dark editor with a centred tool bar,
//! live canvas, background tool side panel and color / stroke controls.

use super::doc::*;
use super::doc::{Rgba, TextStyle};
use super::render;
use crate::app::{self, state, state_mut, Capture};
use crate::history::{CaptureKind, History};
use crate::ui::{self, color, icon};
use crate::{capture, pin, platform, toast};
use gpui::prelude::*;
use gpui::*;
use image::RgbaImage;
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

const STROKES: [f32; 5] = [2.0, 4.0, 6.0, 9.0, 13.0];
const PADDINGS: [(&str, f32); 5] = [("None", 0.0), ("S", 0.05), ("M", 0.10), ("L", 0.16), ("XL", 0.24)];
const RADII: [(&str, f32); 4] = [("0", 0.0), ("8", 8.0), ("16", 16.0), ("28", 28.0)];
const RESIZE_PRESETS: [(&str, f32); 4] = [("25%", 0.25), ("50%", 0.5), ("75%", 0.75), ("200%", 2.0)];
/// "Max N": scale down so the longer side is at most N px.
const RESIZE_FIT: [u32; 3] = [2560, 1920, 1280];

pub fn open(capture: Capture, cx: &mut App) {
    let (iw, ih) = (capture.image.width() as f32, capture.image.height() as f32);
    let (x, y) = capture::cursor_pos();
    let mon = capture::monitor_at(x, y);
    let (scale, work) = mon.as_ref().map(|m| (m.scale, m.work)).unwrap_or((1.0, capture::PRect { x: 0, y: 0, w: 1920, h: 1080 }));
    let (ww, wh) = (work.w as f32 / scale, work.h as f32 / scale);
    let w = (iw / scale + 120.0).clamp(1240.0_f32.min(ww * 0.95), ww * 0.95);
    let h = (ih / scale + 200.0).clamp(640.0, wh * 0.9);
    let bounds = Bounds::new(
        point(px(work.x as f32 / scale + (ww - w) / 2.0), px(work.y as f32 / scale + (wh - h) / 2.0)),
        size(px(w), px(h)),
    );
    let _ = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions { title: Some("Snipsy — Annotate".into()), appears_transparent: true, traffic_light_position: None }),
            focus: true,
            show: true,
            kind: WindowKind::Normal,
            is_movable: true,
            is_resizable: true,
            is_minimizable: true,
            window_background: WindowBackgroundAppearance::Opaque,
            window_min_size: Some(size(px(1120.), px(560.))),
            app_id: Some("Snipsy".into()),
            ..Default::default()
        },
        move |window, cx| {
            if let Some(h) = platform::hwnd_of(window) {
                platform::force_foreground(h);
            }
            cx.new(|cx| Editor::new(capture, window, cx))
        },
    );
}

type DisplayKey = (usize, Vec<(u8, i32, i32, i32, i32)>, [i32; 4]);

#[derive(Clone, Copy)]
enum Drag {
    Draw { start: Pt },
    Move { last: Pt },
    Crop { start: Pt },
}

pub struct Editor {
    capture: Capture,
    doc: Doc,
    undo: Vec<Doc>,
    redo: Vec<Doc>,
    tool: Tool,
    color: Rgba,
    width: f32,
    text_style: TextStyle,
    custom_colors: Vec<Rgba>,
    eyedropper: bool,
    drawing: Option<Annotation>,
    drag: Option<Drag>,
    selected: Option<u64>,
    editing_text: Option<u64>,
    /// Byte length of IME composition text appended to the edited text.
    ime_marked: usize,
    crop: Option<Rect>,
    /// Fixed aspect ratio for the crop drag (Auto = free).
    crop_aspect: Aspect,
    show_resize: bool,
    show_bg: bool,
    next_id: u64,
    /// Stable capture id for drag-out exports, so repeated drags overwrite one
    /// temp file instead of leaving a new one each time.
    drag_id: u64,
    /// Redo stack + dirty flag saved by a speculative undo snapshot, restored
    /// when that snapshot turns out to be a no-op.
    tentative: Option<(Vec<Doc>, bool)>,
    display: Arc<RenderImage>,
    display_key: (usize, Vec<(u8, i32, i32, i32, i32)>),
    /// Display image cropped to the Auto Balance trim rect, so its own
    /// rounded corners match the export.
    display_trim: Option<(DisplayKey, Arc<RenderImage>)>,
    canvas_bounds: Rc<Cell<Bounds<Pixels>>>,
    /// screen origin of image px (0,0) and logical px per image px, from last frame
    view: Cell<(Point<Pixels>, f32)>,
    focus: FocusHandle,
    dirty: bool,
}

impl Editor {
    fn new(capture: Capture, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus);
        let doc = Doc::new(capture.image.clone());
        let display = ui::to_render_image(&doc.base);
        let custom_colors = state(cx).settings.custom_colors.clone();
        Self {
            display_key: (Arc::as_ptr(&doc.base) as usize, vec![]),
            capture,
            doc,
            undo: vec![],
            redo: vec![],
            tool: Tool::Arrow,
            color: PALETTE[0],
            width: 4.0,
            text_style: TextStyle::Outline,
            custom_colors,
            eyedropper: false,
            drawing: None,
            drag: None,
            selected: None,
            editing_text: None,
            ime_marked: 0,
            crop: None,
            crop_aspect: Aspect::Auto,
            show_resize: false,
            show_bg: false,
            next_id: 1,
            drag_id: app::next_capture_id(),
            tentative: None,
            display,
            display_trim: None,
            canvas_bounds: Rc::new(Cell::new(Bounds::default())),
            view: Cell::new((point(px(0.), px(0.)), 1.0)),
            focus,
            dirty: false,
        }
    }

    // ------------------------------------------------------------ doc ops

    fn push_undo(&mut self) {
        self.tentative = None;
        self.undo.push(self.doc.clone());
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
        self.dirty = true;
    }

    /// Snapshot before an operation that may turn out to change nothing
    /// (a selection click, starting a text edit). `drop_noop_undo` undoes it.
    fn push_undo_tentative(&mut self) {
        let saved = (std::mem::take(&mut self.redo), self.dirty);
        self.push_undo();
        self.tentative = Some(saved);
    }

    /// Snapshot before restyling the selection, unless it is the text being
    /// edited under a pending tentative snapshot, which already covers it (so
    /// an abandoned text still leaves no undo step).
    fn push_undo_restyle(&mut self) {
        if self.tentative.is_some() && self.editing_text.is_some() && self.editing_text == self.selected {
            return;
        }
        self.push_undo();
    }

    /// Drop the last undo snapshot if it equals the current doc, restoring the
    /// redo stack / dirty flag a tentative snapshot cleared.
    fn drop_noop_undo(&mut self) {
        if self.undo.last().is_some_and(|d| d.same_content(&self.doc)) {
            self.undo.pop();
            if let Some((r, d)) = self.tentative.take() {
                self.redo = r;
                self.dirty = d;
            }
        }
        self.tentative = None;
    }

    fn do_undo(&mut self) {
        self.commit_text();
        if let Some(d) = self.undo.pop() {
            self.redo.push(std::mem::replace(&mut self.doc, d));
            self.selected = None;
            self.dirty = true;
        }
        self.reset_crop();
    }

    fn do_redo(&mut self) {
        self.commit_text();
        if let Some(d) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.doc, d));
            self.selected = None;
            self.dirty = true;
        }
        self.reset_crop();
    }

    /// While cropping, reset the crop rect to the full (possibly new) image.
    fn reset_crop(&mut self) {
        if self.tool == Tool::Crop {
            let (w, h) = (self.doc.base.width() as f32, self.doc.base.height() as f32);
            self.crop = Some(self.aspect_crop_rect(w, h));
        }
    }

    /// Largest centred rect of the current crop aspect (full image when free).
    fn aspect_crop_rect(&self, w: f32, h: f32) -> Rect {
        let Some(r) = self.crop_aspect.ratio() else { return Rect { x0: 0.0, y0: 0.0, x1: w, y1: h } };
        let (cw, ch) = if w / h > r { (h * r, h) } else { (w, w / r) };
        let (x0, y0) = (((w - cw) / 2.0).floor(), ((h - ch) / 2.0).floor());
        Rect { x0, y0, x1: x0 + cw.floor(), y1: y0 + ch.floor() }
    }

    fn set_crop_aspect(&mut self, a: Aspect) {
        self.crop_aspect = a;
        self.reset_crop();
    }

    /// Resize to `w`×`h` (callers keep the aspect ratio; see `resize_by`).
    fn resize(&mut self, w: u32, h: u32) {
        let (ow, oh) = (self.doc.base.width(), self.doc.base.height());
        let (w, h) = (w.clamp(8, 16384), h.clamp(8, 16384));
        if (w, h) == (ow, oh) {
            return;
        }
        self.commit_text();
        self.push_undo();
        let (sx, sy) = (w as f32 / ow as f32, h as f32 / oh as f32);
        self.doc.base = Arc::new(image::imageops::resize(&*self.doc.base, w, h, image::imageops::FilterType::CatmullRom));
        for a in &mut self.doc.annotations {
            a.scale(sx, sy);
        }
        self.reset_crop();
    }

    /// Uniform resize by `f`, limited so both sides stay within [8, 16384]
    /// (never distorting the aspect ratio).
    fn resize_by(&mut self, f: f32) {
        let (w, h) = (self.doc.base.width() as f32, self.doc.base.height() as f32);
        let (lo, hi) = ((8.0 / w).max(8.0 / h), (16384.0 / w).min(16384.0 / h));
        if lo > hi {
            return;
        }
        let f = f.clamp(lo, hi);
        self.resize((w * f).round() as u32, (h * f).round() as u32);
    }

    /// Scale down (never up) so the longer side is at most `max` px.
    fn resize_fit(&mut self, max: u32) {
        let (w, h) = (self.doc.base.width(), self.doc.base.height());
        let long = w.max(h);
        if long > max {
            self.resize_by(max as f32 / long as f32);
        }
    }

    fn new_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    fn baked_image(&self) -> RgbaImage {
        if self.doc.effects_key().is_empty() { (*self.doc.base).clone() } else { render::apply_effects(&self.doc.base, &self.doc.annotations) }
    }

    fn refresh_display(&mut self, window: &mut Window) {
        let key = (Arc::as_ptr(&self.doc.base) as usize, self.doc.effects_key());
        let mut baked = None;
        if key != self.display_key {
            let img = self.baked_image();
            let new = ui::to_render_image(&img);
            let old = std::mem::replace(&mut self.display, new);
            let _ = window.drop_image(old);
            self.display_key = key.clone();
            baked = Some(img);
        }
        match self.doc.trim() {
            Some(t) => {
                let tk: DisplayKey = (key.0, key.1, [t.x0 as i32, t.y0 as i32, t.x1 as i32, t.y1 as i32]);
                if self.display_trim.as_ref().map(|(k, _)| k) != Some(&tk) {
                    let img = baked.unwrap_or_else(|| self.baked_image());
                    let [x0, y0, x1, y1] = tk.2;
                    let sub = image::imageops::crop_imm(&img, x0.max(0) as u32, y0.max(0) as u32, (x1 - x0).max(1) as u32, (y1 - y0).max(1) as u32).to_image();
                    let new = ui::to_render_image(&sub);
                    if let Some((_, old)) = self.display_trim.replace((tk, new)) {
                        let _ = window.drop_image(old);
                    }
                }
            }
            None => {
                if let Some((_, old)) = self.display_trim.take() {
                    let _ = window.drop_image(old);
                }
            }
        }
    }

    fn selected_mut(&mut self) -> Option<&mut Annotation> {
        let id = self.selected?;
        self.doc.annotations.iter_mut().find(|a| a.id == id)
    }

    fn set_color(&mut self, c: Rgba) {
        self.color = c;
        if self.selected.is_some() {
            self.push_undo_restyle();
            if let Some(a) = self.selected_mut() {
                a.color = c;
            }
        }
    }

    fn set_width(&mut self, w: f32) {
        self.width = w;
        if self.selected.is_some() {
            self.push_undo_restyle();
            if let Some(a) = self.selected_mut() {
                a.width = w;
            }
        }
    }

    fn set_tool(&mut self, t: Tool) {
        self.commit_text();
        if self.tool == Tool::Crop && t != Tool::Crop {
            self.crop = None;
        }
        self.tool = t;
        self.eyedropper = false;
        if t != Tool::Select {
            self.selected = None;
        }
        self.reset_crop();
    }

    fn apply_crop(&mut self) {
        let Some(r) = self.crop.take() else { return };
        let (w, h) = (self.doc.base.width() as f32, self.doc.base.height() as f32);
        let r = r.clamp_to(w, h);
        if r.w() < 4.0 || r.h() < 4.0 || (r.x0 == 0.0 && r.y0 == 0.0 && r.x1 == w && r.y1 == h) {
            self.tool = Tool::Arrow;
            return;
        }
        self.push_undo();
        let img = image::imageops::crop_imm(&*self.doc.base, r.x0 as u32, r.y0 as u32, r.w() as u32, r.h() as u32).to_image();
        self.doc.base = Arc::new(img);
        for a in &mut self.doc.annotations {
            a.translate(-r.x0.floor(), -r.y0.floor());
        }
        self.tool = Tool::Arrow;
    }

    fn commit_text(&mut self) {
        self.ime_marked = 0;
        if let Some(id) = self.editing_text.take() {
            self.doc.annotations.retain(|a| !(a.id == id && matches!(&a.shape, Shape::Text { text, .. } if text.trim().is_empty())));
            // abandoned empty text / edit without changes: no undo step
            self.drop_noop_undo();
        }
    }

    /// Start editing an existing text annotation (snapshotting for undo) and
    /// sync the toolbar's text style from it.
    fn begin_edit_text(&mut self, id: u64) {
        self.commit_text();
        self.push_undo_tentative();
        self.editing_text = Some(id);
        self.selected = Some(id);
        self.ime_marked = 0;
        if let Some(Annotation { shape: Shape::Text { style, .. }, .. }) = self.doc.annotations.iter().find(|a| a.id == id) {
            self.text_style = *style;
        }
    }

    fn delete_selected(&mut self) {
        if let Some(id) = self.selected.take() {
            self.push_undo();
            self.doc.annotations.retain(|a| a.id != id);
        }
    }

    fn rotate(&mut self, clockwise: bool) {
        self.commit_text();
        self.push_undo();
        let (w, h) = (self.doc.base.width() as f32, self.doc.base.height() as f32);
        let img = if clockwise { image::imageops::rotate90(&*self.doc.base) } else { image::imageops::rotate270(&*self.doc.base) };
        self.doc.base = Arc::new(img);
        let map = |p: &mut Pt| {
            let (x, y) = (p.x, p.y);
            if clockwise {
                *p = pt(h - y, x);
            } else {
                *p = pt(y, w - x);
            }
        };
        for a in &mut self.doc.annotations {
            match &mut a.shape {
                Shape::Arrow { a, b } | Shape::Line { a, b } => {
                    map(a);
                    map(b);
                }
                Shape::Rect(r) | Shape::FilledRect(r) | Shape::Ellipse(r) | Shape::Blur(r) | Shape::Pixelate(r) | Shape::Spotlight(r) => {
                    let (mut p0, mut p1) = (pt(r.x0, r.y0), pt(r.x1, r.y1));
                    map(&mut p0);
                    map(&mut p1);
                    *r = Rect::from_pts(p0, p1);
                }
                Shape::Pencil(ps) | Shape::Highlighter(ps) => ps.iter_mut().for_each(map),
                Shape::Text { .. } => {
                    // text stays upright: keep its box centred on the rotated centre
                    let b = a.bounds();
                    let mut c = pt((b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0);
                    let old = c;
                    map(&mut c);
                    a.translate(c.x - old.x, c.y - old.y);
                }
                Shape::Counter { c, .. } => map(c),
            }
        }
        self.reset_crop();
    }

    fn flip(&mut self, horizontal: bool) {
        self.commit_text();
        self.push_undo();
        let (w, h) = (self.doc.base.width() as f32, self.doc.base.height() as f32);
        let img = if horizontal { image::imageops::flip_horizontal(&*self.doc.base) } else { image::imageops::flip_vertical(&*self.doc.base) };
        self.doc.base = Arc::new(img);
        let map = |p: &mut Pt| {
            if horizontal {
                p.x = w - p.x
            } else {
                p.y = h - p.y
            }
        };
        for a in &mut self.doc.annotations {
            match &mut a.shape {
                Shape::Arrow { a, b } | Shape::Line { a, b } => {
                    map(a);
                    map(b);
                }
                Shape::Rect(r) | Shape::FilledRect(r) | Shape::Ellipse(r) | Shape::Blur(r) | Shape::Pixelate(r) | Shape::Spotlight(r) => {
                    let (mut p0, mut p1) = (pt(r.x0, r.y0), pt(r.x1, r.y1));
                    map(&mut p0);
                    map(&mut p1);
                    *r = Rect::from_pts(p0, p1);
                }
                Shape::Pencil(ps) | Shape::Highlighter(ps) => ps.iter_mut().for_each(map),
                Shape::Text { .. } | Shape::Counter { .. } => {
                    // keep text readable: only move its anchor
                    let b = a.bounds();
                    let (mut c, mut c2) = (pt(b.x0, b.y0), pt(b.x1, b.y1));
                    map(&mut c);
                    map(&mut c2);
                    let nb = Rect::from_pts(c, c2);
                    a.translate(nb.x0 - b.x0, nb.y0 - b.y0);
                }
            }
        }
        self.reset_crop();
    }

    // ------------------------------------------------------------ coordinates

    fn to_img(&self, p: Point<Pixels>) -> Pt {
        let (o, z) = self.view.get();
        pt(f32::from(p.x - o.x) / z, f32::from(p.y - o.y) / z)
    }

    // ------------------------------------------------------------ mouse

    fn on_down(&mut self, e: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus);
        if self.drag.is_some() {
            // a release was missed somehow: finish the previous gesture first
            self.finish_drag(window, cx);
        }
        let p = self.to_img(e.position);
        if self.eyedropper {
            let (w, h) = (self.doc.base.width(), self.doc.base.height());
            if p.x >= 0.0 && p.y >= 0.0 && (p.x as u32) < w && (p.y as u32) < h {
                let c = self.doc.base.get_pixel(p.x as u32, p.y as u32).0;
                let c = [c[0], c[1], c[2], 255];
                if !self.custom_colors.contains(&c) {
                    self.custom_colors.insert(0, c);
                    self.custom_colors.truncate(6);
                    state_mut(cx).settings.custom_colors = self.custom_colors.clone();
                    state(cx).settings.save();
                }
                self.set_color(c);
            }
            self.eyedropper = false;
            cx.notify();
            return;
        }
        match self.tool {
            Tool::Select => {
                self.commit_text();
                let hit = self.doc.annotations.iter().rev().find(|a| a.hit(p)).map(|a| a.id);
                self.selected = hit;
                if let Some(id) = hit {
                    if let Some(a) = self.doc.annotations.iter().find(|a| a.id == id) {
                        self.color = a.color;
                        self.width = a.width;
                    }
                    let is_text = matches!(self.doc.annotations.iter().find(|a| a.id == id), Some(Annotation { shape: Shape::Text { .. }, .. }));
                    if e.click_count >= 2 && is_text {
                        // snapshot covers both a possible move and the edit
                        self.begin_edit_text(id);
                    } else {
                        self.push_undo_tentative();
                    }
                    self.drag = Some(Drag::Move { last: p });
                }
            }
            Tool::Crop => self.drag = Some(Drag::Crop { start: p }),
            Tool::Text => {
                // commit first so a just-abandoned empty text cannot be hit
                self.commit_text();
                // click an existing text to edit it
                let hit = self
                    .doc
                    .annotations
                    .iter()
                    .rev()
                    .find(|a| matches!(a.shape, Shape::Text { .. }) && a.bounds().contains(p))
                    .map(|a| a.id);
                if let Some(id) = hit {
                    self.begin_edit_text(id);
                } else {
                    self.push_undo_tentative();
                    let id = self.new_id();
                    let size = (self.width * 8.0).max(18.0);
                    self.doc.annotations.push(Annotation {
                        id,
                        shape: Shape::Text { pos: pt(p.x, p.y - size * TEXT_LINE_HEIGHT / 2.0), text: String::new(), style: self.text_style },
                        color: self.color,
                        width: self.width,
                        floor_scale: 1.0,
                    });
                    self.editing_text = Some(id);
                    self.selected = Some(id);
                    self.ime_marked = 0;
                }
            }
            Tool::Counter => {
                self.push_undo();
                let id = self.new_id();
                let n = self.doc.next_counter();
                self.doc.annotations.push(Annotation { id, shape: Shape::Counter { c: p, n }, color: self.color, width: self.width, floor_scale: 1.0 });
            }
            t => {
                self.commit_text();
                let id = self.new_id();
                let shape = match t {
                    Tool::Arrow => Shape::Arrow { a: p, b: p },
                    Tool::Line => Shape::Line { a: p, b: p },
                    Tool::Rect => Shape::Rect(Rect::from_pts(p, p)),
                    Tool::FilledRect => Shape::FilledRect(Rect::from_pts(p, p)),
                    Tool::Ellipse => Shape::Ellipse(Rect::from_pts(p, p)),
                    Tool::Pencil => Shape::Pencil(vec![p]),
                    Tool::Highlighter => Shape::Highlighter(vec![p]),
                    Tool::Blur => Shape::Blur(Rect::from_pts(p, p)),
                    Tool::Pixelate => Shape::Pixelate(Rect::from_pts(p, p)),
                    _ => Shape::Spotlight(Rect::from_pts(p, p)),
                };
                self.drawing = Some(Annotation { id, shape, color: self.color, width: self.width, floor_scale: 1.0 });
                self.drag = Some(Drag::Draw { start: p });
            }
        }
        cx.notify();
    }

    fn on_move(&mut self, e: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(drag) = self.drag else { return };
        if e.pressed_button != Some(MouseButton::Left) {
            return;
        }
        let mut p = self.to_img(e.position);
        let shift = e.modifiers.shift;
        match drag {
            Drag::Draw { start } => {
                if let Some(a) = self.drawing.as_mut() {
                    match &mut a.shape {
                        Shape::Arrow { b, .. } | Shape::Line { b, .. } => {
                            if shift {
                                // snap to 45°
                                let (dx, dy) = (p.x - start.x, p.y - start.y);
                                let ang = dy.atan2(dx);
                                let snap = (ang / std::f32::consts::FRAC_PI_4).round() * std::f32::consts::FRAC_PI_4;
                                let len = (dx * dx + dy * dy).sqrt();
                                p = pt(start.x + len * snap.cos(), start.y + len * snap.sin());
                            }
                            *b = p;
                        }
                        Shape::Rect(r) | Shape::FilledRect(r) | Shape::Ellipse(r) | Shape::Blur(r) | Shape::Pixelate(r) | Shape::Spotlight(r) => {
                            if shift {
                                let s = (p.x - start.x).abs().max((p.y - start.y).abs());
                                p = pt(start.x + s * (p.x - start.x).signum(), start.y + s * (p.y - start.y).signum());
                            }
                            *r = Rect::from_pts(start, p);
                        }
                        Shape::Pencil(pts) => pts.push(p),
                        Shape::Highlighter(pts) => {
                            if shift {
                                pts.truncate(1);
                            }
                            // book-style: keep the stroke horizontal while Shift is held
                            if shift {
                                p.y = pts[0].y;
                            }
                            pts.push(p);
                        }
                        _ => {}
                    }
                }
            }
            Drag::Move { last } => {
                let (dx, dy) = (p.x - last.x, p.y - last.y);
                if let Some(a) = self.selected_mut() {
                    a.translate(dx, dy);
                }
                self.drag = Some(Drag::Move { last: p });
            }
            Drag::Crop { start } => {
                let (w, h) = (self.doc.base.width() as f32, self.doc.base.height() as f32);
                // snap to image edges within 8px
                let snap = |v: f32, max: f32| if v.abs() < 8.0 { 0.0 } else if (v - max).abs() < 8.0 { max } else { v };
                let mut q = pt(snap(p.x, w), snap(p.y, h));
                let s = pt(snap(start.x.clamp(0.0, w), w), snap(start.y.clamp(0.0, h), h));
                let ratio = self.crop_aspect.ratio().or(if shift { Some(1.0) } else { None });
                if let Some(r) = ratio {
                    let (dx, dy) = (q.x - s.x, q.y - s.y);
                    // room available from the start point in the drag direction
                    let max_w = if dx >= 0.0 { w - s.x } else { s.x };
                    let max_h = if dy >= 0.0 { h - s.y } else { s.y };
                    let (aw, ah) = (dx.abs(), dy.abs());
                    let (mut aw, mut ah) = if aw / ah.max(1.0) > r { (ah.max(1.0) * r, ah.max(1.0)) } else { (aw, aw / r) };
                    if aw > max_w {
                        (aw, ah) = (max_w, max_w / r);
                    }
                    if ah > max_h {
                        (aw, ah) = (max_h * r, max_h);
                    }
                    q = pt(s.x + aw.copysign(dx), s.y + ah.copysign(dy));
                }
                self.crop = Some(Rect::from_pts(s, q).clamp_to(w, h));
            }
        }
        cx.notify();
    }

    /// Mouse released outside the canvas: still finish the gesture.
    fn on_up_out(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        if self.drag.is_some() {
            self.finish_drag(window, cx);
        }
    }

    fn on_up(&mut self, _: &MouseUpEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.finish_drag(window, cx);
    }

    fn finish_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let drag = self.drag.take();
        if let (Some(Drag::Draw { .. }), Some(mut a)) = (drag, self.drawing.take()) {
            let keep = match &mut a.shape {
                Shape::Arrow { a: p0, b } | Shape::Line { a: p0, b } => p0.dist(*b) > 3.0,
                Shape::Rect(r) | Shape::FilledRect(r) | Shape::Ellipse(r) | Shape::Blur(r) | Shape::Pixelate(r) | Shape::Spotlight(r) => r.w() > 3.0 && r.h() > 3.0,
                Shape::Pencil(pts) => {
                    *pts = smooth(pts, 2);
                    !pts.is_empty()
                }
                Shape::Highlighter(pts) => {
                    *pts = smooth(pts, 1);
                    !pts.is_empty()
                }
                _ => true,
            };
            if keep {
                self.push_undo();
                if a.is_effect() {
                    // effects sit beneath vector annotations
                    let pos = self.doc.annotations.iter().position(|x| !x.is_effect()).unwrap_or(self.doc.annotations.len());
                    self.doc.annotations.insert(pos, a);
                } else {
                    self.doc.annotations.push(a);
                }
            }
        }
        if matches!(drag, Some(Drag::Move { .. })) && self.editing_text.is_none() {
            self.drop_noop_undo(); // nothing moved
        }
        self.refresh_display(window);
        cx.notify();
    }

    fn on_key(&mut self, e: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ks = &e.keystroke;
        let ctrl = ks.modifiers.control;
        let key = ks.key.as_str();

        if let Some(id) = self.editing_text {
            let exists = self.doc.annotations.iter().any(|a| a.id == id && matches!(a.shape, Shape::Text { .. }));
            if !exists {
                // stale edit target: drop it so keys reach normal handling
                self.editing_text = None;
                self.ime_marked = 0;
            } else if !ctrl {
                let mut handled = true;
                let mut changed = false;
                if let Some(Annotation { shape: Shape::Text { text, .. }, .. }) = self.doc.annotations.iter_mut().find(|a| a.id == id) {
                    match key {
                        "escape" => handled = false,
                        "enter" if !ks.modifiers.shift => handled = false,
                        "enter" => {
                            text.push('\n');
                            changed = true;
                        }
                        "backspace" => {
                            changed = text.pop().is_some();
                        }
                        // Printable characters (and IME / Unicode input) arrive through
                        // the platform input handler (WM_CHAR) — let the key propagate.
                        _ => return,
                    }
                }
                cx.stop_propagation();
                self.ime_marked = 0;
                if changed {
                    self.dirty = true;
                }
                if !handled {
                    self.commit_text();
                    self.selected = None;
                }
                cx.notify();
                return;
            }
        }

        match (ctrl, ks.modifiers.shift, key) {
            (true, false, "z") => self.do_undo(),
            (true, true, "z") | (true, _, "y") => self.do_redo(),
            (true, _, "c") => self.export_copy(cx),
            (true, false, "s") => self.export_save(cx),
            (true, true, "s") => self.export_save_as(window, cx),
            (true, _, "p") => self.export_pin(cx),
            (false, _, "delete") | (false, _, "backspace") => self.delete_selected(),
            (false, _, "enter") if self.tool == Tool::Crop => self.apply_crop(),
            (false, _, "escape") => {
                if self.tool == Tool::Crop {
                    self.crop = None;
                    self.tool = Tool::Arrow;
                } else if self.eyedropper {
                    self.eyedropper = false;
                } else {
                    self.selected = None;
                }
            }
            (false, false, k) => {
                if let Some(t) = Tool::shortcut(k) {
                    self.set_tool(t);
                } else if let Some(i) = "12345".find(k) {
                    self.set_width(STROKES[i]);
                }
            }
            _ => {}
        }
        self.refresh_display(window);
        cx.notify();
    }

    // ------------------------------------------------------------ export

    fn render_async(&mut self, cx: &mut Context<Self>, then: impl FnOnce(RgbaImage, &mut App) + 'static) {
        self.commit_text();
        let doc = self.doc.clone();
        let task = cx.background_executor().spawn(async move { render::render(&doc) });
        cx.spawn(async move |_, cx| {
            let img = task.await;
            let _ = cx.update(|cx| then(img, cx));
        })
        .detach();
    }

    fn export_copy(&mut self, cx: &mut Context<Self>) {
        self.render_async(cx, |img, cx| match platform::copy_image(&img) {
            Ok(()) => toast::show(cx, "Copied to clipboard", format!("{} × {}", img.width(), img.height())),
            Err(e) => toast::show(cx, "Copy failed", e.to_string()),
        });
    }

    fn export_save(&mut self, cx: &mut Context<Self>) {
        self.dirty = false;
        let created = self.capture.created;
        self.render_async(cx, move |img, cx| {
            let mut c = Capture::new(Arc::new(img), CaptureKind::Edited);
            c.created = created;
            app::save_to_export(&c, cx);
            store_edited(&c, cx);
        });
    }

    fn export_save_as(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        let dir = state(cx).settings.export_dir.clone();
        let rx = cx.prompt_for_new_path(&dir, Some(&self.capture.file_name(&state(cx).settings.file_prefix)));
        let doc = self.doc.clone();
        cx.spawn(async move |_, cx| {
            if let Ok(Ok(Some(path))) = rx.await {
                let path = if path.extension().is_none() { path.with_extension("png") } else { path };
                let img = cx.background_executor().spawn(async move { render::render(&doc) }).await;
                let res = img.save(&path);
                let _ = cx.update(|cx| match res {
                    Ok(()) => toast::show_with_path(cx, "Saved", path.display().to_string(), path.clone()),
                    Err(e) => toast::show(cx, "Save failed", e.to_string()),
                });
            }
        })
        .detach();
    }

    fn export_pin(&mut self, cx: &mut Context<Self>) {
        self.render_async(cx, |img, cx| pin::open(Capture::new(Arc::new(img), CaptureKind::Edited), cx));
    }

    fn export_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(hwnd) = platform::hwnd_of(window) else { return };
        let hwnd = hwnd.0 as isize;
        let prefix = state(cx).settings.file_prefix.clone();
        self.commit_text();
        let doc = self.doc.clone();
        let created = self.capture.created;
        let drag_id = self.drag_id;
        cx.spawn(async move |_, cx| {
            let img = cx.background_executor().spawn(async move { render::render(&doc) }).await;
            let mut c = Capture::new(Arc::new(img), CaptureKind::Edited);
            c.created = created;
            c.id = drag_id;
            if let Some(path) = app::temp_file_for(&c, &prefix) {
                let _ = std::fs::remove_file(&path);
                if c.image.save(&path).is_ok() {
                    let _ = platform::drag_file_out(windows::Win32::Foundation::HWND(hwnd as _), &path);
                }
            }
        })
        .detach();
    }

    // ------------------------------------------------------------ views

    fn toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let mut tools = div().flex().items_center().gap(px(2.));
        for (i, t) in Tool::TOOLBAR.iter().copied().enumerate() {
            if i == 2 || i == 10 {
                tools = tools.child(div().w(px(1.)).h(px(20.)).mx(px(4.)).bg(color::separator()));
            }
            tools = tools.child(
                ui::icon_button(("tool", i), t.icon(), self.tool == t)
                    .tooltip(move |_, cx| cx.new(|_| Tip(t.label().into())).into())
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.set_tool(t);
                        window.focus(&this.focus);
                        cx.notify();
                    })),
            );
        }
        tools
            .child(div().w(px(1.)).h(px(20.)).mx(px(4.)).bg(color::separator()))
            .child(ui::icon_button("undo", "icons/undo.svg", false).when(self.undo.is_empty(), |d| d.opacity(0.35)).on_click(cx.listener(|this, _, w, cx| {
                this.do_undo();
                this.refresh_display(w);
                cx.notify();
            })))
            .child(ui::icon_button("redo", "icons/redo.svg", false).when(self.redo.is_empty(), |d| d.opacity(0.35)).on_click(cx.listener(|this, _, w, cx| {
                this.do_redo();
                this.refresh_display(w);
                cx.notify();
            })))
    }

    fn actions(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .px(px(8.))
            .child(
                ui::icon_button("bg", "icons/background.svg", self.show_bg)
                    .tooltip(|_, cx| cx.new(|_| Tip("Background tool".into())).into())
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
                    .bg(rgba(0xFFFFFF14))
                    .border_1()
                    .border_color(rgba(0xFFFFFF1F))
                    .text_size(px(12.))
                    .text_color(color::text_dim())
                    .cursor(CursorStyle::OpenHand)
                    .tooltip(|_, cx| cx.new(|_| Tip("Drag into any app".into())).into())
                    .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| this.export_drag(window, cx)))
                    .child(icon("icons/drag.svg", px(14.), color::text_dim()))
                    .child("Drag me"),
            )
            .child(ui::icon_button("pin", "icons/pin.svg", false).tooltip(|_, cx| cx.new(|_| Tip("Pin to screen (Ctrl+P)".into())).into()).on_click(cx.listener(|this, _, _, cx| this.export_pin(cx))))
            .child(ui::button("copy", "Copy", false).on_click(cx.listener(|this, _, _, cx| this.export_copy(cx))))
            .child(ui::button("save", "Save", true).on_click(cx.listener(|this, _, _, cx| this.export_save(cx))))
    }

    fn bottom_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

        let text_styles = (self.tool == Tool::Text).then(|| {
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
        });

        let crop_controls = (self.tool == Tool::Crop).then(|| {
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
        });

        let transform = div()
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
            })));

        let resize_controls = self.show_resize.then(|| {
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
        });

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

    fn bg_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
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

    fn canvas_view(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let bounds = self.canvas_bounds.get();
        let (iw, ih) = (self.doc.base.width() as f32, self.doc.base.height() as f32);
        let trim = self.doc.trim();
        let lay = layout(iw, ih, &self.doc.backdrop, trim);
        let sf = window.scale_factor();
        let avail_w = (f32::from(bounds.size.width) - 64.0).max(50.0);
        let avail_h = (f32::from(bounds.size.height) - 64.0).max(50.0);
        let z = (avail_w / lay.canvas_w).min(avail_h / lay.canvas_h).min(1.0 / sf);
        let comp_w = lay.canvas_w * z;
        let comp_h = lay.canvas_h * z;
        let comp_o = point(
            bounds.origin.x + px((f32::from(bounds.size.width) - comp_w) / 2.0),
            bounds.origin.y + px((f32::from(bounds.size.height) - comp_h) / 2.0),
        );
        // where image px (0,0) of the *base* lands on screen
        let img_o = point(comp_o.x + px((lay.img_x - lay.src.x0) * z), comp_o.y + px((lay.img_y - lay.src.y0) * z));
        self.view.set((img_o, z));
        let local = |p: Point<Pixels>| p - bounds.origin;

        let bd = self.doc.backdrop;
        let dst = Bounds::new(
            point(comp_o.x + px(lay.img_x * z), comp_o.y + px(lay.img_y * z)),
            size(px(lay.src.w() * z), px(lay.src.h() * z)),
        );

        // annotations to paint (committed + in-progress)
        let mut anns: Vec<Annotation> = self.doc.annotations.clone();
        if let Some(d) = &self.drawing {
            anns.push(d.clone());
        }
        let selected = self.selected;
        let crop = if self.tool == Tool::Crop { self.crop } else { None };
        let editing = self.editing_text;
        let base_w = iw;
        let base_h = ih;
        let paint_anns = anns.clone();
        let cb = self.canvas_bounds.clone();
        let entity_id = cx.entity_id();

        let entity = cx.entity();
        let input_focus = self.focus.clone();
        let wants_input = editing.is_some();
        let dragging = self.drag.is_some();
        let geometry = canvas(
            move |b, window, _| {
                // layout above used last frame's bounds: re-render once they change
                if cb.get() != b {
                    cb.set(b);
                    let id = entity_id;
                    window.on_next_frame(move |_, cx| cx.notify(id));
                }
            },
            move |b, _, window, cx| {
                if wants_input {
                    window.handle_input(&input_focus, ElementInputHandler::new(b, entity.clone()), cx);
                }
                if dragging {
                    // keep following the pointer after it leaves the canvas
                    let entity = entity.clone();
                    window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
                        if phase == DispatchPhase::Capture && !b.contains(&e.position) {
                            entity.update(cx, |this, cx| this.on_move(e, window, cx));
                        }
                    });
                }
                let map = |p: Pt| point(img_o.x + px(p.x * z), img_o.y + px(p.y * z));
                let mrect = |r: Rect| Bounds::from_corners(map(pt(r.x0, r.y0)), map(pt(r.x1, r.y1)));
                window.with_content_mask(Some(ContentMask { bounds: dst }), |window| {
                    // spotlight
                    let spots: Vec<Rect> = paint_anns.iter().filter_map(|a| if let Shape::Spotlight(r) = a.shape { Some(r) } else { None }).collect();
                    for c in spotlight_dim_cells(base_w, base_h, &spots) {
                        window.paint_quad(fill(mrect(c), rgba(0x00000080)));
                    }
                    for a in &paint_anns {
                        paint_annotation(window, a, &map, z);
                    }
                });
                // selection outline
                if let Some(id) = selected {
                    if editing.is_none() || editing != Some(id) {
                        if let Some(a) = paint_anns.iter().find(|a| a.id == id) {
                            let b = mrect(a.bounds().inflate(4.0 / z));
                            window.paint_quad(quad(b, px(4.), transparent_black(), px(1.5), color::accent(), BorderStyle::Dashed));
                        }
                    }
                }
                if let Some(r) = crop {
                    let full = mrect(Rect { x0: 0.0, y0: 0.0, x1: base_w, y1: base_h });
                    let c = mrect(r);
                    let dim = rgba(0x000000A0);
                    let (x0, y0, x1, y1) = (c.origin.x, c.origin.y, c.origin.x + c.size.width, c.origin.y + c.size.height);
                    let (fx0, fy0, fx1, fy1) = (full.origin.x, full.origin.y, full.origin.x + full.size.width, full.origin.y + full.size.height);
                    for b in [
                        Bounds::from_corners(point(fx0, fy0), point(fx1, y0)),
                        Bounds::from_corners(point(fx0, y1), point(fx1, fy1)),
                        Bounds::from_corners(point(fx0, y0), point(x0, y1)),
                        Bounds::from_corners(point(x1, y0), point(fx1, y1)),
                    ] {
                        if b.size.width > px(0.) && b.size.height > px(0.) {
                            window.paint_quad(fill(b, dim));
                        }
                    }
                    window.paint_quad(outline(c, rgb(0xFFFFFF), BorderStyle::Solid));
                    // rule of thirds
                    for i in 1..3 {
                        let fx = x0 + (x1 - x0) * (i as f32 / 3.0);
                        let fy = y0 + (y1 - y0) * (i as f32 / 3.0);
                        window.paint_quad(fill(Bounds::new(point(fx, y0), size(px(1.), y1 - y0)), rgba(0xFFFFFF55)));
                        window.paint_quad(fill(Bounds::new(point(x0, fy), size(x1 - x0, px(1.))), rgba(0xFFFFFF55)));
                    }
                    for (hx, hy) in [(x0, y0), (x1, y0), (x0, y1), (x1, y1)] {
                        window.paint_quad(fill(Bounds::new(point(hx - px(4.), hy - px(4.)), size(px(8.), px(8.))), rgb(0xFFFFFF)));
                    }
                }
            },
        )
        .absolute()
        .size_full();

        // text + counters as real text elements
        let mut labels: Vec<AnyElement> = vec![];
        for a in &anns {
            match &a.shape {
                Shape::Text { pos, text, style } => {
                    let o = point(img_o.x + px(pos.x * z), img_o.y + px(pos.y * z)) - dst.origin;
                    labels.push(text_element(a, text, *style, o, z, editing == Some(a.id)).into_any_element());
                }
                Shape::Counter { c, n } => {
                    let r = a.counter_radius() * z;
                    let o = point(img_o.x + px(c.x * z - r), img_o.y + px(c.y * z - r)) - dst.origin;
                    let fg = render::contrast_on(a.color);
                    labels.push(
                        div()
                            .absolute()
                            .left(o.x)
                            .top(o.y)
                            .size(px(r * 2.0))
                            .rounded_full()
                            .bg(ui::hsla_of(a.color))
                            .border(px((r * 0.12).max(1.0)))
                            .border_color(rgb(0xFFFFFF))
                            .shadow_md()
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(px(r * 1.15))
                            .line_height(px(r * 2.0))
                            .font_weight(FontWeight::BOLD)
                            .text_color(ui::hsla_of(fg))
                            .child(n.to_string())
                            .into_any_element(),
                    );
                }
                _ => {}
            }
        }

        let (angle, c0, c1) = BACKGROUNDS[bd.preset % BACKGROUNDS.len()];
        let lo = local(comp_o);
        let ld = local(dst.origin);
        let rad = if bd.enabled { px(bd.radius * z) } else { px(0.) };

        div()
            .id("canvas")
            .flex_1()
            .h_full()
            .relative()
            .overflow_hidden()
            .bg(color::canvas_bg())
            .cursor(match self.tool {
                Tool::Select => CursorStyle::Arrow,
                Tool::Text => CursorStyle::IBeam,
                _ => CursorStyle::Crosshair,
            })
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_down))
            .on_mouse_move(cx.listener(Self::on_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_up_out))
            .when(bd.enabled, |d| {
                d.child(
                    div()
                        .absolute()
                        .left(lo.x)
                        .top(lo.y)
                        .w(px(comp_w))
                        .h(px(comp_h))
                        .bg(linear_gradient(angle, linear_color_stop(ui::hex_hsla(c0), 0.0), linear_color_stop(ui::hex_hsla(c1), 1.0))),
                )
            })
            .child(
                div()
                    .absolute()
                    .left(ld.x)
                    .top(ld.y)
                    .w(dst.size.width)
                    .h(dst.size.height)
                    .rounded(rad)
                    .overflow_hidden()
                    .when(bd.enabled && bd.shadow, |d| d.shadow_xl())
                    .when(!bd.enabled, |d| d.shadow_lg())
                    .child(match (&trim, &self.display_trim) {
                        // trimmed: draw the cropped copy so its own corners round like the export
                        (Some(_), Some((_, t))) => img(t.clone()).absolute().left_0().top_0().w(dst.size.width).h(dst.size.height).rounded(rad),
                        _ => img(self.display.clone())
                            .absolute()
                            .left(px(-lay.src.x0 * z))
                            .top(px(-lay.src.y0 * z))
                            .w(px(iw * z))
                            .h(px(ih * z))
                            .rounded(if trim.is_none() { rad } else { px(0.) }),
                    }),
            )
            .child(geometry)
            .child(div().absolute().left(ld.x).top(ld.y).w(dst.size.width).h(dst.size.height).overflow_hidden().children(labels))
            .when(self.eyedropper, |d| {
                d.child(
                    div().absolute().top(px(12.)).left_0().right_0().flex().justify_center().child(
                        div().px(px(12.)).py(px(6.)).rounded(px(8.)).bg(color::hud()).text_size(px(12.)).text_color(color::text()).child("Click anywhere on the image to pick a color"),
                    ),
                )
            })
    }
}

struct Tip(SharedString);
impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(8.))
            .py(px(4.))
            .rounded(px(6.))
            .bg(rgb(0x3A3A3C))
            .border_1()
            .border_color(color::hud_border())
            .text_size(px(12.))
            .font_family(ui::FONT)
            .text_color(color::text())
            .child(self.0.clone())
    }
}

fn text_element(a: &Annotation, text: &str, style: TextStyle, o: Point<Pixels>, z: f32, editing: bool) -> Div {
    let size = a.text_size() * z;
    let lh = size * TEXT_LINE_HEIGHT;
    let shown = if editing { format!("{text}▏") } else { text.to_string() };
    let shown = if shown.is_empty() { " ".to_string() } else { shown };
    let mut fg = a.color;
    let mut bg: Option<Rgba> = None;
    match style {
        TextStyle::Pill => {
            bg = Some(a.color);
            fg = render::contrast_on(a.color);
        }
        TextStyle::Card => {
            bg = Some([255, 255, 255, 255]);
            if a.color == [255, 255, 255, 255] {
                fg = [28, 28, 30, 255];
            }
        }
        _ => {}
    }
    let lines = |color: Rgba| {
        let mut d = div().flex().flex_col();
        for l in shown.split('\n') {
            d = d.child(
                div()
                    .h(px(lh))
                    .line_height(px(lh))
                    .whitespace_nowrap()
                    .text_color(ui::hsla_of(color))
                    .child(if l.is_empty() { " ".to_string() } else { l.to_string() }),
            );
        }
        d
    };
    let pad = size * 0.35;
    let mut el = div()
        .absolute()
        .left(o.x - px(if bg.is_some() { pad } else { 0.0 }))
        .top(o.y - px(if bg.is_some() { pad * 0.5 } else { 0.0 }))
        .font_family(ui::FONT)
        .font_weight(FontWeight::BOLD)
        .text_size(px(size));
    if let Some(b) = bg {
        el = el.px(px(pad)).py(px(pad * 0.5)).rounded(px(size * 0.3)).bg(ui::hsla_of(b)).shadow_md();
    }
    if style == TextStyle::Outline {
        let oc = render::contrast_on(fg);
        let off = (size * 0.06).max(1.0);
        let mut stack = div().relative();
        for (dx, dy) in [(-off, 0.0), (off, 0.0), (0.0, -off), (0.0, off), (-off, -off), (off, off), (-off, off), (off, -off)] {
            stack = stack.child(lines(oc).absolute().left(px(dx)).top(px(dy)));
        }
        el = el.child(stack.child(lines(fg)));
    } else {
        el = el.child(lines(fg));
    }
    if editing {
        el = el.border_1().border_color(color::accent()).rounded(px(4.));
    }
    el
}

fn stroke_path(points: &[Point<Pixels>], width: f32, square: bool) -> Option<Path<Pixels>> {
    use lyon::tessellation::{LineCap, LineJoin};
    let cap = if square { LineCap::Square } else { LineCap::Round };
    let opts = StrokeOptions::default().with_line_width(width).with_line_cap(cap).with_line_join(LineJoin::Round);
    let mut b = PathBuilder::stroke(px(width)).with_style(PathStyle::Stroke(opts));
    let first = points.first()?;
    b.move_to(*first);
    if points.len() == 1 {
        b.line_to(*first + point(px(0.1), px(0.1)));
    }
    for p in &points[1..] {
        b.line_to(*p);
    }
    b.build().ok()
}

fn paint_annotation(window: &mut Window, a: &Annotation, map: &dyn Fn(Pt) -> Point<Pixels>, z: f32) {
    let col = ui::hsla_of(a.color);
    let w = a.width * z;
    let shadow = hsla(0., 0., 0., 0.22);
    let shift = |p: Point<Pixels>| p + point(px(0.), px((a.width * 0.35).max(1.0) * z));
    match &a.shape {
        Shape::Arrow { a: p0, b } => {
            let poly: Vec<Point<Pixels>> = arrow_polygon(*p0, *b, a.width).into_iter().map(map).collect();
            for (pts, c) in [(poly.iter().map(|p| shift(*p)).collect::<Vec<_>>(), shadow), (poly.clone(), col)] {
                let mut pb = PathBuilder::fill();
                pb.add_polygon(&pts, true);
                if let Ok(path) = pb.build() {
                    window.paint_path(path, c);
                }
                if let Some(path) = stroke_path(&[pts[0], pts[1], pts[2], pts[3], pts[4], pts[5], pts[6], pts[0]], w * 0.35, false) {
                    window.paint_path(path, c);
                }
            }
        }
        Shape::Line { a: p0, b } => {
            if let Some(path) = stroke_path(&[shift(map(*p0)), shift(map(*b))], w, false) {
                window.paint_path(path, shadow);
            }
            if let Some(path) = stroke_path(&[map(*p0), map(*b)], w, false) {
                window.paint_path(path, col);
            }
        }
        Shape::Rect(r) => {
            let b = Bounds::from_corners(map(pt(r.x0, r.y0)), map(pt(r.x1, r.y1)));
            let half = px(w / 2.0);
            let outer = Bounds::new(b.origin - point(half, half), b.size + size(px(w), px(w)));
            let rad = px(a.width.min(6.0) * z + w / 2.0);
            let sh = Bounds::new(shift(outer.origin), outer.size);
            window.paint_quad(quad(sh, rad, transparent_black(), px(w), shadow, BorderStyle::Solid));
            window.paint_quad(quad(outer, rad, transparent_black(), px(w), col, BorderStyle::Solid));
        }
        Shape::FilledRect(r) => {
            let b = Bounds::from_corners(map(pt(r.x0, r.y0)), map(pt(r.x1, r.y1)));
            window.paint_quad(quad(b, px(a.width.min(6.0) * z), col, px(0.), transparent_black(), BorderStyle::Solid));
        }
        Shape::Ellipse(r) => {
            let c = map(pt((r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0));
            let (rx, ry) = (r.w() * z / 2.0, r.h() * z / 2.0);
            let n = ((rx + ry) / 2.0).clamp(24.0, 160.0) as usize;
            let pts: Vec<Point<Pixels>> = (0..=n)
                .map(|i| {
                    let t = i as f32 / n as f32 * std::f32::consts::TAU;
                    c + point(px(rx * t.cos()), px(ry * t.sin()))
                })
                .collect();
            if let Some(path) = stroke_path(&pts.iter().map(|p| shift(*p)).collect::<Vec<_>>(), w, false) {
                window.paint_path(path, shadow);
            }
            if let Some(path) = stroke_path(&pts, w, false) {
                window.paint_path(path, col);
            }
        }
        Shape::Pencil(ps) => {
            let pts: Vec<Point<Pixels>> = ps.iter().map(|p| map(*p)).collect();
            if let Some(path) = stroke_path(&pts.iter().map(|p| shift(*p)).collect::<Vec<_>>(), w, false) {
                window.paint_path(path, shadow);
            }
            if let Some(path) = stroke_path(&pts, w, false) {
                window.paint_path(path, col);
            }
        }
        Shape::Highlighter(ps) => {
            let pts: Vec<Point<Pixels>> = ps.iter().map(|p| map(*p)).collect();
            let mut c = col;
            c.a = 0.43;
            if let Some(path) = stroke_path(&pts, a.highlighter_width() * z, true) {
                window.paint_path(path, c);
            }
        }
        Shape::Blur(r) | Shape::Pixelate(r) => {
            // committed effects are baked into the display image; show the frame while drawing
            let b = Bounds::from_corners(map(pt(r.x0, r.y0)), map(pt(r.x1, r.y1)));
            window.paint_quad(quad(b, px(2.), transparent_black(), px(1.), rgba(0xFFFFFF40), BorderStyle::Dashed));
        }
        Shape::Spotlight(_) | Shape::Text { .. } | Shape::Counter { .. } => {}
    }
}

fn store_edited(c: &Capture, cx: &mut App) {
    let img = c.image.clone();
    let task = cx.background_executor().spawn(async move { History::make_entry(&img, CaptureKind::Edited) });
    cx.spawn(async move |cx: &mut AsyncApp| {
        if let Ok(e) = task.await {
            let _ = cx.update(|cx| app::add_history_entry(e, cx));
        }
    })
    .detach();
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = format!(
            "{}{}",
            self.capture.file_name(&state(cx).settings.file_prefix).trim_end_matches(".png"),
            if self.dirty { " — Edited" } else { "" }
        );
        if self.drag.is_none() {
            // keep baked effects / trimmed copy in sync (skipped mid-drag: re-baking blur per move is slow)
            self.refresh_display(window);
        }
        let toolbar = self.toolbar(cx).into_any_element();
        let actions = self.actions(cx).into_any_element();
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(color::window_bg())
            .font_family(ui::FONT)
            .text_color(color::text())
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .child(ui::title_bar(title, Some(toolbar), Some(actions)))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.canvas_view(window, cx))
                    .when(self.show_bg, |d| d.child(self.bg_panel(cx))),
            )
            .child(self.bottom_bar(cx))
    }
}

impl Editor {
    fn edited_text_mut(&mut self) -> Option<&mut String> {
        let id = self.editing_text?;
        match self.doc.annotations.iter_mut().find(|a| a.id == id) {
            Some(Annotation { shape: Shape::Text { text, .. }, .. }) => Some(text),
            _ => None,
        }
    }

    fn insert_text(&mut self, s: &str, mark: bool, cx: &mut Context<Self>) {
        let marked = std::mem::take(&mut self.ime_marked);
        let clean: String = s.chars().filter(|c| !c.is_control() || *c == '\n').collect();
        if let Some(text) = self.edited_text_mut() {
            let mut keep = text.len().saturating_sub(marked);
            while !text.is_char_boundary(keep) {
                keep -= 1;
            }
            text.truncate(keep);
            text.push_str(&clean);
            if mark {
                self.ime_marked = clean.len();
            }
            self.dirty = true;
            cx.notify();
        }
    }
}

/// Text annotations receive typed characters (including IME composition and
/// non-ASCII input) through GPUI's platform input handler.
impl EntityInputHandler for Editor {
    fn text_for_range(&mut self, range: std::ops::Range<usize>, adjusted: &mut Option<std::ops::Range<usize>>, _: &mut Window, _: &mut Context<Self>) -> Option<String> {
        let text = self.edited_text_mut()?.clone();
        let utf16: Vec<u16> = text.encode_utf16().collect();
        let r = range.start.min(utf16.len())..range.end.min(utf16.len());
        *adjusted = Some(r.clone());
        Some(String::from_utf16_lossy(&utf16[r]))
    }

    fn selected_text_range(&mut self, _: bool, _: &mut Window, _: &mut Context<Self>) -> Option<UTF16Selection> {
        let n = self.edited_text_mut()?.encode_utf16().count();
        Some(UTF16Selection { range: n..n, reversed: false })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<std::ops::Range<usize>> {
        if self.ime_marked == 0 {
            return None;
        }
        let id = self.editing_text?;
        let text = match self.doc.annotations.iter().find(|a| a.id == id) {
            Some(Annotation { shape: Shape::Text { text, .. }, .. }) => text,
            _ => return None,
        };
        let mut start = text.len().saturating_sub(self.ime_marked);
        while !text.is_char_boundary(start) {
            start -= 1;
        }
        let s16 = text[..start].encode_utf16().count();
        let e16 = text.encode_utf16().count();
        Some(s16..e16)
    }

    fn unmark_text(&mut self, _: &mut Window, _: &mut Context<Self>) {
        self.ime_marked = 0;
    }

    fn replace_text_in_range(&mut self, _: Option<std::ops::Range<usize>>, text: &str, _: &mut Window, cx: &mut Context<Self>) {
        self.insert_text(text, false, cx);
    }

    fn replace_and_mark_text_in_range(&mut self, _: Option<std::ops::Range<usize>>, text: &str, _: Option<std::ops::Range<usize>>, _: &mut Window, cx: &mut Context<Self>) {
        self.insert_text(text, true, cx);
    }

    fn bounds_for_range(&mut self, _: std::ops::Range<usize>, element_bounds: Bounds<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<Bounds<Pixels>> {
        let (origin, z) = self.view.get();
        let id = self.editing_text?;
        let a = self.doc.annotations.iter().find(|a| a.id == id)?;
        let r = a.bounds();
        let _ = element_bounds;
        Some(Bounds::new(point(origin.x + px(r.x1 * z), origin.y + px(r.y0 * z)), size(px(2.), px((r.y1 - r.y0) * z))))
    }

    fn character_index_for_point(&mut self, _: Point<Pixels>, _: &mut Window, _: &mut Context<Self>) -> Option<usize> {
        None
    }
}