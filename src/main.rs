//! Snipsy — a CleanShot X-style screenshot toolkit for Windows, built with Rust + GPUI.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod annotate;
mod app;
mod capture;
mod history;
mod history_view;
mod overlay;
mod pin;
mod platform;
mod quick;
mod settings;
mod settings_view;
mod toast;
mod ui;

use gpui::*;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Single instance: a second launch forwards its arguments to the first
    // (no arguments = All-In-One) and exits.
    let _guard = match single_instance() {
        Some(g) => g,
        None => {
            ipc::forward(&args);
            return;
        }
    };
    let remote = ipc::listen();
    // Drag & drop temp files from earlier sessions are no longer in use.
    let _ = std::fs::remove_dir_all(app::temp_root());
    Application::new().with_assets(ui::Assets).run(move |cx: &mut App| {
        app::init(cx);
        handle_args(&args, cx);
        // Arguments forwarded from later launches.
        cx.spawn(async move |cx: &mut AsyncApp| {
            while let Ok(args) = remote.recv_async().await {
                let _ = cx.update(|cx| {
                    if args.is_empty() {
                        app::send(cx, app::Command::Capture(app::Mode::AllInOne));
                    } else {
                        handle_args(&args, cx);
                    }
                });
            }
        })
        .detach();
    });
}

/// `snipsy --capture area|full|window|text|aio|history|settings|timer` and
/// `snipsy --open <file> [--pin|--quick]`, for scripting / testing.
fn handle_args(args: &[String], cx: &mut App) {
    if let Some(i) = args.iter().position(|a| a == "--capture") {
        let cmd = match args.get(i + 1).map(|s| s.as_str()) {
            Some("full") => Some(app::Command::Fullscreen),
            Some("window") => Some(app::Command::Capture(app::Mode::Window)),
            Some("text") => Some(app::Command::Capture(app::Mode::Ocr)),
            Some("aio") => Some(app::Command::Capture(app::Mode::AllInOne)),
            Some("history") => Some(app::Command::OpenHistory),
            Some("settings") => Some(app::Command::OpenSettings),
            Some("timer") => Some(app::Command::SelfTimer(None)),
            _ => Some(app::Command::Capture(app::Mode::Area)),
        };
        if let Some(c) = cmd {
            app::send(cx, c);
        }
    }
    if let Some(i) = args.iter().position(|a| a == "--open") {
        if let Some(path) = args.get(i + 1) {
            if let Ok(img) = image::open(path) {
                let c = app::Capture::new(std::sync::Arc::new(img.to_rgba8()), history::CaptureKind::Area);
                if args.iter().any(|a| a == "--pin") {
                    pin::open(c, cx);
                } else if args.iter().any(|a| a == "--quick") {
                    quick::push(c, cx);
                } else {
                    annotate::open(c, cx);
                }
            }
        }
    }
}

struct InstanceGuard(windows::Win32::Foundation::HANDLE);
impl Drop for InstanceGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

fn single_instance() -> Option<InstanceGuard> {
    use windows::core::w;
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;
    unsafe {
        let h = CreateMutexW(None, true, w!("Local\\SnipsySingleInstance")).ok()?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = windows::Win32::Foundation::CloseHandle(h);
            return None;
        }
        Some(InstanceGuard(h))
    }
}

/// Forwards argv from a second launch to the running instance: WM_COPYDATA to a
/// hidden message-only window owned by a background thread in the first instance.
mod ipc {
    use std::sync::OnceLock;
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::DataExchange::COPYDATASTRUCT;
    use windows::Win32::UI::WindowsAndMessaging::*;

    const CLASS: PCWSTR = w!("SnipsyIpcWindow-6f1c2a52-8d3e-4b7a-9c41-2e5d7b90a3f4");
    /// Tag in COPYDATASTRUCT::dwData ("SNIP").
    const MAGIC: usize = 0x534E_4950;

    static TX: OnceLock<flume::Sender<Vec<String>>> = OnceLock::new();

    /// First instance: start the listener thread. Forwarded argv arrives on the receiver.
    pub fn listen() -> flume::Receiver<Vec<String>> {
        let (tx, rx) = flume::unbounded();
        let _ = TX.set(tx);
        let _ = std::thread::Builder::new().name("snipsy-ipc".into()).spawn(|| unsafe {
            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(wndproc),
                lpszClassName: CLASS,
                ..Default::default()
            };
            if RegisterClassExW(&wc) == 0 {
                eprintln!("ipc: RegisterClassExW failed");
                return;
            }
            let hwnd = match CreateWindowExW(
                Default::default(),
                CLASS,
                w!("Snipsy IPC"),
                Default::default(),
                0,
                0,
                0,
                0,
                Some(HWND_MESSAGE),
                None,
                None,
                None,
            ) {
                Ok(h) => h,
                Err(e) => {
                    eprintln!("ipc: CreateWindowExW failed: {e}");
                    return;
                }
            };
            // Accept WM_COPYDATA from a launch at a lower integrity level too.
            let _ = ChangeWindowMessageFilterEx(hwnd, WM_COPYDATA, MSGFLT_ALLOW, None);
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        });
        rx
    }

    unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
        if msg == WM_COPYDATA && lp.0 != 0 {
            let cds = &*(lp.0 as *const COPYDATASTRUCT);
            if cds.dwData == MAGIC {
                let bytes = if cds.cbData == 0 || cds.lpData.is_null() {
                    &[][..]
                } else {
                    std::slice::from_raw_parts(cds.lpData as *const u8, cds.cbData as usize)
                };
                let text = String::from_utf8_lossy(bytes);
                let args: Vec<String> =
                    if text.is_empty() { vec![] } else { text.split('\0').map(str::to_string).collect() };
                if let Some(tx) = TX.get() {
                    let _ = tx.send(args);
                }
                return LRESULT(1);
            }
        }
        DefWindowProcW(hwnd, msg, wp, lp)
    }

    /// Second instance: send argv to the running instance, retrying briefly in
    /// case it is still starting up.
    pub fn forward(args: &[String]) {
        // Resolve a relative --open path against *this* process's working directory.
        let mut args = args.to_vec();
        if let Some(i) = args.iter().position(|a| a == "--open") {
            if let Some(p) = args.get_mut(i + 1) {
                if let Ok(abs) = std::path::absolute(&*p) {
                    *p = abs.to_string_lossy().into_owned();
                }
            }
        }
        let payload = args.join("\0").into_bytes();
        unsafe {
            for _ in 0..50 {
                if let Ok(hwnd) = FindWindowExW(Some(HWND_MESSAGE), None, CLASS, PCWSTR::null()) {
                    // Let the first instance bring its windows to the front.
                    let mut pid = 0u32;
                    GetWindowThreadProcessId(hwnd, Some(&mut pid));
                    if pid != 0 {
                        let _ = AllowSetForegroundWindow(pid);
                    }
                    let cds = COPYDATASTRUCT {
                        dwData: MAGIC,
                        cbData: payload.len() as u32,
                        lpData: payload.as_ptr() as *mut _,
                    };
                    SendMessageTimeoutW(
                        hwnd,
                        WM_COPYDATA,
                        WPARAM(0),
                        LPARAM(&cds as *const _ as isize),
                        SMTO_ABORTIFHUNG | SMTO_BLOCK,
                        5000,
                        None,
                    );
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        eprintln!("Snipsy is already running but did not respond");
    }
}
