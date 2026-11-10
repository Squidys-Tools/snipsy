mod hotkeys;
mod pipeline;
mod tray;

use crate::capture::{self, PRect};
use crate::history::view as history_view;
use crate::history::{CaptureKind, History};
use crate::settings::view as settings_view;
use crate::settings::{HotkeyAction, Settings};
use crate::{overlay, pin, quick, toast};
use global_hotkey::hotkey::HotKey;
use global_hotkey::GlobalHotKeyManager;
use gpui::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tray_icon::TrayIcon;

pub use hotkeys::{register_hotkeys, unregister_hotkeys};
pub use pipeline::{
    add_history_entry, capture_rect_now, copy_capture, finish, next_capture_id, recognize_and_copy, save_to_export,
    temp_file_for, temp_root, Capture,
};

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

    let tray = tray::build_tray(tx);
    state_mut(cx)._tray = tray;

    hotkeys::init_hotkeys(cx);

    cx.spawn(async move |cx: &mut AsyncApp| {
        while let Ok(cmd) = rx.recv_async().await {
            let _ = cx.update(|cx| run(cmd, cx));
        }
    })
    .detach();

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

pub fn delay(cx: &App, ms: u64) -> Task<()> {
    cx.background_executor().timer(Duration::from_millis(ms))
}
