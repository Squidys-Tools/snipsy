pub mod color {
    use gpui::{rgb, rgba, Rgba};
    pub fn accent() -> Rgba {
        rgb(0x0A84FF)
    }
    pub fn accent_hover() -> Rgba {
        rgb(0x3395FF)
    }
    /// Translucent HUD panel (dark vibrancy approximation).
    pub fn hud() -> Rgba {
        rgba(0x1E1E20E6)
    }
    pub fn hud_border() -> Rgba {
        rgba(0xFFFFFF1F)
    }
    pub fn hover() -> Rgba {
        rgba(0xFFFFFF1A)
    }
    pub fn selected() -> Rgba {
        rgba(0xFFFFFF29)
    }
    pub fn text() -> Rgba {
        rgb(0xF5F5F7)
    }
    pub fn text_dim() -> Rgba {
        rgba(0xEBEBF599)
    }
    pub fn text_faint() -> Rgba {
        rgba(0xEBEBF54D)
    }
    pub fn window_bg() -> Rgba {
        rgb(0x1C1C1E)
    }
    pub fn sidebar_bg() -> Rgba {
        rgb(0x242426)
    }
    pub fn group_bg() -> Rgba {
        rgb(0x2C2C2E)
    }
    pub fn separator() -> Rgba {
        rgba(0xFFFFFF14)
    }
    pub fn canvas_bg() -> Rgba {
        rgb(0x151517)
    }
    pub fn danger() -> Rgba {
        rgb(0xFF453A)
    }
    pub fn on_accent() -> Rgba {
        rgb(0xFFFFFF)
    }
    pub fn control() -> Rgba {
        rgba(0xFFFFFF1F)
    }
    pub fn control_hover() -> Rgba {
        rgba(0xFFFFFF2E)
    }
    pub fn field() -> Rgba {
        rgba(0xFFFFFF14)
    }
    pub fn raised() -> Rgba {
        rgb(0x3A3A3C)
    }
}

pub const FONT: &str = "Segoe UI";
