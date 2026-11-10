//! Thin Win32 / WinRT helpers that GPUI does not expose.

mod clipboard;
mod ocr;
mod shell;
mod sound;
mod window;

pub use clipboard::{copy_image, copy_text};
pub use ocr::recognize_text;
pub use shell::{drag_file_out, open_path, reveal_in_explorer};
pub use sound::play_shutter;
pub use window::{
    force_foreground, hide_window, hwnd_of, move_window_by, place_window, place_window_ex, set_click_through,
    set_hud_frame, set_no_activate, set_tool_window, show_no_activate, window_rect,
};
