pub mod color {
    use gpui::{rgb, rgba, Rgba};
    pub fn window_bg() -> Rgba {
        rgb(0x1E1E1E)
    }
    pub fn sidebar_bg() -> Rgba {
        rgb(0x262626)
    }
    pub fn toolbar_bg() -> Rgba {
        rgb(0x2A2A2A)
    }
    pub fn canvas_bg() -> Rgba {
        rgb(0x161616)
    }
    pub fn group_bg() -> Rgba {
        rgba(0xFFFFFF0A)
    }
    pub fn hud() -> Rgba {
        rgba(0x262628E6)
    }
    pub fn hud_border() -> Rgba {
        rgba(0xFFFFFF1F)
    }
    pub fn border() -> Rgba {
        rgba(0xFFFFFF14)
    }
    pub fn separator() -> Rgba {
        rgba(0xFFFFFF14)
    }
    pub fn control() -> Rgba {
        rgba(0xFFFFFF1A)
    }
    pub fn control_hover() -> Rgba {
        rgba(0xFFFFFF26)
    }
    pub fn control_pressed() -> Rgba {
        rgba(0xFFFFFF33)
    }
    pub fn hover() -> Rgba {
        rgba(0xFFFFFF14)
    }
    pub fn selected() -> Rgba {
        rgba(0xFFFFFF24)
    }
    pub fn field() -> Rgba {
        rgba(0xFFFFFF14)
    }
    pub fn text() -> Rgba {
        rgba(0xFFFFFFE6)
    }
    pub fn text_dim() -> Rgba {
        rgba(0xFFFFFF99)
    }
    pub fn text_faint() -> Rgba {
        rgba(0xFFFFFF59)
    }
    pub fn accent() -> Rgba {
        rgb(0x0A84FF)
    }
    pub fn accent_hover() -> Rgba {
        rgb(0x409CFF)
    }
    pub fn on_accent() -> Rgba {
        rgb(0xFFFFFF)
    }
    pub fn danger() -> Rgba {
        rgb(0xFF453A)
    }
    pub fn close_hover() -> Rgba {
        rgb(0xC42B1C)
    }
    pub fn scrim() -> Rgba {
        rgba(0x00000073)
    }
    pub fn light_pill() -> Rgba {
        rgba(0xF2F2F7E6)
    }
    pub fn light_pill_text() -> Rgba {
        rgb(0x1C1C1E)
    }
}

pub mod radius {
    use gpui::{px, Pixels};
    pub const SM: Pixels = px(6.);
    pub const MD: Pixels = px(8.);
    pub const LG: Pixels = px(12.);
    pub const XL: Pixels = px(18.);
}

pub mod font_size {
    use gpui::{px, Pixels};
    pub const CAPTION: Pixels = px(11.);
    pub const SMALL: Pixels = px(12.);
    pub const BODY: Pixels = px(13.);
    pub const TITLE: Pixels = px(15.);
    pub const DISPLAY: Pixels = px(20.);
}

pub const FONT: &str = "Inter";
