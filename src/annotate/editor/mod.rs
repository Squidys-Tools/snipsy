//! The Annotate window: CleanShot-style dark editor.

mod backdrop_panel;
mod canvas;
mod export;
mod input;
mod inspector;
mod text_input;
mod toolbar;
mod transform;
mod undo;

use crate::annotate::doc::*;
use crate::annotate::doc::{Rgba, TextStyle};
use crate::annotate::render;
use crate::app::{self, state, Capture};
use crate::ui::{self, color};
use crate::{capture, platform};
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

    fn selected_mut(&mut self) -> Option<&mut Annotation> {
        let id = self.selected?;
        self.doc.annotations.iter_mut().find(|a| a.id == id)
    }

    fn new_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    fn to_img(&self, p: Point<Pixels>) -> Pt {
        let (o, z) = self.view.get();
        pt(f32::from(p.x - o.x) / z, f32::from(p.y - o.y) / z)
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
