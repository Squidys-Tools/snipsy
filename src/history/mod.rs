pub mod view;

use crate::settings::data_dir;
use chrono::{DateTime, Local};
use image::RgbaImage;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureKind {
    Area,
    Fullscreen,
    Window,
    Timer,
    Edited,
}

impl CaptureKind {
    pub fn label(self) -> &'static str {
        match self {
            CaptureKind::Area => "Area",
            CaptureKind::Fullscreen => "Fullscreen",
            CaptureKind::Window => "Window",
            CaptureKind::Timer => "Timer",
            CaptureKind::Edited => "Edited",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: u64,
    pub kind: CaptureKind,
    pub created: DateTime<Local>,
    pub width: u32,
    pub height: u32,
    pub file: PathBuf,
    pub thumb: PathBuf,
}

#[derive(Default, Serialize, Deserialize)]
pub struct History {
    pub entries: Vec<HistoryEntry>,
}

fn dir() -> PathBuf {
    let d = data_dir().join("history");
    let _ = std::fs::create_dir_all(&d);
    d
}

impl History {
    fn index() -> PathBuf {
        data_dir().join("history.json")
    }

    pub fn load(retention_days: u32) -> Self {
        let path = Self::index();
        let mut h: History = match std::fs::read_to_string(&path) {
            Ok(t) => match serde_json::from_str(&t) {
                Ok(h) => h,
                Err(e) => {
                    // Keep the unreadable index instead of overwriting it, and
                    // rebuild what we can from the stored PNGs.
                    eprintln!("history.json unreadable ({e}); backing up and rebuilding");
                    let _ = std::fs::copy(&path, path.with_extension("json.bak"));
                    Self::rebuild()
                }
            },
            Err(_) => History::default(),
        };
        let cutoff = Local::now() - chrono::Duration::days(retention_days.max(1) as i64);
        let (keep, drop): (Vec<_>, Vec<_>) =
            h.entries.into_iter().partition(|e| e.created > cutoff && e.file.exists());
        for e in drop {
            let _ = std::fs::remove_file(&e.file);
            let _ = std::fs::remove_file(&e.thumb);
        }
        h.entries = keep;
        h.save();
        h
    }

    fn rebuild() -> Self {
        let d = dir();
        let mut entries: Vec<HistoryEntry> = std::fs::read_dir(&d)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|f| {
                let file = f.path();
                if file.extension().is_none_or(|x| x != "png") {
                    return None;
                }
                let id: u64 = file.file_stem()?.to_str()?.parse().ok()?;
                let (width, height) = image::image_dimensions(&file).ok()?;
                let created = DateTime::from_timestamp_nanos(id as i64).with_timezone(&Local);
                Some(HistoryEntry { id, kind: CaptureKind::Area, created, width, height, thumb: d.join(format!("{id}_t.png")), file })
            })
            .collect();
        entries.sort_by(|a, b| b.created.cmp(&a.created));
        History { entries }
    }

    /// Atomic write (temp file + rename) so a crash mid-save can't truncate the index.
    pub fn save(&self) {
        if let Ok(t) = serde_json::to_string(self) {
            let p = Self::index();
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, t).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }

    /// Store a capture. PNG encoding happens on the caller's (background) thread.
    pub fn make_entry(img: &RgbaImage, kind: CaptureKind) -> anyhow::Result<HistoryEntry> {
        let id = Local::now().timestamp_nanos_opt().unwrap_or_default() as u64;
        let file = dir().join(format!("{id}.png"));
        let thumb = dir().join(format!("{id}_t.png"));
        img.save(&file)?;
        let t = thumbnail(img, 480);
        t.save(&thumb)?;
        Ok(HistoryEntry {
            id,
            kind,
            created: Local::now(),
            width: img.width(),
            height: img.height(),
            file,
            thumb,
        })
    }

    pub fn push(&mut self, e: HistoryEntry) {
        self.entries.insert(0, e);
        self.save();
    }

    pub fn remove(&mut self, id: u64) {
        if let Some(i) = self.entries.iter().position(|e| e.id == id) {
            let e = self.entries.remove(i);
            let _ = std::fs::remove_file(&e.file);
            let _ = std::fs::remove_file(&e.thumb);
            self.save();
        }
    }

    pub fn clear(&mut self) {
        for e in self.entries.drain(..) {
            let _ = std::fs::remove_file(&e.file);
            let _ = std::fs::remove_file(&e.thumb);
        }
        self.save();
    }
}

pub fn thumbnail(img: &RgbaImage, max: u32) -> RgbaImage {
    let (w, h) = (img.width(), img.height());
    let s = (max as f32 / w.max(h) as f32).min(1.0);
    let (nw, nh) = (((w as f32 * s) as u32).max(1), ((h as f32 * s) as u32).max(1));
    image::imageops::resize(img, nw, nh, image::imageops::FilterType::Triangle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_index_roundtrips_entries() {
        let e = HistoryEntry {
            id: 42,
            kind: CaptureKind::Timer,
            created: Local::now(),
            width: 640,
            height: 480,
            file: PathBuf::from("a/42.png"),
            thumb: PathBuf::from("a/42_t.png"),
        };
        let h = History { entries: vec![e.clone()] };
        let back: History = serde_json::from_str(&serde_json::to_string(&h).unwrap()).unwrap();
        assert_eq!(back.entries.len(), 1);
        let b = &back.entries[0];
        assert_eq!((b.id, b.kind, b.created, b.width, b.height), (42, CaptureKind::Timer, e.created, 640, 480));
        assert_eq!((&b.file, &b.thumb), (&e.file, &e.thumb));
    }

    #[test]
    fn thumbnail_fits_max_keeps_aspect_never_upscales_and_never_collapses() {
        let t = thumbnail(&RgbaImage::new(1000, 500), 480);
        assert_eq!((t.width(), t.height()), (480, 240));
        let small = thumbnail(&RgbaImage::new(100, 50), 480);
        assert_eq!((small.width(), small.height()), (100, 50));
        let thin = thumbnail(&RgbaImage::new(1, 5000), 480);
        assert_eq!((thin.width(), thin.height()), (1, 480));
    }
}
