//! Win32 screen / window capture (GDI + DWM).
//!
//! All coordinates in this module are *physical* desktop pixels.

use anyhow::{anyhow, Result};
use image::RgbaImage;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Dwm::{
    DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, EnumDisplayMonitors,
    GetDC, GetMonitorInfoW, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    CAPTUREBLT, DIB_RGB_COLORS, HDC, HMONITOR, MONITORINFO, SRCCOPY,
};
use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetAncestor, GetClassNameW, GetWindowLongW, GetWindowRect, GetWindowTextW,
    GetWindowThreadProcessId, IsHungAppWindow, IsIconic, IsWindowVisible, GA_ROOT, GWL_EXSTYLE, WS_EX_TOOLWINDOW,
    WS_EX_TRANSPARENT,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl PRect {
    pub fn from_rect(r: RECT) -> Self {
        Self { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top }
    }
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }
    pub fn intersect(&self, o: &PRect) -> Option<PRect> {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = (self.x + self.w).min(o.x + o.w);
        let y1 = (self.y + self.h).min(o.y + o.h);
        (x1 > x0 && y1 > y0).then(|| PRect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 })
    }
}

#[derive(Clone, Debug)]
pub struct MonitorInfo {
    pub rect: PRect,
    /// work area (excludes the taskbar)
    pub work: PRect,
    pub scale: f32,
    pub primary: bool,
}

pub fn monitors() -> Vec<MonitorInfo> {
    unsafe extern "system" fn cb(hmon: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        let list = &mut *(data.0 as *mut Vec<MonitorInfo>);
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetMonitorInfoW(hmon, &mut info).as_bool() {
            let (mut dx, mut dy) = (96u32, 96u32);
            let _ = GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
            list.push(MonitorInfo {
                rect: PRect::from_rect(info.rcMonitor),
                work: PRect::from_rect(info.rcWork),
                scale: dx as f32 / 96.0,
                primary: info.dwFlags & 1 != 0,
            });
        }
        true.into()
    }
    let mut list: Vec<MonitorInfo> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(cb), LPARAM(&mut list as *mut _ as isize));
    }
    list
}

/// Monitor containing the given physical point (falls back to primary).
pub fn monitor_at(x: i32, y: i32) -> Option<MonitorInfo> {
    let mons = monitors();
    mons.iter()
        .find(|m| m.rect.contains(x, y))
        .or_else(|| mons.iter().find(|m| m.primary))
        .or(mons.first())
        .cloned()
}

pub fn cursor_pos() -> (i32, i32) {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
    let mut p = POINT::default();
    unsafe {
        let _ = GetCursorPos(&mut p);
    }
    (p.x, p.y)
}

struct MemDc {
    screen: HDC,
    mem: HDC,
    bmp: windows::Win32::Graphics::Gdi::HBITMAP,
    old: windows::Win32::Graphics::Gdi::HGDIOBJ,
    bits: *mut u8,
    w: i32,
    h: i32,
}

impl MemDc {
    unsafe fn new(w: i32, h: i32) -> Result<Self> {
        if w <= 0 || h <= 0 {
            return Err(anyhow!("empty bitmap"));
        }
        let screen = GetDC(None);
        let mem = CreateCompatibleDC(Some(screen));
        let bi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h, // top-down
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let bmp = match CreateDIBSection(Some(mem), &bi, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(b) => b,
            Err(e) => {
                let _ = DeleteDC(mem);
                ReleaseDC(None, screen);
                return Err(e.into());
            }
        };
        let old = SelectObject(mem, bmp.into());
        Ok(Self { screen, mem, bmp, old, bits: bits as *mut u8, w, h })
    }

    /// Copy out as straight RGBA with opaque alpha.
    unsafe fn to_rgba(&self) -> RgbaImage {
        let len = (self.w * self.h * 4) as usize;
        let src = std::slice::from_raw_parts(self.bits, len);
        let mut out = vec![0u8; len];
        for (d, s) in out.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = 255;
        }
        RgbaImage::from_raw(self.w as u32, self.h as u32, out).unwrap()
    }
}

impl Drop for MemDc {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.mem, self.old);
            let _ = DeleteObject(self.bmp.into());
            let _ = DeleteDC(self.mem);
            ReleaseDC(None, self.screen);
        }
    }
}

/// Capture a rectangle of the virtual desktop.
pub fn capture_rect(r: PRect) -> Result<RgbaImage> {
    if r.w <= 0 || r.h <= 0 {
        return Err(anyhow!("empty capture rect"));
    }
    unsafe {
        let dc = MemDc::new(r.w, r.h)?;
        BitBlt(dc.mem, 0, 0, r.w, r.h, Some(dc.screen), r.x, r.y, SRCCOPY | CAPTUREBLT)?;
        Ok(dc.to_rgba())
    }
}

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
        // crop to the current visible (DWM) frame
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

pub fn primary_monitor() -> Option<MonitorInfo> {
    let m = monitors();
    m.iter().find(|m| m.primary).or(m.first()).cloned()
}
