mod sections;

use super::autostart::launch_at_login;
use super::hotkey_recorder::accel_from;
use crate::app::{self, state, state_mut};
use crate::platform;
use crate::settings::HotkeyAction;
use crate::ui::{self, color, font_size, icon, radius, Icon};
use gpui::prelude::*;
use gpui::*;

#[derive(Clone, Copy, PartialEq)]
enum Section {
    General,
    Shortcuts,
    QuickAccess,
    Screenshots,
    About,
}

// Section tile tints and the logo gradient are brand content, not chrome tokens.
const TILES: [(&'static str, Icon, u32, Section); 5] = [
    ("General", Icon::Settings, 0x8E8E93, Section::General),
    ("Shortcuts", Icon::Keyboard, 0x636366, Section::Shortcuts),
    ("Quick Access", Icon::Image, 0x30B158, Section::QuickAccess),
    ("Screenshots", Icon::Area, 0x0A84FF, Section::Screenshots),
    ("About", Icon::Sparkles, 0xAF52DE, Section::About),
];
const LOGO_STOPS: (u32, u32) = (0x3D9BFF, 0x0A64E0);

fn logo(size: Pixels, icon_size: Pixels) -> Div {
    div()
        .size(size)
        .rounded(size * 0.25)
        .bg(linear_gradient(160., linear_color_stop(rgb(LOGO_STOPS.0), 0.), linear_color_stop(rgb(LOGO_STOPS.1), 1.)))
        .flex()
        .items_center()
        .justify_center()
        .child(icon(Icon::Area, icon_size, color::on_accent()))
}

pub struct SettingsView {
    section: Section,
    recording: Option<HotkeyAction>,
    focus: FocusHandle,
    /// Cached "launch at login" state (querying it spawns reg.exe).
    login: bool,
    _subs: Vec<Subscription>,
}

pub fn open(cx: &mut App) {
    if let Some(h) = state(cx).settings_window {
        if h.update(cx, |_, window, _| window.activate_window()).is_ok() {
            return;
        }
    }
    let bounds = Bounds::centered(None, size(px(760.), px(600.)), cx);
    let res = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions { title: Some("Snipsy — Settings".into()), appears_transparent: true, traffic_light_position: None }),
            window_min_size: Some(size(px(680.), px(480.))),
            app_id: Some("Snipsy".into()),
            ..Default::default()
        },
        |window, cx| {
            if let Some(h) = platform::hwnd_of(window) {
                platform::force_foreground(h);
            }
            cx.new(|cx| {
                let focus = cx.focus_handle();
                window.focus(&focus);
                let subs = vec![
                    // Global hotkeys are suspended while recording; restore them if the
                    // window closes or loses activation mid-recording.
                    cx.on_release(|this: &mut SettingsView, cx| {
                        if this.recording.take().is_some() {
                            app::register_hotkeys(cx);
                        }
                    }),
                    cx.observe_window_activation(window, |this: &mut SettingsView, window, cx| {
                        if !window.is_window_active() {
                            this.stop_recording(cx);
                        }
                    }),
                ];
                SettingsView { section: Section::General, recording: None, focus, login: launch_at_login(), _subs: subs }
            })
        },
    );
    if let Ok(h) = res {
        state_mut(cx).settings_window = Some(h);
    }
}

fn save(cx: &mut App) {
    state(cx).settings.save();
}

impl SettingsView {
    fn stop_recording(&mut self, cx: &mut Context<Self>) {
        if self.recording.take().is_some() {
            app::register_hotkeys(cx);
            cx.notify();
        }
    }

    fn on_key(&mut self, e: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
        let Some(action) = self.recording else { return };
        if e.keystroke.key == "escape" {
            self.stop_recording(cx);
            return;
        }
        if let Some(accel) = accel_from(&e.keystroke) {
            if let Some(h) = state_mut(cx).settings.hotkeys.iter_mut().find(|h| h.action == action) {
                h.accel = accel;
                h.enabled = true;
            }
            save(cx);
            app::register_hotkeys(cx);
            self.recording = None;
            cx.notify();
        }
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let item = |id: ElementId, label: &'static str, path: Icon, tint: u32, s: Section, active: bool| {
            div()
                .id(id)
                .h(px(32.))
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(8.))
                .rounded(radius::MD)
                .cursor_pointer()
                .when(active, |d| d.bg(color::selected()))
                .when(!active, |d| d.hover(|st| st.bg(color::hover())))
                .child(
                    div()
                        .size(px(22.))
                        .rounded(radius::SM)
                        .bg(rgb(tint))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon(path, px(14.), color::on_accent())),
                )
                .child(div().text_size(font_size::BODY).text_color(color::text()).child(label))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.section = s;
                    this.stop_recording(cx);
                    cx.notify();
                }))
        };
        let s = self.section;
        let mut items = div().flex().flex_col().gap(px(2.));
        for (i, (label, path, tint, sec)) in TILES.iter().copied().enumerate() {
            if sec == Section::About {
                continue;
            }
            items = items.child(item(("section", i).into(), label, path, tint, sec, s == sec));
        }
        div()
            .w(px(200.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(2.))
            .p(px(8.))
            .rounded(radius::LG)
            .border_1()
            .border_color(color::border())
            .bg(color::sidebar_bg())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(6.))
                    .py(px(10.))
                    .mb(px(4.))
                    .child(logo(px(36.), px(20.)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_size(font_size::TITLE).font_weight(FontWeight::SEMIBOLD).child("Snipsy"))
                            .child(div().text_size(font_size::CAPTION).text_color(color::text_faint()).child("Screenshot toolkit")),
                    ),
            )
            .child(items)
            .child(div().flex_1())
            .child(item("s-about".into(), "About", Icon::Sparkles, TILES[4].2, Section::About, s == Section::About))
    }

    fn content(&self, cx: &mut Context<Self>) -> AnyElement {
        match self.section {
            Section::General => self.general(cx),
            Section::Shortcuts => self.shortcuts(cx),
            Section::QuickAccess => self.quick_access(cx),
            Section::Screenshots => self.screenshots(cx),
            Section::About => self.about(),
        }
    }
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .bg(color::window_bg())
            .font_family(ui::FONT)
            .text_color(color::text())
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key))
            .child(ui::title_bar("Settings", None, None))
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .p(px(8.))
                    .child(self.sidebar(cx))
                    .child(div().id("content").flex_1().min_w_0().h_full().overflow_y_scroll().px(px(20.)).py(px(16.)).child(self.content(cx))),
            )
    }
}
