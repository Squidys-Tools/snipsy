//! Settings window (sidebar + grouped rows, modelled on CleanShot's preferences).

use crate::app::{self, state, state_mut};
use crate::settings::{default_hotkeys, pretty_accel, AfterCapture, HotkeyAction, OverlayCorner};
use crate::ui::{self, color, icon};
use crate::platform;
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

// ------------------------------------------------------------ launch at login

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";

fn reg(args: &[&str]) -> bool {
    use std::os::windows::process::CommandExt;
    std::process::Command::new("reg")
        .args(args)
        .creation_flags(0x0800_0000)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn launch_at_login() -> bool {
    reg(&["query", RUN_KEY, "/v", "Snipsy"])
}

fn set_launch_at_login(on: bool) {
    if on {
        if let Ok(exe) = std::env::current_exe() {
            let v = format!("\"{}\"", exe.display());
            reg(&["add", RUN_KEY, "/v", "Snipsy", "/t", "REG_SZ", "/d", &v, "/f"]);
        }
    } else {
        reg(&["delete", RUN_KEY, "/v", "Snipsy", "/f"]);
    }
}

// ------------------------------------------------------------ hotkey recording

/// gpui reports Shift+digit / Shift+punctuation as the shifted character with
/// `shift` cleared; map it back to the physical (US layout) key.
fn unshift(c: &str) -> Option<&'static str> {
    Some(match c {
        "!" => "1",
        "@" => "2",
        "#" => "3",
        "$" => "4",
        "%" => "5",
        "^" => "6",
        "&" => "7",
        "*" => "8",
        "(" => "9",
        ")" => "0",
        "_" => "-",
        "+" => "=",
        "{" => "[",
        "}" => "]",
        "|" => "\\",
        ":" => ";",
        "\"" => "'",
        "<" => ",",
        ">" => ".",
        "?" => "/",
        "~" => "`",
        _ => return None,
    })
}

fn accel_from(ks: &Keystroke) -> Option<String> {
    let mut m = ks.modifiers.clone();
    let k = match unshift(ks.key.as_str()) {
        Some(base) => {
            m.shift = true;
            base
        }
        None => ks.key.as_str(),
    };
    let code = match k {
        k if k.len() == 1 && k.chars().next().unwrap().is_ascii_alphabetic() => format!("Key{}", k.to_ascii_uppercase()),
        k if k.len() == 1 && k.chars().next().unwrap().is_ascii_digit() => format!("Digit{k}"),
        k if k.starts_with('f') && k[1..].parse::<u8>().is_ok() => k.to_ascii_uppercase(),
        "space" => "Space".into(),
        "-" => "Minus".into(),
        "=" => "Equal".into(),
        "[" => "BracketLeft".into(),
        "]" => "BracketRight".into(),
        ";" => "Semicolon".into(),
        "'" => "Quote".into(),
        "," => "Comma".into(),
        "." => "Period".into(),
        "/" => "Slash".into(),
        "\\" => "Backslash".into(),
        "`" => "Backquote".into(),
        _ => return None,
    };
    let fkey = code.starts_with('F') && code.len() <= 3;
    if !(m.control || m.alt || m.shift || m.platform) && !fkey {
        return None;
    }
    let mut parts = vec![];
    if m.control {
        parts.push("ctrl".to_string());
    }
    if m.alt {
        parts.push("alt".to_string());
    }
    if m.shift {
        parts.push("shift".to_string());
    }
    if m.platform {
        parts.push("super".to_string());
    }
    parts.push(code);
    Some(parts.join("+"))
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
        let item = |id: &'static str, label: &'static str, path: &'static str, tint: u32, s: Section, active: bool| {
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
                            .child(icon("icons/area.svg", px(20.), rgb(0xFFFFFF))),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_size(px(14.)).font_weight(FontWeight::SEMIBOLD).child("Snipsy"))
                            .child(div().text_size(px(11.)).text_color(color::text_faint()).child("Screenshot toolkit")),
                    ),
            )
            .child(item("s-gen", "General", "icons/settings.svg", 0x8E8E93, Section::General, s == Section::General))
            .child(item("s-keys", "Shortcuts", "icons/keyboard.svg", 0x636366, Section::Shortcuts, s == Section::Shortcuts))
            .child(item("s-quick", "Quick Access", "icons/image.svg", 0x30B158, Section::QuickAccess, s == Section::QuickAccess))
            .child(item("s-shots", "Screenshots", "icons/area.svg", 0x0A84FF, Section::Screenshots, s == Section::Screenshots))
            .child(div().flex_1())
            .child(item("s-about", "About", "icons/sparkles.svg", 0xAF52DE, Section::About, s == Section::About))
    }

    fn content(&self, cx: &mut Context<Self>) -> AnyElement {
        let st = &state(cx).settings;
        let group = || div().flex().flex_col().rounded(px(10.)).bg(color::group_bg()).border_1().border_color(color::separator());
        let heading = |t: &'static str| div().mt(px(18.)).mb(px(8.)).text_size(px(13.)).font_weight(FontWeight::SEMIBOLD).text_color(color::text()).child(t);
        let row = |label: SharedString, sub: Option<SharedString>, control: AnyElement, last: bool| {
            div()
                .min_h(px(44.))
                .px(px(14.))
                .py(px(8.))
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(12.))
                .when(!last, |d| d.border_b_1().border_color(color::separator()))
                .child(
                    // Text shrinks and wraps; when the control no longer fits beside
                    // it, the control wraps onto its own line (right aligned).
                    div()
                        .flex_1()
                        .min_w(px(140.))
                        .flex()
                        .flex_col()
                        .child(div().text_size(px(13.)).text_color(color::text()).child(label))
                        .children(sub.map(|s| div().text_size(px(11.)).text_color(color::text_faint()).child(s))),
                )
                .child(div().flex_none().ml_auto().max_w_full().child(control))
        };

        macro_rules! toggle_row {
            ($id:literal, $label:expr, $sub:expr, $field:ident, $last:expr) => {{
                let on = st.$field;
                row(
                    $label.into(),
                    $sub,
                    ui::toggle($id, on)
                        .on_click(cx.listener(|_, _, _, cx| {
                            state_mut(cx).settings.$field = !state(cx).settings.$field;
                            save(cx);
                            cx.notify();
                        }))
                        .into_any_element(),
                    $last,
                )
            }};
        }

        let title = |t: &'static str| div().text_size(px(20.)).font_weight(FontWeight::BOLD).text_color(color::text()).child(t);

        match self.section {
            Section::General => {
                let after = st.after_capture;
                let mut seg = ui::segmented();
                for (i, (l, v)) in [("Overlay", AfterCapture::QuickAccess), ("Copy", AfterCapture::Copy), ("Save", AfterCapture::Save), ("Annotate", AfterCapture::Annotate)].into_iter().enumerate() {
                    seg = seg.child(ui::segment(("after", i), l, after == v).on_click(cx.listener(move |_, _, _, cx| {
                        state_mut(cx).settings.after_capture = v;
                        save(cx);
                        cx.notify();
                    })));
                }
                let export = st.export_dir.clone();
                let folder_name = export.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| export.display().to_string());
                let login = self.login;
                div()
                    .flex()
                    .flex_col()
                    .child(title("General"))
                    .child(heading("App"))
                    .child(
                        group()
                            .child(row(
                                "Launch at login".into(),
                                None,
                                ui::toggle("login", login)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        set_launch_at_login(!this.login);
                                        this.login = launch_at_login();
                                        cx.notify();
                                    }))
                                    .into_any_element(),
                                false,
                            ))
                            .child(toggle_row!("sounds", "Play sounds", Some("Shutter sound after each capture".into()), play_sounds, true)),
                    )
                    .child(heading("After Capture"))
                    .child(
                        group()
                            .child(row("Action".into(), Some("What happens right after a screenshot is taken".into()), seg.into_any_element(), false))
                            .child(toggle_row!("alsocopy", "Also copy to clipboard", None, also_copy_to_clipboard, true)),
                    )
                    .child(heading("Export"))
                    .child(
                        group().child(row(
                            "Export location".into(),
                            Some(export.display().to_string().into()),
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(icon("icons/folder.svg", px(14.), color::accent()))
                                .child(div().text_size(px(13.)).text_color(color::text_dim()).child(folder_name))
                                .child(ui::button("choose", "Choose…", false).on_click(cx.listener(|_, _, _, cx| {
                                    let rx = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("Choose export folder".into()) });
                                    cx.spawn(async move |this, cx| {
                                        if let Ok(Ok(Some(paths))) = rx.await {
                                            if let Some(p) = paths.into_iter().next() {
                                                let _ = cx.update(|cx| {
                                                    state_mut(cx).settings.export_dir = p;
                                                    save(cx);
                                                });
                                                let _ = this.update(cx, |_, cx| cx.notify());
                                            }
                                        }
                                    })
                                    .detach();
                                })))
                                .into_any_element(),
                            true,
                        )),
                    )
                    .into_any_element()
            }
            Section::Shortcuts => {
                let mut g = group();
                let n = HotkeyAction::ALL.len();
                let errors = state(cx).hotkey_errors.clone();
                for (i, a) in HotkeyAction::ALL.iter().copied().enumerate() {
                    let hk = st.hotkeys.iter().find(|h| h.action == a).cloned();
                    let recording = self.recording == Some(a);
                    let label = match (&hk, recording) {
                        (_, true) => "Type shortcut…".to_string(),
                        (Some(h), _) if h.enabled => pretty_accel(&h.accel),
                        _ => "Not set".to_string(),
                    };
                    let enabled = hk.as_ref().is_some_and(|h| h.enabled);
                    let control = div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .id(("rec", i))
                                .min_w(px(150.))
                                .h(px(28.))
                                .px(px(10.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(px(7.))
                                .border_1()
                                .border_color(if recording { color::accent().into() } else { rgba(0xFFFFFF1F) })
                                .bg(rgba(0x00000033))
                                .text_size(px(12.))
                                .text_color(if recording { color::accent().into() } else if enabled { color::text() } else { color::text_faint() })
                                .cursor_pointer()
                                .child(label)
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    // Registered combos never reach the window as key
                                    // events, so suspend them while recording.
                                    app::unregister_hotkeys(cx);
                                    this.recording = Some(a);
                                    window.focus(&this.focus);
                                    cx.notify();
                                })),
                        )
                        .when(recording, |d| {
                            // PrintScreen never arrives as a key-down event, so offer it explicitly.
                            d.child(ui::button(("prtsc", i), "Print Screen", false).on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(h) = state_mut(cx).settings.hotkeys.iter_mut().find(|h| h.action == a) {
                                    h.accel = "PrintScreen".into();
                                    h.enabled = true;
                                }
                                save(cx);
                                this.recording = None;
                                app::register_hotkeys(cx);
                                cx.notify();
                            })))
                        })
                        .child(ui::toggle(("hk-on", i), enabled).on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(h) = state_mut(cx).settings.hotkeys.iter_mut().find(|h| h.action == a) {
                                h.enabled = !h.enabled;
                            }
                            save(cx);
                            this.recording = None;
                            app::register_hotkeys(cx);
                            cx.notify();
                        })));
                    g = g.child(row(a.label().into(), None, control.into_any_element(), i + 1 == n));
                }
                div()
                    .flex()
                    .flex_col()
                    .child(title("Shortcuts"))
                    .child(div().mt(px(6.)).text_size(px(12.)).text_color(color::text_faint()).child("Global shortcuts work from any app. Click a shortcut, then press the new key combination (Esc cancels)."))
                    .child(heading("Capture"))
                    .child(g)
                    .children(errors.into_iter().map(|e| div().mt(px(6.)).text_size(px(11.)).text_color(color::danger()).child(format!("Couldn't register {e} (already used by another app?)"))))
                    .child(
                        div().mt(px(12.)).flex().child(ui::button("reset-keys", "Restore Defaults", false).on_click(cx.listener(|this, _, _, cx| {
                            state_mut(cx).settings.hotkeys = default_hotkeys();
                            save(cx);
                            this.recording = None;
                            app::register_hotkeys(cx);
                            cx.notify();
                        }))),
                    )
                    .into_any_element()
            }
            Section::QuickAccess => {
                let corner = st.overlay_corner;
                let mut pos = ui::segmented();
                for (i, (l, v)) in [("Bottom Left", OverlayCorner::BottomLeft), ("Bottom Right", OverlayCorner::BottomRight), ("Top Left", OverlayCorner::TopLeft), ("Top Right", OverlayCorner::TopRight)].into_iter().enumerate() {
                    pos = pos.child(ui::segment(("corner", i), l, corner == v).on_click(cx.listener(move |_, _, _, cx| {
                        state_mut(cx).settings.overlay_corner = v;
                        save(cx);
                        cx.notify();
                    })));
                }
                let sc = st.overlay_scale;
                let mut sizes = ui::segmented();
                for (i, (l, v)) in [("Small", 0.8f32), ("Medium", 1.0), ("Large", 1.25)].into_iter().enumerate() {
                    sizes = sizes.child(ui::segment(("osize", i), l, (sc - v).abs() < 0.01).on_click(cx.listener(move |_, _, _, cx| {
                        state_mut(cx).settings.overlay_scale = v;
                        save(cx);
                        cx.notify();
                    })));
                }
                let secs = st.overlay_auto_close_secs;
                let mut close = ui::segmented();
                for (i, v) in [5u32, 10, 15, 30, 60].into_iter().enumerate() {
                    close = close.child(ui::segment(("ac", i), format!("{v}s"), secs == v).on_click(cx.listener(move |_, _, _, cx| {
                        state_mut(cx).settings.overlay_auto_close_secs = v;
                        save(cx);
                        cx.notify();
                    })));
                }
                div()
                    .flex()
                    .flex_col()
                    .child(title("Quick Access"))
                    .child(heading("Overlay"))
                    .child(
                        group()
                            .child(row("Position on screen".into(), None, pos.into_any_element(), false))
                            .child(row("Overlay size".into(), None, sizes.into_any_element(), true)),
                    )
                    .child(heading("Behavior"))
                    .child(
                        group()
                            .child(toggle_row!("autoclose", "Auto close", Some("Dismiss captures you haven't interacted with".into()), overlay_auto_close, false))
                            .child(row("Close after".into(), None, close.into_any_element(), true)),
                    )
                    .child(div().mt(px(10.)).text_size(px(11.)).text_color(color::text_faint()).child("Tip: right-click a capture to dismiss it, drag it into any app to drop the file, and use “Restore Recently Closed” from the tray to bring it back."))
                    .into_any_element()
            }
            Section::Screenshots => {
                let t = st.self_timer_secs;
                let mut timer = ui::segmented();
                for (i, v) in [3u32, 5, 10].into_iter().enumerate() {
                    timer = timer.child(ui::segment(("timer", i), format!("{v}s"), t == v).on_click(cx.listener(move |_, _, _, cx| {
                        state_mut(cx).settings.self_timer_secs = v;
                        save(cx);
                        cx.notify();
                    })));
                }
                let days = st.history_days;
                let mut hist = ui::segmented();
                for (i, v) in [1u32, 7, 14, 30].into_iter().enumerate() {
                    hist = hist.child(ui::segment(("hist", i), if v == 1 { "1 day".to_string() } else { format!("{v} days") }, days == v).on_click(cx.listener(move |_, _, _, cx| {
                        state_mut(cx).settings.history_days = v;
                        save(cx);
                        cx.notify();
                    })));
                }
                let wp = st.window_padding;
                let mut padding = ui::segmented();
                for (i, (l, v)) in [("Small", 24u32), ("Medium", 48), ("Large", 96)].into_iter().enumerate() {
                    padding = padding.child(ui::segment(("wpad", i), l, wp == v).on_click(cx.listener(move |_, _, _, cx| {
                        state_mut(cx).settings.window_padding = v;
                        save(cx);
                        cx.notify();
                    })));
                }
                div()
                    .flex()
                    .flex_col()
                    .child(title("Screenshots"))
                    .child(heading("Area selection"))
                    .child(
                        group()
                            .child(toggle_row!("mag", "Show magnifier", Some("Pixel-precise zoom next to the cursor".into()), show_magnifier, false))
                            .child(toggle_row!("cross", "Show crosshair", None, show_crosshair, true)),
                    )
                    .child(heading("Window screenshots"))
                    .child(
                        group()
                            .child(toggle_row!("wshadow", "Add shadow", Some("Transparent PNG with a soft drop shadow".into()), window_shadow, false))
                            .child(toggle_row!("wbg", "Add background", Some("Place the window on a gradient backdrop".into()), window_background, false))
                            .child(row("Background padding".into(), None, padding.into_any_element(), true)),
                    )
                    .child(heading("Other"))
                    .child(
                        group()
                            .child(row("Self-timer".into(), None, timer.into_any_element(), false))
                            .child(row("Keep capture history".into(), None, hist.into_any_element(), true)),
                    )
                    .into_any_element()
            }
            Section::About => div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(10.))
                .pt(px(60.))
                .child(
                    div()
                        .size(px(72.))
                        .rounded(px(18.))
                        .bg(linear_gradient(160., linear_color_stop(rgb(0x3D9BFF), 0.), linear_color_stop(rgb(0x0A64E0), 1.)))
                        .flex()
                        .items_center()
                        .justify_center()
                        .shadow_lg()
                        .child(icon("icons/area.svg", px(38.), rgb(0xFFFFFF))),
                )
                .child(div().text_size(px(22.)).font_weight(FontWeight::BOLD).child("Snipsy"))
                .child(div().text_size(px(12.)).text_color(color::text_faint()).child(format!("Version {} · Rust + GPUI for Windows", env!("CARGO_PKG_VERSION"))))
                .child(div().mt(px(8.)).text_size(px(12.)).text_color(color::text_dim()).child("Captures, annotations and text recognition all run locally on this PC."))
                .child(
                    div().mt(px(12.)).flex().gap(px(8.)).child(ui::button("data", "Open Data Folder", false).on_click(|_, _, _| {
                        platform::open_path(&crate::settings::data_dir());
                    })),
                )
                .into_any_element(),
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
