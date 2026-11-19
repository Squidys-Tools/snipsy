use super::motion;
use super::theme::{color, font_size, radius, FONT};
use gpui::prelude::*;
use gpui::*;

struct Tip(SharedString);

impl Render for Tip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        motion::fade_in(
            "tip",
            div()
                .px(px(8.))
                .py(px(4.))
                .rounded(radius::SM)
                .bg(color::hud())
                .border_1()
                .border_color(color::hud_border())
                .shadow_lg()
                .text_size(font_size::CAPTION)
                .font_family(FONT)
                .text_color(color::text())
                .child(self.0.clone()),
        )
    }
}

pub fn tip(text: impl Into<SharedString>) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_, cx| cx.new(|_| Tip(text.clone())).into()
}
