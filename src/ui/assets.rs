use gpui::prelude::*;
use gpui::*;
use std::borrow::Cow;

pub struct Assets;

macro_rules! icons {
    ($($name:literal),* $(,)?) => {
        fn icon_bytes(path: &str) -> Option<&'static [u8]> {
            match path {
                $(concat!("icons/", $name, ".svg") => Some(include_bytes!(concat!("../../assets/icons/", $name, ".svg"))),)*
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
