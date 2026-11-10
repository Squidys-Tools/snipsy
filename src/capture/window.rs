use super::{MemDc, PRect};
use anyhow::{anyhow, Result};
use image::RgbaImage;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
};
use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetAncestor, GetClassNameW, GetWindowLongW, GetWindowRect, GetWindowTextW,
    GetWindowThreadProcessId, IsHungAppWindow, IsIconic, IsWindowVisible, GA_ROOT, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
    WS_EX_TRANSPARENT,
};

#[derive(Clone, Debug)]
pub struct WindowInfo {
    pub hwnd: isize,
    pub rect: PRect,
    pub title: String,
}

fn is_cloaked(hwnd: HWND) -> bool {
    let mut cloaked: u32 = 0;
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut _ as *mut _,
            std::mem::size_of::<u32>() as u32,
        )
        .is_ok()
            && cloaked != 0
    }
}

pub fn frame_bounds(hwnd: HWND) -> PRect {
    let mut r = RECT::default();
    unsafe {
        if DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut r as *mut _ as *mut _,
            std::mem::size_of::<RECT>() as u32,
        )
        .is_err()
        {
            let _ = GetWindowRect(hwnd, &mut r);
        }
    }
    PRect::from_rect(r)
}

/// Visible top-level windows in z-order (front first), excluding our own process.
pub fn list_windows() -> Vec<WindowInfo> {
    unsafe extern "system" fn cb(hwnd: HWND, data: LPARAM) -> BOOL {
        let list = &mut *(data.0 as *mut Vec<WindowInfo>);
        if !IsWindowVisible(hwnd).as_bool() || IsIconic(hwnd).as_bool() || is_cloaked(hwnd) {
            return true.into();
        }
        let ex = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        if ex & WS_EX_TRANSPARENT.0 != 0 {
            return true.into();
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == std::process::id() {
            return true.into();
        }
        let mut cls = [0u16; 128];
        let n = GetClassNameW(hwnd, &mut cls);
        let class = String::from_utf16_lossy(&cls[..n as usize]);
        if class == "Progman" || class == "WorkerW" || class == "Shell_TrayWnd" || class == "Shell_SecondaryTrayWnd" {
            // desktop / taskbar are not capturable windows
            return true.into();
        }
        let mut buf = [0u16; 256];
        let n = GetWindowTextW(hwnd, &mut buf);
        let title = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
        if title.is_empty() && ex & WS_EX_TOOLWINDOW.0 != 0 {
            return true.into();
        }
        let rect = frame_bounds(hwnd);
        if rect.w < 8 || rect.h < 8 {
            return true.into();
        }
        list.push(WindowInfo { hwnd: hwnd.0 as isize, rect, title });
        true.into()
    }
    let mut list: Vec<WindowInfo> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut list as *mut _ as isize));
    }
    list
}

pub fn root_window_at(x: i32, y: i32, windows: &[WindowInfo]) -> Option<WindowInfo> {
    windows.iter().find(|w| w.rect.contains(x, y)).cloned()
}

#[allow(dead_code)]
pub fn root_of(hwnd: isize) -> isize {
    unsafe { GetAncestor(HWND(hwnd as _), GA_ROOT).0 as isize }
}

/// Capture a single window's content (works even when partially covered).
/// Errors if PrintWindow fails, returns black, or the window is hung; it never
/// copies the live screen (our overlay is still on top), so callers should fall
/// back to a crop of the frozen frame.
pub fn capture_window(info: &WindowInfo) -> Result<RgbaImage> {
    let hwnd = HWND(info.hwnd as _);
    unsafe {
        // PrintWindow sends WM_PRINT to the target's thread with no timeout.
        if IsHungAppWindow(hwnd).as_bool() {
            return Err(anyhow!("target window not responding"));
        }
        let mut wr = RECT::default();
        GetWindowRect(hwnd, &mut wr)?;
        let full = PRect::from_rect(wr);
        let dc = MemDc::new(full.w, full.h)?;
        // PW_RENDERFULLCONTENT = 2
        let ok = PrintWindow(hwnd, dc.mem, PRINT_WINDOW_FLAGS(2)).as_bool();
        if !ok {
            return Err(anyhow!("PrintWindow failed"));
        }
        let img = dc.to_rgba();
        let vis = frame_bounds(hwnd);
        let fx = (vis.x - full.x).max(0) as u32;
        let fy = (vis.y - full.y).max(0) as u32;
        if fx >= img.width() || fy >= img.height() {
            return Err(anyhow!("window frame outside rect"));
        }
        let fw = (vis.w.max(0) as u32).min(img.width() - fx);
        let fh = (vis.h.max(0) as u32).min(img.height() - fy);
        if fw == 0 || fh == 0 {
            return Err(anyhow!("empty window crop"));
        }
        let cropped = image::imageops::crop_imm(&img, fx, fy, fw, fh).to_image();
        // PrintWindow can return black for some GPU-composited apps
        if is_mostly_black(&cropped) {
            return Err(anyhow!("PrintWindow returned black"));
        }
        Ok(cropped)
    }
}

fn is_mostly_black(img: &RgbaImage) -> bool {
    let step = ((img.width() * img.height()) / 2000).max(1) as usize;
    let mut dark = 0;
    let mut total = 0;
    for p in img.pixels().step_by(step) {
        total += 1;
        if p[0] < 3 && p[1] < 3 && p[2] < 3 {
            dark += 1;
        }
    }
    total > 0 && dark * 100 / total > 97
}

/// Round window corners (Windows 11 style) with transparent alpha.
pub fn round_corners(img: &mut RgbaImage, radius: f32) {
    let (w, h) = (img.width() as f32, img.height() as f32);
    let r = radius;
    for y in 0..img.height() {
        for x in 0..img.width() {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let cx = if fx < r { r } else if fx > w - r { w - r } else { continue };
            let cy = if fy < r { r } else if fy > h - r { h - r } else { continue };
            let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            let a = (r - d + 0.5).clamp(0.0, 1.0);
            let p = img.get_pixel_mut(x, y);
            p[3] = (p[3] as f32 * a) as u8;
        }
    }
}
