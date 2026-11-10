use crate::capture::PRect;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::*;

pub fn hwnd_of(window: &gpui::Window) -> Option<HWND> {
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as _)),
        _ => None,
    }
}

pub fn place_window(hwnd: HWND, r: PRect, topmost: bool) {
    place_window_ex(hwnd, r, topmost, true);
}

/// Like `place_window`, but only shows the window when `show` is true (a
/// hidden window stays hidden).
pub fn place_window_ex(hwnd: HWND, r: PRect, topmost: bool, show: bool) {
    unsafe {
        let after = if topmost { HWND_TOPMOST } else { HWND_NOTOPMOST };
        let flags = if show { SWP_NOACTIVATE | SWP_SHOWWINDOW } else { SWP_NOACTIVATE };
        let _ = SetWindowPos(hwnd, Some(after), r.x, r.y, r.w, r.h, flags);
    }
}

#[allow(dead_code)]
pub fn set_topmost(hwnd: HWND, topmost: bool) {
    unsafe {
        let after = if topmost { HWND_TOPMOST } else { HWND_NOTOPMOST };
        let _ = SetWindowPos(hwnd, Some(after), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
    }
}

pub fn move_window_by(hwnd: HWND, dx: i32, dy: i32) {
    unsafe {
        let mut r = windows::Win32::Foundation::RECT::default();
        if GetWindowRect(hwnd, &mut r).is_ok() {
            let _ = SetWindowPos(
                hwnd,
                None,
                r.left + dx,
                r.top + dy,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

/// Make the window re-announce its current size (two real size changes, since an unchanged size sends nothing).
pub fn resync_size(hwnd: HWND) {
    let r = window_rect(hwnd);
    let flags = SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE;
    unsafe {
        let _ = SetWindowPos(hwnd, None, 0, 0, r.w, r.h + 1, flags);
        let _ = SetWindowPos(hwnd, None, 0, 0, r.w, r.h, flags);
    }
}

pub fn window_rect(hwnd: HWND) -> PRect {
    let mut r = windows::Win32::Foundation::RECT::default();
    unsafe {
        let _ = GetWindowRect(hwnd, &mut r);
    }
    PRect::from_rect(r)
}

pub fn set_click_through(hwnd: HWND, enabled: bool) {
    unsafe {
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        let flags = WS_EX_TRANSPARENT.0 | WS_EX_LAYERED.0;
        let nex = if enabled { ex | flags } else { ex & !WS_EX_TRANSPARENT.0 };
        SetWindowLongW(hwnd, GWL_EXSTYLE, nex as i32);
        if enabled {
            let _ = SetLayeredWindowAttributes(hwnd, windows::Win32::Foundation::COLORREF(0), 255, LWA_ALPHA);
        }
    }
}

pub fn set_tool_window(hwnd: HWND) {
    unsafe {
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        SetWindowLongW(hwnd, GWL_EXSTYLE, ((ex | WS_EX_TOOLWINDOW.0) & !WS_EX_APPWINDOW.0) as i32);
    }
}

pub fn force_foreground(hwnd: HWND) {
    use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, SetFocus, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_MENU,
    };
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        if GetForegroundWindow() == hwnd {
            let _ = SetFocus(Some(hwnd));
            return;
        }
        // Windows refuses SetForegroundWindow from a process that did not receive
        // the last input event. A synthetic Alt tap counts as input and lifts the
        // foreground lock; attaching to the foreground thread's input queue
        // lets SetFocus work across threads.
        let key = |flags| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki: KEYBDINPUT { wVk: VK_MENU, dwFlags: flags, ..Default::default() } },
        };
        let _ = SendInput(&[key(Default::default()), key(KEYEVENTF_KEYUP)], std::mem::size_of::<INPUT>() as i32);
        let fg = GetForegroundWindow();
        let fg_thread = GetWindowThreadProcessId(fg, None);
        let me = GetCurrentThreadId();
        let attached = fg_thread != 0 && fg_thread != me && AttachThreadInput(me, fg_thread, true).as_bool();
        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(hwnd));
        if attached {
            let _ = AttachThreadInput(me, fg_thread, false);
        }
    }
}

pub fn set_no_activate(hwnd: HWND) {
    unsafe {
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        SetWindowLongW(hwnd, GWL_EXSTYLE, (ex | WS_EX_NOACTIVATE.0 | WS_EX_TOOLWINDOW.0) as i32);
    }
}

/// Remove the Windows 11 1px DWM border and corner rounding from a transparent
/// HUD window (toasts, Quick Access, pins). No-op on Windows 10.
pub fn set_hud_frame(hwnd: HWND) {
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE, DWMWA_WINDOW_CORNER_PREFERENCE,
        DWMWCP_DONOTROUND,
    };
    unsafe {
        let none: u32 = DWMWA_COLOR_NONE;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &none as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
        let corner = DWMWCP_DONOTROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as *const _,
            std::mem::size_of_val(&corner) as u32,
        );
    }
}

pub fn hide_window(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}

pub fn show_no_activate(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
    }
}
