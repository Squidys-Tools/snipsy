//! Win32 screen / window capture (GDI + DWM).
//!
//! All coordinates in this module are *physical* desktop pixels.

mod window;

use anyhow::{anyhow, Result};
use image::RgbaImage;
use windows::core::BOOL;
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, EnumDisplayMonitors,
    GetDC, GetMonitorInfoW, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    CAPTUREBLT, DIB_RGB_COLORS, HDC, HMONITOR, MONITORINFO, SRCCOPY,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};

pub use window::{capture_window, list_windows, root_window_at, round_corners, WindowInfo};

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

pub fn primary_monitor() -> Option<MonitorInfo> {
    let m = monitors();
    m.iter().find(|m| m.primary).or(m.first()).cloned()
}
