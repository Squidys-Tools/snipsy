#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tool {
    Select,
    Crop,
    Arrow,
    Rect,
    FilledRect,
    Ellipse,
    Line,
    Pencil,
    Highlighter,
    Text,
    Counter,
    Blur,
    Pixelate,
    Spotlight,
}

impl Tool {
    pub const TOOLBAR: [Tool; 14] = [
        Tool::Select,
        Tool::Crop,
        Tool::Rect,
        Tool::FilledRect,
        Tool::Ellipse,
        Tool::Line,
        Tool::Arrow,
        Tool::Text,
        Tool::Pencil,
        Tool::Highlighter,
        Tool::Counter,
        Tool::Spotlight,
        Tool::Blur,
        Tool::Pixelate,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tool::Select => "Select (V)",
            Tool::Crop => "Crop (C)",
            Tool::Arrow => "Arrow (A)",
            Tool::Rect => "Rectangle (R)",
            Tool::FilledRect => "Filled Rectangle (F)",
            Tool::Ellipse => "Ellipse (O)",
            Tool::Line => "Line (L)",
            Tool::Pencil => "Pencil (P)",
            Tool::Highlighter => "Highlighter (H)",
            Tool::Text => "Text (T)",
            Tool::Counter => "Counter (N)",
            Tool::Blur => "Blur (B)",
            Tool::Pixelate => "Pixelate (X)",
            Tool::Spotlight => "Spotlight (S)",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Tool::Select => "icons/cursor.svg",
            Tool::Crop => "icons/crop.svg",
            Tool::Arrow => "icons/arrow.svg",
            Tool::Rect => "icons/rect.svg",
            Tool::FilledRect => "icons/rect-filled.svg",
            Tool::Ellipse => "icons/ellipse.svg",
            Tool::Line => "icons/line.svg",
            Tool::Pencil => "icons/pencil.svg",
            Tool::Highlighter => "icons/highlighter.svg",
            Tool::Text => "icons/text.svg",
            Tool::Counter => "icons/counter.svg",
            Tool::Blur => "icons/blur.svg",
            Tool::Pixelate => "icons/pixelate.svg",
            Tool::Spotlight => "icons/spotlight.svg",
        }
    }

    pub fn shortcut(c: &str) -> Option<Tool> {
        Some(match c {
            "v" => Tool::Select,
            "c" => Tool::Crop,
            "a" => Tool::Arrow,
            "r" => Tool::Rect,
            "f" => Tool::FilledRect,
            "o" => Tool::Ellipse,
            "l" => Tool::Line,
            "p" => Tool::Pencil,
            "h" => Tool::Highlighter,
            "t" => Tool::Text,
            "n" => Tool::Counter,
            "b" => Tool::Blur,
            "x" => Tool::Pixelate,
            "s" => Tool::Spotlight,
            _ => return None,
        })
    }

    /// Tools that use the color / stroke controls.
    pub fn uses_color(self) -> bool {
        !matches!(self, Tool::Select | Tool::Crop | Tool::Blur | Tool::Pixelate | Tool::Spotlight)
    }
}
