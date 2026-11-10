pub mod doc;
pub mod editor;
pub mod render;

use crate::app::Capture;
use gpui::*;

pub fn open(capture: Capture, cx: &mut App) {
    editor::open(capture, cx);
}
