use crate::annotate::render;
use crate::app::{self, state, Capture};
use crate::history::{CaptureKind, History};
use crate::{pin, platform, toast};
use gpui::prelude::*;
use gpui::*;
use image::RgbaImage;
use std::sync::Arc;
use super::Editor;

impl Editor {
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

    pub(super) fn export_copy(&mut self, cx: &mut Context<Self>) {
        self.render_async(cx, |img, cx| match platform::copy_image(&img) {
            Ok(()) => toast::show(cx, "Copied to clipboard", format!("{} × {}", img.width(), img.height())),
            Err(e) => toast::show(cx, "Copy failed", e.to_string()),
        });
    }

    pub(super) fn export_save(&mut self, cx: &mut Context<Self>) {
        self.dirty = false;
        let created = self.capture.created;
        self.render_async(cx, move |img, cx| {
            let mut c = Capture::new(Arc::new(img), CaptureKind::Edited);
            c.created = created;
            app::save_to_export(&c, cx);
            store_edited(&c, cx);
        });
    }

    pub(super) fn export_save_as(&mut self, _: &mut Window, cx: &mut Context<Self>) {
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

    pub(super) fn export_pin(&mut self, cx: &mut Context<Self>) {
        self.render_async(cx, |img, cx| pin::open(Capture::new(Arc::new(img), CaptureKind::Edited), cx));
    }

    pub(super) fn export_drag(&mut self, window: &mut Window, cx: &mut Context<Self>) {
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
