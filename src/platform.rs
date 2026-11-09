//! Thin Win32 / WinRT helpers that GPUI does not expose.

use anyhow::{anyhow, Result};
use image::RgbaImage;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::path::Path;
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::capture::PRect;

pub fn hwnd_of(window: &gpui::Window) -> Option<HWND> {
    match HasWindowHandle::window_handle(window).ok()?.as_raw() {
        RawWindowHandle::Win32(h) => Some(HWND(h.hwnd.get() as _)),
        _ => None,
    }
}

/// Force a window to an exact physical rect, optionally always-on-top.
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

pub fn window_rect(hwnd: HWND) -> PRect {
    let mut r = windows::Win32::Foundation::RECT::default();
    unsafe {
        let _ = GetWindowRect(hwnd, &mut r);
    }
    PRect::from_rect(r)
}

/// Make a window ignore mouse input (clicks pass through to whatever is below).
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

/// Hide from Alt-Tab / taskbar.
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

// ---------------------------------------------------------------- clipboard

pub fn copy_image(img: &RgbaImage) -> Result<()> {
    let mut cb = arboard::Clipboard::new()?;
    cb.set_image(arboard::ImageData {
        width: img.width() as usize,
        height: img.height() as usize,
        bytes: std::borrow::Cow::Borrowed(img.as_raw()),
    })?;
    Ok(())
}

pub fn copy_text(text: &str) -> Result<()> {
    let mut cb = arboard::Clipboard::new()?;
    cb.set_text(text.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------- shell

pub fn reveal_in_explorer(path: &Path) {
    let _ = std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", path.display()))
        .spawn();
}

pub fn open_path(path: &Path) {
    unsafe {
        let p = HSTRING::from(path.as_os_str());
        windows::Win32::UI::Shell::ShellExecuteW(
            None,
            &HSTRING::from("open"),
            &p,
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

/// Start an OLE drag of a file (so it can be dropped into any app).
/// Blocks inside a modal drag loop until the drop completes.
pub fn drag_file_out(hwnd: HWND, path: &Path) -> Result<()> {
    use windows::Win32::System::Com::IDataObject;
    use windows::Win32::System::Ole::{DROPEFFECT_COPY, DROPEFFECT_MOVE};
    use windows::Win32::UI::Shell::{
        BHID_DataObject, IShellItem, SHCreateItemFromParsingName, SHDoDragDrop,
    };
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(path.as_os_str()), None)?;
        let data: IDataObject = item.BindToHandler(None, &BHID_DataObject)?;
        let _ = SHDoDragDrop(Some(hwnd), &data, None, DROPEFFECT_COPY | DROPEFFECT_MOVE);
    }
    Ok(())
}

// ---------------------------------------------------------------- sound

static SHUTTER: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();

/// Synthesised camera-shutter click (two short filtered noise bursts).
fn shutter_wav() -> &'static [u8] {
    SHUTTER.get_or_init(|| {
        let rate = 44100u32;
        let n = (rate as f32 * 0.16) as usize;
        let mut seed = 0x1234_5678u32;
        let mut samples = Vec::with_capacity(n);
        let mut lp = 0.0f32;
        for i in 0..n {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let noise = (seed as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let t = i as f32 / rate as f32;
            let env1 = if t < 0.05 { (-t * 90.0).exp() } else { 0.0 };
            let t2 = t - 0.07;
            let env2 = if t2 > 0.0 { (-t2 * 70.0).exp() * 0.8 } else { 0.0 };
            lp += (noise - lp) * 0.35;
            samples.push(((lp * (env1 + env2)) * 0.6 * i16::MAX as f32) as i16);
        }
        let data_len = (samples.len() * 2) as u32;
        let mut w = Vec::with_capacity(44 + data_len as usize);
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36 + data_len).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes());
        w.extend_from_slice(&rate.to_le_bytes());
        w.extend_from_slice(&(rate * 2).to_le_bytes());
        w.extend_from_slice(&2u16.to_le_bytes());
        w.extend_from_slice(&16u16.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&data_len.to_le_bytes());
        for s in samples {
            w.extend_from_slice(&s.to_le_bytes());
        }
        w
    })
}

pub fn play_shutter() {
    use windows::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_MEMORY, SND_NODEFAULT};
    let wav = shutter_wav();
    unsafe {
        let _ = PlaySoundW(
            PCWSTR(wav.as_ptr() as *const u16),
            None,
            SND_MEMORY | SND_ASYNC | SND_NODEFAULT,
        );
    }
}

// ---------------------------------------------------------------- OCR

/// Recognise text using the built-in Windows OCR engine (on-device).
/// Must be called off the UI thread; blocks until done.
pub fn recognize_text(img: &RgbaImage) -> Result<String> {
    use windows::Graphics::Imaging::{BitmapAlphaMode, BitmapPixelFormat, SoftwareBitmap};
    use windows::Media::Ocr::OcrEngine;
    use windows::Storage::Streams::DataWriter;
    use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

    unsafe {
        let _ = RoInitialize(RO_INIT_MULTITHREADED);
    }
    // Upscale small captures: the engine is much more accurate on larger glyphs.
    let max_dim = OcrEngine::MaxImageDimension().unwrap_or(10000);
    let mut work = img.clone();
    if work.height() < 400 && work.width() * 3 < max_dim && work.height() * 3 < max_dim {
        work = image::imageops::resize(
            &work,
            work.width() * 3,
            work.height() * 3,
            image::imageops::FilterType::CatmullRom,
        );
    } else if work.height() < 900 && work.width() * 2 < max_dim && work.height() * 2 < max_dim {
        work = image::imageops::resize(
            &work,
            work.width() * 2,
            work.height() * 2,
            image::imageops::FilterType::CatmullRom,
        );
    }
    if work.width() > max_dim || work.height() > max_dim {
        let s = max_dim as f32 / work.width().max(work.height()) as f32;
        work = image::imageops::resize(
            &work,
            (work.width() as f32 * s) as u32,
            (work.height() as f32 * s) as u32,
            image::imageops::FilterType::Triangle,
        );
    }
    let (ww, wh) = (work.width() as i32, work.height() as i32);
    let mut bgra = work.into_raw();
    for p in bgra.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    let writer = DataWriter::new()?;
    writer.WriteBytes(&bgra)?;
    let buffer = writer.DetachBuffer()?;
    let bitmap = SoftwareBitmap::CreateCopyWithAlphaFromBuffer(
        &buffer,
        BitmapPixelFormat::Bgra8,
        ww,
        wh,
        BitmapAlphaMode::Premultiplied,
    )?;
    let engine = OcrEngine::TryCreateFromUserProfileLanguages()
        .map_err(|e| anyhow!("No OCR language pack installed: {e}"))?;
    let result = engine.RecognizeAsync(&bitmap)?.get()?;
    let mut out = String::new();
    for line in result.Lines()? {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&line.Text()?.to_string_lossy());
    }
    Ok(out)
}

/// Clicking the window will not activate it (no focus stealing).
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
