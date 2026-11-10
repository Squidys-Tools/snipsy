//! Settings window (sidebar + grouped rows, modelled on CleanShot's preferences).

mod sections;

use super::autostart::launch_at_login;
use super::hotkey_recorder::accel_from;
use crate::app::{self, state, state_mut};
use crate::platform;
use crate::settings::HotkeyAction;
use crate::ui::{self, color, icon, Icon};
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
    let bounds = Bounds::centered(None, size(px(780.), px(620.)), cx);
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
    /// Cancel an in-progress recording and re-enable the global hotkeys.
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
        let item = |id: &'static str, label: &'static str, path: Icon, tint: u32, s: Section, active: bool| {
            div()
                .id(id)
                .h(px(34.))
                .px(px(8.))
                .flex()
                .items_center()
                .gap(px(10.))
                .rounded(px(8.))
                .cursor_pointer()
                .when(active, |d| d.bg(rgba(0xFFFFFF1F)))
                .when(!active, |d| d.hover(|st| st.bg(color::hover())))
                .child(
                    div()
                        .size(px(24.))
                        .rounded(px(6.))
                        .bg(rgb(tint))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon(path, px(14.), rgb(0xFFFFFF))),
                )
                .child(div().text_size(px(13.)).text_color(color::text()).child(label))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.section = s;
                    this.stop_recording(cx);
                    cx.notify();
                }))
        };
        let s = self.section;
        div()
            .w(px(210.))
            .flex_none()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(2.))
            .p(px(10.))
            .bg(color::sidebar_bg())
            .border_r_1()
            .border_color(color::separator())
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(8.))
                    .py(px(12.))
                    .mb(px(6.))
                    .child(
                        div()
                            .size(px(36.))
                            .rounded(px(9.))
                            .bg(linear_gradient(160., linear_color_stop(rgb(0x3D9BFF), 0.), linear_color_stop(rgb(0x0A64E0), 1.)))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(icon(Icon::Area, px(20.), rgb(0xFFFFFF))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child("Snipsy"))
                            .child(div().text_size(px(11.)).text_color(color::text_faint()).child("Screenshot toolkit")),
                    ),
            )
            .child(item("s-gen", "General", Icon::Settings, 0x8E8E93, Section::General, s == Section::General))
            .child(item("s-keys", "Shortcuts", Icon::Keyboard, 0x636366, Section::Shortcuts, s == Section::Shortcuts))
            .child(item("s-quick", "Quick Access", Icon::Image, 0x30B158, Section::QuickAccess, s == Section::QuickAccess))
            .child(item("s-shots", "Screenshots", Icon::Area, 0x0A84FF, Section::Screenshots, s == Section::Screenshots))
            .child(div().flex_1())
            .child(item("s-about", "About", Icon::Sparkles, 0xAF52DE, Section::About, s == Section::About))
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
                    .child(self.sidebar(cx))
                    .child(div().id("content").flex_1().min_w_0().h_full().overflow_y_scroll().px(px(28.)).py(px(22.)).child(self.content(cx))),
            )
    }
}
