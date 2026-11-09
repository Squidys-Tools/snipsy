//! Shared visual language: colors, assets and small reusable elements that
//! mimic CleanShot X's dark HUD look (translucent graphite, 10-12px radii,
//! system-blue accent, white line icons).

use gpui::prelude::*;
use gpui::*;
use std::borrow::Cow;

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
}

pub const FONT: &str = "Segoe UI";

pub struct Assets;

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        fn icon_bytes(path: &str) -> Option<&'static [u8]> {
            match path {
                $(concat!("icons/", $name, ".svg") => Some(include_bytes!(concat!("../assets/icons/", $name, ".svg"))),)*
                _ => None,
            }
        }
        const ICON_LIST: &[&str] = &[$(concat!("icons/", $name, ".svg")),*];
    };
}

icons!(
    "area", "fullscreen", "window", "timer", "ocr", "history", "pin", "copy", "save", "edit", "close",
    "cursor", "crop", "arrow", "rect", "rect-filled", "ellipse", "line", "pencil", "highlighter", "text",
    "counter", "blur", "pixelate", "spotlight", "undo", "redo", "trash", "background", "drag", "settings",
    "check", "lock", "unlock", "folder", "restore", "keyboard", "image", "sparkles", "eye", "minus",
    "maximize", "zoom-in", "zoom-out", "chevron-down", "more",
);

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(icon_bytes(path).map(Cow::Borrowed))
    }
    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(ICON_LIST.iter().filter(|p| p.starts_with(path)).map(|p| SharedString::from(*p)).collect())
    }
}

pub fn icon(path: &'static str, size: Pixels, color: impl Into<Hsla>) -> Svg {
    svg().path(path).size(size).flex_none().text_color(color.into())
}

/// Square icon button used in HUD toolbars.
pub fn icon_button(id: impl Into<ElementId>, path: &'static str, active: bool) -> Stateful<Div> {
    div()
        .id(id)
        .size(px(32.))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .rounded(px(7.))
        .cursor_pointer()
        .when(active, |d| d.bg(color::selected()))
        .when(!active, |d| d.hover(|s| s.bg(color::hover())))
        .child(icon(path, px(18.), if active { color::text() } else { rgba(0xEBEBF5D0) }))
}

/// CleanShot style translucent pill button ("Copy" / "Save" on overlay cards).
pub fn pill_button(id: impl Into<ElementId>, label: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(14.))
        .h(px(26.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(rgba(0xF2F2F7E6))
        .text_color(rgb(0x1C1C1E))
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(|s| s.bg(rgb(0xFFFFFF)))
        .child(label)
}

/// Standard text button.
pub fn button(id: impl Into<ElementId>, label: impl Into<SharedString>, primary: bool) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(12.))
        .h(px(28.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(7.))
        .text_size(px(13.))
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .when(primary, |d| d.bg(color::accent()).text_color(rgb(0xFFFFFF)).hover(|s| s.bg(color::accent_hover())))
        .when(!primary, |d| {
            d.bg(rgba(0xFFFFFF1F)).text_color(color::text()).hover(|s| s.bg(rgba(0xFFFFFF2E)))
        })
        .child(label.into())
}

/// macOS style toggle switch.
pub fn toggle(id: impl Into<ElementId>, on: bool) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(38.))
        .h(px(22.))
        .flex_none()
        .rounded_full()
        .p(px(2.))
        .cursor_pointer()
        .bg(if on { color::accent() } else { rgba(0xFFFFFF2E) })
        .flex()
        .when(on, |d| d.justify_end())
        .child(div().size(px(18.)).rounded_full().bg(rgb(0xFFFFFF)).shadow_sm())
}

/// Segmented control item.
pub fn segment(id: impl Into<ElementId>, label: impl Into<SharedString>, active: bool) -> Stateful<Div> {
    div()
        .id(id)
        .px(px(10.))
        .h(px(24.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(px(6.))
        .text_size(px(12.))
        .cursor_pointer()
        .text_color(if active { color::text() } else { color::text_dim() })
        .when(active, |d| d.bg(rgba(0xFFFFFF33)).shadow_sm())
        .when(!active, |d| d.hover(|s| s.bg(color::hover())))
        .child(label.into())
}

pub fn segmented() -> Div {
    div().flex().gap(px(2.)).p(px(2.)).rounded(px(8.)).bg(rgba(0x00000040))
}

pub fn hsla_of(c: [u8; 4]) -> Hsla {
    Rgba { r: c[0] as f32 / 255.0, g: c[1] as f32 / 255.0, b: c[2] as f32 / 255.0, a: c[3] as f32 / 255.0 }.into()
}

pub fn hex_hsla(c: u32) -> Hsla {
    rgb(c).into()
}

/// Custom window chrome: drag area plus Windows-style min / max / close.
pub fn title_bar(title: impl Into<SharedString>, center: Option<AnyElement>, right: Option<AnyElement>) -> Div {
    let caption = |id: &'static str, icon_path: &'static str, area: WindowControlArea, danger: bool| {
        div()
            .id(id)
            .w(px(46.))
            .flex_none()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .window_control_area(area)
            .when(danger, |d| d.hover(|s| s.bg(rgb(0xC42B1C))))
            .when(!danger, |d| d.hover(|s| s.bg(color::hover())))
            .child(icon(icon_path, px(14.), color::text_dim()))
    };
    div()
        .h(px(44.))
        .w_full()
        .flex_none()
        .flex()
        .items_center()
        .border_b_1()
        .border_color(color::separator())
        .child(
            div()
                .id("drag")
                .flex_1()
                .min_w(px(60.))
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .h_full()
                .flex()
                .items_center()
                .px(px(14.))
                .gap(px(8.))
                .window_control_area(WindowControlArea::Drag)
                .text_size(px(13.))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(color::text_dim())
                .child(title.into()),
        )
        .children(center.map(|c| div().flex_none().child(c)))
        .child(div().flex_1().min_w(px(12.)).h_full().window_control_area(WindowControlArea::Drag))
        .children(right.map(|r| div().flex_none().child(r)))
        .child(caption("min", "icons/minus.svg", WindowControlArea::Min, false))
        .child(caption("max", "icons/maximize.svg", WindowControlArea::Max, false))
        .child(caption("close", "icons/close.svg", WindowControlArea::Close, true))
}

/// Convert an RGBA image into a GPUI render image (BGRA).
pub fn to_render_image(img: &image::RgbaImage) -> std::sync::Arc<RenderImage> {
    let mut data = img.as_raw().clone();
    for p in data.chunks_exact_mut(4) {
        p.swap(0, 2);
    }
    let buf = image::RgbaImage::from_raw(img.width(), img.height(), data).unwrap();
    std::sync::Arc::new(RenderImage::new(smallvec::smallvec![image::Frame::new(buf)]))
}
