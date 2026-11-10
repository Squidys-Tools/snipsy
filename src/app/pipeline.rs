//! Capture -> after-capture pipeline: history, clipboard, export folder, OCR.

use super::{state, state_mut};
use crate::capture::{self, PRect};
use crate::history::{self, CaptureKind, History, HistoryEntry};
use crate::settings::AfterCapture;
use crate::{annotate, platform, quick, toast, ui};
use gpui::*;
use image::RgbaImage;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone)]
pub struct Capture {
    pub id: u64,
    pub image: Arc<RgbaImage>,
    pub preview: Arc<RenderImage>,
    pub kind: CaptureKind,
    pub created: chrono::DateTime<chrono::Local>,
    /// Stored history PNG (filled in once the background save completes).
    pub file: Option<PathBuf>,
}

/// Timestamp-based capture id, bumped so two captures never share one.
pub fn next_capture_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static LAST: AtomicU64 = AtomicU64::new(0);
    let now = chrono::Local::now().timestamp_nanos_opt().unwrap_or_default() as u64;
    let prev = LAST.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |last| Some(now.max(last + 1))).unwrap_or_default();
    now.max(prev + 1)
}

impl Capture {
    pub fn new(image: Arc<RgbaImage>, kind: CaptureKind) -> Self {
        let thumb = history::thumbnail(&image, 560);
        Self {
            id: next_capture_id(),
            preview: ui::to_render_image(&thumb),
            image,
            kind,
            created: chrono::Local::now(),
            file: None,
        }
    }

    pub fn file_name(&self, prefix: &str) -> String {
        format!("{} {}.png", prefix, self.created.format("%Y-%m-%d at %H.%M.%S"))
    }
}

/// Capture a desktop rect without UI (fullscreen, previous area, timer).
pub fn capture_rect_now(r: PRect, kind: CaptureKind, cx: &mut App) {
    match capture::capture_rect(r) {
        Ok(img) => finish(img, kind, cx),
        Err(e) => toast::show(cx, "Capture failed", e.to_string()),
    }
}

pub fn finish(img: RgbaImage, kind: CaptureKind, cx: &mut App) {
    if state(cx).settings.play_sounds {
        platform::play_shutter();
    }
    let capture = Capture::new(Arc::new(img), kind);
    store_in_history(&capture, cx);

    let s = &state(cx).settings;
    if s.also_copy_to_clipboard {
        let _ = platform::copy_image(&capture.image);
    }
    match s.after_capture {
        AfterCapture::QuickAccess => quick::push(capture, cx),
        AfterCapture::Copy => {
            match platform::copy_image(&capture.image) {
                Ok(()) => toast::show(cx, "Copied to clipboard", format!("{} × {}", capture.image.width(), capture.image.height())),
                Err(e) => toast::show(cx, "Copy failed", e.to_string()),
            }
        }
        AfterCapture::Save => save_to_export(&capture, cx),
        AfterCapture::Annotate => annotate::open(capture, cx),
    }
}

fn store_in_history(capture: &Capture, cx: &mut App) {
    let img = capture.image.clone();
    let kind = capture.kind;
    let id = capture.id;
    let task = cx.background_executor().spawn(async move { History::make_entry(&img, kind) });
    cx.spawn(async move |cx: &mut AsyncApp| {
        if let Ok(mut entry) = task.await {
            let _ = cx.update(|cx| {
                entry.id = id;
                let file = entry.file.clone();
                state_mut(cx).history.push(entry);
                quick::set_file(id, file, cx);
                history::view::refresh(cx);
            });
        }
    })
    .detach();
}

pub fn add_history_entry(entry: HistoryEntry, cx: &mut App) {
    state_mut(cx).history.push(entry);
    history::view::refresh(cx);
}

/// Save to the export folder with a CleanShot-like file name.
pub fn save_to_export(capture: &Capture, cx: &mut App) {
    let dir = state(cx).settings.export_dir.clone();
    let name = capture.file_name(&state(cx).settings.file_prefix);
    let img = capture.image.clone();
    let task = cx.background_executor().spawn(async move {
        let _ = std::fs::create_dir_all(&dir);
        let path = unique_path(dir.join(name));
        img.save(&path).map(|_| path)
    });
    cx.spawn(async move |cx: &mut AsyncApp| {
        let res = task.await;
        let _ = cx.update(|cx| match res {
            Ok(path) => toast::show_with_path(cx, "Saved", path.display().to_string(), path.clone()),
            Err(e) => toast::show(cx, "Save failed", e.to_string()),
        });
    })
    .detach();
}

pub fn unique_path(p: PathBuf) -> PathBuf {
    if !p.exists() {
        return p;
    }
    let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = p.extension().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "png".into());
    let dir = p.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    (2..).map(|i| dir.join(format!("{stem} ({i}).{ext}"))).find(|q| !q.exists()).unwrap()
}

/// %TEMP%\Snipsy, holding the drag & drop files (cleared at startup).
pub fn temp_root() -> PathBuf {
    std::env::temp_dir().join("Snipsy")
}

/// Write a capture under a friendly name in %TEMP% (used for drag & drop).
/// Each capture gets its own id-named subfolder, so captures taken in the same
/// second (same friendly name) never share a temp file.
pub fn temp_file_for(capture: &Capture, prefix: &str) -> Option<PathBuf> {
    let dir = temp_root().join(capture.id.to_string());
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(capture.file_name(prefix));
    if path.exists() {
        // Same id means same pixels; an earlier drag already wrote it.
        return Some(path);
    }
    if let Some(f) = &capture.file {
        if std::fs::copy(f, &path).is_ok() {
            return Some(path);
        }
    }
    capture.image.save(&path).ok().map(|_| path)
}

pub fn copy_capture(capture: &Capture, cx: &mut App) {
    match platform::copy_image(&capture.image) {
        Ok(()) => toast::show(cx, "Copied to clipboard", format!("{} × {}", capture.image.width(), capture.image.height())),
        Err(e) => toast::show(cx, "Copy failed", e.to_string()),
    }
}

pub fn recognize_and_copy(img: Arc<RgbaImage>, cx: &mut App) {
    let task = cx.background_executor().spawn(async move {
        std::thread::spawn(move || platform::recognize_text(&img)).join().unwrap_or_else(|_| Err(anyhow::anyhow!("OCR crashed")))
    });
    cx.spawn(async move |cx: &mut AsyncApp| {
        let res = task.await;
        let _ = cx.update(|cx| match res {
            Ok(text) if !text.trim().is_empty() => {
                let _ = platform::copy_text(&text);
                let preview: String = text.lines().take(2).collect::<Vec<_>>().join(" · ");
                let preview = if preview.chars().count() > 80 { format!("{}…", preview.chars().take(80).collect::<String>()) } else { preview };
                toast::show(cx, "Text copied to clipboard", preview);
            }
            Ok(_) => toast::show(cx, "No text found", "Try selecting a larger area"),
            Err(e) => toast::show(cx, "Text recognition failed", e.to_string()),
        });
    })
    .detach();
}
