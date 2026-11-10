//! Persistent user settings (%APPDATA%\Snipsy\settings.json).

mod autostart;
mod hotkey_recorder;
pub mod view;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OverlayCorner {
    BottomLeft,
    BottomRight,
    TopLeft,
    TopRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AfterCapture {
    /// Show the Quick Access Overlay.
    QuickAccess,
    /// Copy straight to the clipboard.
    Copy,
    /// Save straight to the export folder.
    Save,
    /// Open the annotation editor.
    Annotate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HotkeyAction {
    AllInOne,
    CaptureArea,
    CaptureFullscreen,
    CaptureWindow,
    CapturePreviousArea,
    SelfTimer,
    CaptureText,
    OpenHistory,
    RestoreLastOverlay,
    ToggleOverlays,
}

impl HotkeyAction {
    pub const ALL: [HotkeyAction; 10] = [
        HotkeyAction::AllInOne,
        HotkeyAction::CaptureArea,
        HotkeyAction::CaptureFullscreen,
        HotkeyAction::CaptureWindow,
        HotkeyAction::CapturePreviousArea,
        HotkeyAction::SelfTimer,
        HotkeyAction::CaptureText,
        HotkeyAction::OpenHistory,
        HotkeyAction::RestoreLastOverlay,
        HotkeyAction::ToggleOverlays,
    ];
    pub fn label(self) -> &'static str {
        match self {
            HotkeyAction::AllInOne => "All-In-One",
            HotkeyAction::CaptureArea => "Capture Area",
            HotkeyAction::CaptureFullscreen => "Capture Fullscreen",
            HotkeyAction::CaptureWindow => "Capture Window",
            HotkeyAction::CapturePreviousArea => "Capture Previous Area",
            HotkeyAction::SelfTimer => "Self-Timer",
            HotkeyAction::CaptureText => "Capture Text (OCR)",
            HotkeyAction::OpenHistory => "Open Capture History",
            HotkeyAction::RestoreLastOverlay => "Restore Recently Closed",
            HotkeyAction::ToggleOverlays => "Hide / Show Overlays",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hotkey {
    pub action: HotkeyAction,
    /// global-hotkey accelerator string, e.g. "ctrl+shift+KeyA"
    pub accel: String,
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub export_dir: PathBuf,
    pub after_capture: AfterCapture,
    pub also_copy_to_clipboard: bool,
    pub play_sounds: bool,
    pub overlay_corner: OverlayCorner,
    /// 0.75 ..= 1.5
    pub overlay_scale: f32,
    pub overlay_auto_close: bool,
    pub overlay_auto_close_secs: u32,
    pub show_magnifier: bool,
    pub show_crosshair: bool,
    pub window_shadow: bool,
    pub window_background: bool,
    pub window_padding: u32,
    pub self_timer_secs: u32,
    pub history_days: u32,
    pub file_prefix: String,
    pub dark_editor: bool,
    pub hotkeys: Vec<Hotkey>,
    pub last_area: Option<(i32, i32, i32, i32)>,
    pub custom_colors: Vec<[u8; 4]>,
}

impl Default for Settings {
    fn default() -> Self {
        let export_dir = dirs::desktop_dir()
            .or_else(dirs::picture_dir)
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            export_dir,
            after_capture: AfterCapture::QuickAccess,
            also_copy_to_clipboard: false,
            play_sounds: true,
            overlay_corner: OverlayCorner::BottomLeft,
            overlay_scale: 1.0,
            overlay_auto_close: false,
            overlay_auto_close_secs: 15,
            show_magnifier: true,
            show_crosshair: true,
            window_shadow: true,
            window_background: false,
            window_padding: 48,
            self_timer_secs: 5,
            history_days: 30,
            file_prefix: "Snipsy".into(),
            dark_editor: true,
            hotkeys: default_hotkeys(),
            last_area: None,
            custom_colors: vec![],
        }
    }
}

pub fn default_hotkeys() -> Vec<Hotkey> {
    use HotkeyAction::*;
    let hk = |action, accel: &str| Hotkey { action, accel: accel.into(), enabled: true };
    vec![
        hk(AllInOne, "ctrl+shift+Digit1"),
        hk(CaptureArea, "ctrl+shift+Digit4"),
        hk(CaptureFullscreen, "ctrl+shift+Digit3"),
        hk(CaptureWindow, "ctrl+shift+Digit5"),
        hk(CapturePreviousArea, "ctrl+shift+Digit6"),
        hk(SelfTimer, "ctrl+shift+Digit7"),
        hk(CaptureText, "ctrl+shift+Digit2"),
        hk(OpenHistory, "ctrl+shift+KeyH"),
        hk(RestoreLastOverlay, "ctrl+shift+KeyR"),
        hk(ToggleOverlays, "ctrl+shift+KeyO"),
    ]
}

pub fn data_dir() -> PathBuf {
    let d = dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("Snipsy");
    let _ = std::fs::create_dir_all(&d);
    d
}

impl Settings {
    fn path() -> PathBuf {
        data_dir().join("settings.json")
    }

    pub fn load() -> Self {
        let path = Self::path();
        let mut s: Settings = match std::fs::read_to_string(&path) {
            Ok(t) => serde_json::from_str(&t).unwrap_or_else(|e| {
                // Keep a copy before the next save replaces it with defaults.
                eprintln!("settings.json unreadable ({e}); using defaults, backup at settings.json.bak");
                let _ = std::fs::copy(&path, path.with_extension("json.bak"));
                Settings::default()
            }),
            Err(_) => Settings::default(),
        };
        // make sure every action has a hotkey entry (new actions added later)
        for d in default_hotkeys() {
            if !s.hotkeys.iter().any(|h| h.action == d.action) {
                s.hotkeys.push(d);
            }
        }
        s.overlay_scale = s.overlay_scale.clamp(0.75, 1.5);
        s
    }

    pub fn save(&self) {
        // Atomic write (temp file + rename) so a crash mid-save can't truncate it.
        if let Ok(t) = serde_json::to_string_pretty(self) {
            let p = Self::path();
            let tmp = p.with_extension("json.tmp");
            if std::fs::write(&tmp, t).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }

    pub fn hotkey_for(&self, action: HotkeyAction) -> Option<&Hotkey> {
        self.hotkeys.iter().find(|h| h.action == action && h.enabled)
    }
}

/// Human readable form of an accelerator ("ctrl+shift+Digit4" -> "Ctrl ⇧ 4").
pub fn pretty_accel(accel: &str) -> String {
    accel
        .split('+')
        .map(|p| match p.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => "Ctrl".to_string(),
            "shift" => "Shift".to_string(),
            "alt" => "Alt".to_string(),
            "super" | "win" | "meta" => "Win".to_string(),
            _ => p
                .trim_start_matches("Digit")
                .trim_start_matches("Key")
                .to_string(),
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_json_loads_with_defaults_and_roundtrips() {
        let s: Settings = serde_json::from_str(r#"{"play_sounds":false,"file_prefix":"X"}"#).unwrap();
        assert!(!s.play_sounds);
        assert_eq!(s.file_prefix, "X");
        assert_eq!(s.window_padding, 48);
        assert_eq!(s.overlay_corner, OverlayCorner::BottomLeft);
        assert_eq!(s.hotkeys.len(), HotkeyAction::ALL.len());

        let mut custom = Settings::default();
        custom.after_capture = AfterCapture::Annotate;
        custom.last_area = Some((1, 2, 3, 4));
        custom.custom_colors = vec![[1, 2, 3, 255]];
        let back: Settings = serde_json::from_str(&serde_json::to_string(&custom).unwrap()).unwrap();
        assert_eq!(back.after_capture, AfterCapture::Annotate);
        assert_eq!(back.last_area, Some((1, 2, 3, 4)));
        assert_eq!(back.custom_colors, vec![[1, 2, 3, 255]]);
    }

    #[test]
    fn default_hotkeys_cover_every_action_with_unique_accelerators() {
        let hk = default_hotkeys();
        for a in HotkeyAction::ALL {
            assert_eq!(hk.iter().filter(|h| h.action == a).count(), 1);
        }
        let mut accels: Vec<_> = hk.iter().map(|h| h.accel.clone()).collect();
        accels.sort();
        accels.dedup();
        assert_eq!(accels.len(), hk.len());
    }

    #[test]
    fn hotkey_for_skips_disabled_entries() {
        let mut s = Settings::default();
        assert!(s.hotkey_for(HotkeyAction::OpenHistory).is_some());
        s.hotkeys.iter_mut().find(|h| h.action == HotkeyAction::OpenHistory).unwrap().enabled = false;
        assert!(s.hotkey_for(HotkeyAction::OpenHistory).is_none());
    }

    #[test]
    fn pretty_accel_formats_modifiers_digits_and_keys() {
        assert_eq!(pretty_accel("ctrl+shift+Digit4"), "Ctrl + Shift + 4");
        assert_eq!(pretty_accel("Alt+KeyA"), "Alt + A");
        assert_eq!(pretty_accel("super+F5"), "Win + F5");
    }
}
