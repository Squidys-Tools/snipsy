//! Application core: global state, command bus (tray + global hotkeys),
//! and the capture → after-capture pipeline.

use crate::capture::{self, PRect};
use crate::history::{CaptureKind, History, HistoryEntry};
use crate::settings::{AfterCapture, HotkeyAction, Settings};
use crate::{annotate, history_view, overlay, pin, platform, quick, settings_view, toast, ui};
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use gpui::*;
use image::RgbaImage;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    AllInOne,
    Area,
    Window,
    Ocr,
}

#[derive(Clone, Debug)]
pub enum Command {
    Capture(Mode),
    Fullscreen,
    PreviousArea,
    SelfTimer(Option<PRect>),
    OpenHistory,
    RestoreLast,
    ToggleOverlays,
    OpenSettings,
    UnlockPins,
    Quit,
}

impl Command {
    fn from_action(a: HotkeyAction) -> Command {
        match a {
            HotkeyAction::AllInOne => Command::Capture(Mode::AllInOne),
            HotkeyAction::CaptureArea => Command::Capture(Mode::Area),
            HotkeyAction::CaptureFullscreen => Command::Fullscreen,
            HotkeyAction::CaptureWindow => Command::Capture(Mode::Window),
            HotkeyAction::CapturePreviousArea => Command::PreviousArea,
            HotkeyAction::SelfTimer => Command::SelfTimer(None),
            HotkeyAction::CaptureText => Command::Capture(Mode::Ocr),
            HotkeyAction::OpenHistory => Command::OpenHistory,
            HotkeyAction::RestoreLastOverlay => Command::RestoreLast,
            HotkeyAction::ToggleOverlays => Command::ToggleOverlays,
        }
    }
}

/// A finished capture living in memory (Quick Access, editor, pins).
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
        let thumb = crate::history::thumbnail(&image, 560);
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

pub struct AppState {
    pub settings: Settings,
    pub history: History,
    pub capturing: bool,
    pub overlays: Vec<AnyWindowHandle>,
    pub quick: Option<WindowHandle<quick::QuickAccess>>,
    pub recently_closed: Vec<Capture>,
    pub overlays_hidden: bool,
    pub pins: Vec<WindowHandle<pin::PinView>>,
    pub history_window: Option<WindowHandle<history_view::HistoryView>>,
    pub settings_window: Option<WindowHandle<settings_view::SettingsView>>,
    pub tx: flume::Sender<Command>,
    hotkey_manager: Option<GlobalHotKeyManager>,
    hotkey_ids: Arc<std::sync::Mutex<HashMap<u32, HotkeyAction>>>,
    registered: Vec<HotKey>,
    pub hotkey_errors: Vec<String>,
    _tray: Option<TrayIcon>,
    _host: Option<AnyWindowHandle>,
}

impl Global for AppState {}

pub fn state(cx: &App) -> &AppState {
    cx.global::<AppState>()
}

pub fn state_mut(cx: &mut App) -> &mut AppState {
    cx.global_mut::<AppState>()
}

pub fn send(cx: &App, cmd: Command) {
    let _ = state(cx).tx.send(cmd);
}

// ---------------------------------------------------------------- bootstrap

pub fn init(cx: &mut App) {
    let settings = Settings::load();
    let history = History::load(settings.history_days);
    let (tx, rx) = flume::unbounded::<Command>();

    cx.set_global(AppState {
        settings,
        history,
        capturing: false,
        overlays: vec![],
        quick: None,
        recently_closed: vec![],
        overlays_hidden: false,
        pins: vec![],
        history_window: None,
        settings_window: None,
        tx: tx.clone(),
        hotkey_manager: None,
        hotkey_ids: Default::default(),
        registered: vec![],
        hotkey_errors: vec![],
        _tray: None,
        _host: None,
    });

    // GPUI quits when the last window closes; keep an invisible host window
    // alive so Snipsy can live in the tray.
    let host = cx
        .open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::new(point(px(-32000.), px(-32000.)), size(px(1.), px(1.))))),
                titlebar: None,
                focus: false,
                show: false,
                kind: WindowKind::PopUp,
                is_movable: false,
                ..Default::default()
            },
            |_, cx| cx.new(|_| HostView),
        )
        .ok()
        .map(|h| h.into());
    state_mut(cx)._host = host;

    // Tray icon + menu
    let tray = build_tray(tx.clone());
    state_mut(cx)._tray = tray;

    // Global hotkeys
    match GlobalHotKeyManager::new() {
        Ok(m) => state_mut(cx).hotkey_manager = Some(m),
        Err(e) => eprintln!("hotkeys unavailable: {e}"),
    }
    register_hotkeys(cx);
    {
        let tx = tx.clone();
        let ids = state(cx).hotkey_ids.clone();
        GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
            if e.state == HotKeyState::Pressed {
                if let Some(a) = ids.lock().unwrap().get(&e.id).copied() {
                    let _ = tx.send(Command::from_action(a));
                }
            }
        }));
    }

    // Command loop (foreground)
    cx.spawn(async move |cx: &mut AsyncApp| {
        while let Ok(cmd) = rx.recv_async().await {
            let _ = cx.update(|cx| run(cmd, cx));
        }
    })
    .detach();

    // First run hint
    toast::show(
        cx,
        "Snipsy is running",
        format!(
            "{} to capture an area · right-click the tray icon for more",
            state(cx)
                .settings
                .hotkey_for(HotkeyAction::CaptureArea)
                .map(|h| crate::settings::pretty_accel(&h.accel))
                .unwrap_or_default()
        ),
    );
}

struct HostView;
impl Render for HostView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

/// Unregister every global hotkey (e.g. while the settings recorder listens
/// for a new combination). Call `register_hotkeys` to restore them.
pub fn unregister_hotkeys(cx: &mut App) {
    let st = state_mut(cx);
    if let Some(mgr) = st.hotkey_manager.as_ref() {
        let old = std::mem::take(&mut st.registered);
        let _ = mgr.unregister_all(&old);
    }
}

pub fn register_hotkeys(cx: &mut App) {
    let st = state_mut(cx);
    let Some(mgr) = st.hotkey_manager.as_ref() else { return };
    let old = std::mem::take(&mut st.registered);
    let _ = mgr.unregister_all(&old);
    let ids = st.hotkey_ids.clone();
    let mut ids = ids.lock().unwrap();
    ids.clear();
    st.hotkey_errors.clear();
    for hk in st.settings.hotkeys.clone() {
        if !hk.enabled {
            continue;
        }
        match HotKey::from_str(&hk.accel) {
            Ok(k) => match mgr.register(k) {
                Ok(()) => {
                    ids.insert(k.id(), hk.action);
                    st.registered.push(k);
                }
                Err(e) => st.hotkey_errors.push(format!("{}: {e}", hk.action.label())),
            },
            Err(e) => st.hotkey_errors.push(format!("{}: {e}", hk.action.label())),
        }
    }
}

fn tray_icon_image() -> tray_icon::Icon {
    // 32x32 rounded blue square with a white viewfinder
    let n = 32u32;
    let mut px = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let r = 7.0;
            let cx = fx.clamp(r + 1.0, n as f32 - r - 1.0);
            let cy = fy.clamp(r + 1.0, n as f32 - r - 1.0);
            let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            let a = (r - d + 0.5).clamp(0.0, 1.0);
            let i = ((y * n + x) * 4) as usize;
            let t = fy / n as f32;
            let (mut cr, mut cg, mut cb) = (
                (40.0 + 20.0 * t) as u8,
                (150.0 - 50.0 * t) as u8,
                255u8,
            );
            // viewfinder corners
            let (lx, ly) = (x as i32, y as i32);
            let corner = |a: i32, b: i32| (8..=11).contains(&a) && (8..=23).contains(&b) && !(12..=19).contains(&b);
            let inside = corner(lx, ly) || corner(31 - lx, ly) || corner(ly, lx) || corner(31 - ly, lx);
            let dot = ((fx - 16.0).powi(2) + (fy - 16.0).powi(2)).sqrt() < 3.2;
            if inside || dot {
                cr = 255;
                cg = 255;
                cb = 255;
            }
            px[i] = cr;
            px[i + 1] = cg;
            px[i + 2] = cb;
            px[i + 3] = (a * 255.0) as u8;
        }
    }
    tray_icon::Icon::from_rgba(px, n, n).unwrap()
}

fn build_tray(tx: flume::Sender<Command>) -> Option<TrayIcon> {
    let menu = Menu::new();
    let mut map: HashMap<MenuId, Command> = HashMap::new();
    let mut add = |label: &str, cmd: Command| {
        let item = MenuItem::new(label, true, None);
        map.insert(item.id().clone(), cmd);
        let _ = menu.append(&item);
    };
    add("All-In-One", Command::Capture(Mode::AllInOne));
    add("Capture Area", Command::Capture(Mode::Area));
    add("Capture Previous Area", Command::PreviousArea);
    add("Capture Fullscreen", Command::Fullscreen);
    add("Capture Window", Command::Capture(Mode::Window));
    add("Self-Timer", Command::SelfTimer(None));
    add("Capture Text (OCR)", Command::Capture(Mode::Ocr));
    let _ = menu.append(&PredefinedMenuItem::separator());
    add("Open Capture History", Command::OpenHistory);
    add("Restore Recently Closed", Command::RestoreLast);
    add("Hide / Show Overlays", Command::ToggleOverlays);
    add("Unlock Pinned Screenshots", Command::UnlockPins);
    let _ = menu.append(&PredefinedMenuItem::separator());
    add("Settings…", Command::OpenSettings);
    add("Quit Snipsy", Command::Quit);

    let tray = TrayIconBuilder::new()
        .with_tooltip("Snipsy")
        .with_icon(tray_icon_image())
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .map_err(|e| eprintln!("tray: {e}"))
        .ok()?;

    let mtx = tx.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        if let Some(c) = map.get(&e.id) {
            let _ = mtx.send(c.clone());
        }
    }));
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: tray_icon::MouseButtonState::Up, .. } = e {
            let _ = tx.send(Command::Capture(Mode::AllInOne));
        }
    }));
    Some(tray)
}

// ---------------------------------------------------------------- commands

pub fn run(cmd: Command, cx: &mut App) {
    match cmd {
        Command::Capture(mode) => overlay::start(mode, cx),
        Command::Fullscreen => {
            if state(cx).capturing {
                return;
            }
            let (x, y) = capture::cursor_pos();
            if let Some(m) = capture::monitor_at(x, y) {
                capture_rect_now(m.rect, CaptureKind::Fullscreen, cx);
            }
        }
        Command::PreviousArea => {
            if state(cx).capturing {
                return;
            }
            match state(cx).settings.last_area {
                Some((x, y, w, h)) => capture_rect_now(PRect { x, y, w, h }, CaptureKind::Area, cx),
                None => overlay::start(Mode::Area, cx),
            }
        }
        Command::SelfTimer(area) => toast::start_timer(area, cx),
        Command::OpenHistory => history_view::open(cx),
        Command::RestoreLast => {
            if let Some(c) = state_mut(cx).recently_closed.pop() {
                quick::push(c, cx);
            }
        }
        Command::ToggleOverlays => quick::toggle_hidden(cx),
        Command::OpenSettings => settings_view::open(cx),
        Command::UnlockPins => pin::unlock_all(cx),
        Command::Quit => cx.quit(),
    }
}

/// Capture a desktop rect without UI (fullscreen, previous area, timer).
pub fn capture_rect_now(r: PRect, kind: CaptureKind, cx: &mut App) {
    match capture::capture_rect(r) {
        Ok(img) => finish(img, kind, cx),
        Err(e) => toast::show(cx, "Capture failed", e.to_string()),
    }
}

/// After-capture pipeline: sound, history (background), then the configured action.
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
                history_view::refresh(cx);
            });
        }
    })
    .detach();
}

pub fn add_history_entry(entry: HistoryEntry, cx: &mut App) {
    state_mut(cx).history.push(entry);
    history_view::refresh(cx);
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

/// Run OCR on a background thread and copy the result.
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


pub fn delay(cx: &App, ms: u64) -> Task<()> {
    cx.background_executor().timer(Duration::from_millis(ms))
}
