use super::{Command, Mode};
use std::collections::HashMap;
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem};
use tray_icon::{MouseButton, TrayIcon, TrayIconBuilder, TrayIconEvent};

fn tray_icon_image() -> tray_icon::Icon {
    let n = 32u32;
    let mut px = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        for x in 0..n {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            let r = 7.0;
            let cx = fx.clamp(r + 1.0, n as f32 - r - 1.0);
            let cy = fy.clamp(r + 1.0, n as f32 - r - 1.0);
            let d = ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt();
            let a = (r - d + 0.5).clamp(0.0, 1.0);
            let i = ((y * n + x) * 4) as usize;
            let t = fy / n as f32;
            let (mut cr, mut cg, mut cb) = (
                (40.0 + 20.0 * t) as u8,
                (150.0 - 50.0 * t) as u8,
                255u8,
            );
            let (lx, ly) = (x as i32, y as i32);
            let corner = |a: i32, b: i32| (8..=11).contains(&a) && (8..=23).contains(&b) && !(12..=19).contains(&b);
            let inside = corner(lx, ly) || corner(31 - lx, ly) || corner(ly, lx) || corner(31 - ly, lx);
            let dot = ((fx - 16.0).powi(2) + (fy - 16.0).powi(2)).sqrt() < 3.2;
            if inside || dot {
                cr = 255;
                cg = 255;
                cb = 255;
            }
            px[i] = cr;
            px[i + 1] = cg;
            px[i + 2] = cb;
            px[i + 3] = (a * 255.0) as u8;
        }
    }
    tray_icon::Icon::from_rgba(px, n, n).unwrap()
}

pub(super) fn build_tray(tx: flume::Sender<Command>) -> Option<TrayIcon> {
    let menu = Menu::new();
    let mut map: HashMap<MenuId, Command> = HashMap::new();
    let mut add = |label: &str, cmd: Command| {
        let item = MenuItem::new(label, true, None);
        map.insert(item.id().clone(), cmd);
        let _ = menu.append(&item);
    };
    add("All-In-One", Command::Capture(Mode::AllInOne));
    add("Capture Area", Command::Capture(Mode::Area));
    add("Capture Previous Area", Command::PreviousArea);
    add("Capture Fullscreen", Command::Fullscreen);
    add("Capture Window", Command::Capture(Mode::Window));
    add("Self-Timer", Command::SelfTimer(None));
    add("Capture Text (OCR)", Command::Capture(Mode::Ocr));
    let _ = menu.append(&PredefinedMenuItem::separator());
    add("Open Capture History", Command::OpenHistory);
    add("Restore Recently Closed", Command::RestoreLast);
    add("Hide / Show Overlays", Command::ToggleOverlays);
    add("Unlock Pinned Screenshots", Command::UnlockPins);
    let _ = menu.append(&PredefinedMenuItem::separator());
    add("Settings…", Command::OpenSettings);
    add("Quit Snipsy", Command::Quit);

    let tray = TrayIconBuilder::new()
        .with_tooltip("Snipsy")
        .with_icon(tray_icon_image())
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .build()
        .map_err(|e| eprintln!("tray: {e}"))
        .ok()?;

    let mtx = tx.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        if let Some(c) = map.get(&e.id) {
            let _ = mtx.send(c.clone());
        }
    }));
    TrayIconEvent::set_event_handler(Some(move |e: TrayIconEvent| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: tray_icon::MouseButtonState::Up, .. } = e {
            let _ = tx.send(Command::Capture(Mode::AllInOne));
        }
    }));
    Some(tray)
}
