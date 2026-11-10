//! Shared HUD look (CleanShot X-style translucent graphite, system-blue accent) and small reusable elements.

mod assets;
mod controls;
mod image;
mod theme;
mod title_bar;

pub use self::image::{hex_hsla, hsla_of, to_render_image};
pub use assets::{icon, Assets, Icon};
pub use controls::{button, icon_button, pill_button, segment, segmented, toggle};
pub use theme::{color, FONT};
pub use title_bar::title_bar;
