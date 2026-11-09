pub mod doc;
pub mod editor;
pub mod render;

use crate::app::Capture;
use gpui::*;

/// Open the annotation editor for a capture.
pub fn open(capture: Capture, cx: &mut App) {
    editor::open(capture, cx);
}
