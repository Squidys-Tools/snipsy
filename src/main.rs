//! Snipsy — a CleanShot X-style screenshot toolkit for Windows, built with Rust + GPUI.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod annotate;
mod app;
mod capture;
mod history;
mod instance;
mod overlay;
mod pin;
mod platform;
mod quick;
mod settings;
mod toast;
mod ui;

use gpui::*;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Single instance: a second launch forwards its arguments to the first
    // (no arguments = All-In-One) and exits.
    let _guard = match instance::single_instance() {
        Some(g) => g,
        None => {
            instance::forward(&args);
            return;
        }
    };
    let remote = instance::listen();
    // Drag & drop temp files from earlier sessions are no longer in use.
    let _ = std::fs::remove_dir_all(app::temp_root());
    Application::new().with_assets(ui::Assets).run(move |cx: &mut App| {
        app::init(cx);
        handle_args(&args, cx);
        cx.spawn(async move |cx: &mut AsyncApp| {
            while let Ok(args) = remote.recv_async().await {
                let _ = cx.update(|cx| {
                    if args.is_empty() {
                        app::send(cx, app::Command::Capture(app::Mode::AllInOne));
                    } else {
                        handle_args(&args, cx);
                    }
                });
            }
        })
        .detach();
    });
}

/// `snipsy --capture area|full|window|text|aio|history|settings|timer` and
/// `snipsy --open <file> [--pin|--quick]`, for scripting / testing.
fn handle_args(args: &[String], cx: &mut App) {
    if let Some(i) = args.iter().position(|a| a == "--capture") {
        let cmd = match args.get(i + 1).map(|s| s.as_str()) {
            Some("full") => Some(app::Command::Fullscreen),
            Some("window") => Some(app::Command::Capture(app::Mode::Window)),
            Some("text") => Some(app::Command::Capture(app::Mode::Ocr)),
            Some("aio") => Some(app::Command::Capture(app::Mode::AllInOne)),
            Some("history") => Some(app::Command::OpenHistory),
            Some("settings") => Some(app::Command::OpenSettings),
            Some("timer") => Some(app::Command::SelfTimer(None)),
            _ => Some(app::Command::Capture(app::Mode::Area)),
        };
        if let Some(c) = cmd {
            app::send(cx, c);
        }
    }
    if let Some(i) = args.iter().position(|a| a == "--open") {
        if let Some(path) = args.get(i + 1) {
            if let Ok(img) = image::open(path) {
                let c = app::Capture::new(std::sync::Arc::new(img.to_rgba8()), history::CaptureKind::Area);
                if args.iter().any(|a| a == "--pin") {
                    pin::open(c, cx);
                } else if args.iter().any(|a| a == "--quick") {
                    quick::push(c, cx);
                } else {
                    annotate::open(c, cx);
                }
            }
        }
    }
}
