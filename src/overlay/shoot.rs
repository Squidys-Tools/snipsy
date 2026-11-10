use super::{close_all, OverlayView};
use crate::app::{self, state, state_mut, Mode};
use crate::capture::{self, PRect};
use crate::history::CaptureKind;
use crate::toast;
use gpui::*;
use image::RgbaImage;
use std::sync::Arc;

impl OverlayView {
    pub(super) fn crop(&self, b: Bounds<Pixels>) -> Option<(RgbaImage, PRect)> {
        let r = self.to_image_rect(b);
        if r.w < 1 || r.h < 1 {
            return None;
        }
        let img = image::imageops::crop_imm(&*self.image, r.x as u32, r.y as u32, r.w as u32, r.h as u32).to_image();
        let global = PRect { x: r.x + self.monitor.rect.x, y: r.y + self.monitor.rect.y, w: r.w, h: r.h };
        Some((img, global))
    }

    pub(super) fn capture_selection(&mut self, kind: CaptureKind, cx: &mut Context<Self>) {
        let Some(sel) = self.selection else { return };
        let Some((img, global)) = self.crop(sel) else { return };
        state_mut(cx).settings.last_area = Some((global.x, global.y, global.w, global.h));
        state(cx).settings.save();
        let ocr = self.mode == Mode::Ocr;
        close_all(cx, move |cx| {
            if ocr {
                app::recognize_and_copy(Arc::new(img), cx);
            } else {
                app::finish(img, kind, cx);
            }
        });
    }

    pub(super) fn capture_fullscreen(&mut self, cx: &mut Context<Self>) {
        let img = (*self.image).clone();
        close_all(cx, move |cx| app::finish(img, CaptureKind::Fullscreen, cx));
    }

    pub(super) fn capture_window(&mut self, cx: &mut Context<Self>) {
        let Some(info) = self.hovered_window.clone() else { return };
        let scale = self.scale;
        let fallback = self.local_from_global(info.rect).and_then(|b| self.crop(b)).map(|c| c.0);
        let (shadow, background) = (state(cx).settings.window_shadow, state(cx).settings.window_background);
        // window_padding is in logical px around the window
        let pad_px = state(cx).settings.window_padding as f32 * scale;
        close_all(cx, move |cx| {
            let raw = capture::capture_window(&info).ok().or(fallback);
            let Some(mut img) = raw else {
                toast::show(cx, "Capture failed", "Could not capture that window");
                return;
            };
            capture::round_corners(&mut img, 8.0 * scale);
            let img = if background {
                // Backdrop padding is a fraction of the shorter image side.
                let padding = pad_px / img.width().min(img.height()).max(1) as f32;
                let mut doc = crate::annotate::doc::Doc::new(Arc::new(img));
                doc.backdrop.enabled = true;
                doc.backdrop.radius = 0.0;
                doc.backdrop.shadow = true;
                doc.backdrop.padding = padding;
                crate::annotate::render::render(&doc)
            } else if shadow {
                crate::annotate::render::window_shadow(&img, scale)
            } else {
                img
            };
            app::finish(img, CaptureKind::Window, cx);
        });
    }
}
