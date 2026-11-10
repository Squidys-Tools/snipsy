use gpui::prelude::*;
use gpui::*;
use std::borrow::Cow;

pub struct Assets;

macro_rules! icons {
    ($($variant:ident => $name:literal),* $(,)?) => {
        #[allow(dead_code)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Icon {
            $($variant),*
        }

        impl Icon {
            pub fn path(self) -> &'static str {
                match self {
                    $(Icon::$variant => concat!("icons/", $name, ".svg")),*
                }
            }
        }

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
    Area => "area", Fullscreen => "fullscreen", Window => "window", Timer => "timer", Ocr => "ocr",
    History => "history", Pin => "pin", Copy => "copy", Save => "save", Edit => "edit", Close => "close",
    Cursor => "cursor", Crop => "crop", Arrow => "arrow", Rect => "rect", RectFilled => "rect-filled",
    Ellipse => "ellipse", Line => "line", Pencil => "pencil", Highlighter => "highlighter", Text => "text",
    Counter => "counter", Blur => "blur", Pixelate => "pixelate", Spotlight => "spotlight", Undo => "undo",
    Redo => "redo", Trash => "trash", Background => "background", Drag => "drag", Settings => "settings",
    Check => "check", Lock => "lock", Unlock => "unlock", Folder => "folder", Restore => "restore",
    Keyboard => "keyboard", Image => "image", Sparkles => "sparkles", Eye => "eye", Minus => "minus",
    Maximize => "maximize", ZoomIn => "zoom-in", ZoomOut => "zoom-out", ChevronDown => "chevron-down",
    More => "more", Camera => "camera", ExternalLink => "external-link", EyeOff => "eye-off",
    FlipH => "flip-h", FlipV => "flip-v", Info => "info", Picker => "picker", Resize => "resize",
    RotateLeft => "rotate-left", RotateRight => "rotate-right", Sound => "sound",
);

impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        Ok(icon_bytes(path).map(Cow::Borrowed))
    }
    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        Ok(ICON_LIST.iter().filter(|p| p.starts_with(path)).map(|p| SharedString::from(*p)).collect())
    }
}

pub fn icon(icon: Icon, size: Pixels, color: impl Into<Hsla>) -> Svg {
    svg().path(icon.path()).size(size).flex_none().text_color(color.into())
}
