use super::{state, state_mut, Command};
use global_hotkey::hotkey::HotKey;
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use gpui::App;
use std::str::FromStr;

pub(super) fn init_hotkeys(cx: &mut App) {
    match GlobalHotKeyManager::new() {
        Ok(m) => state_mut(cx).hotkey_manager = Some(m),
        Err(e) => eprintln!("hotkeys unavailable: {e}"),
    }
    register_hotkeys(cx);
    let tx = state(cx).tx.clone();
    let ids = state(cx).hotkey_ids.clone();
    GlobalHotKeyEvent::set_event_handler(Some(move |e: GlobalHotKeyEvent| {
        if e.state == HotKeyState::Pressed {
            if let Some(a) = ids.lock().unwrap().get(&e.id).copied() {
                let _ = tx.send(Command::from_action(a));
            }
        }
    }));
}

pub fn unregister_hotkeys(cx: &mut App) {
    let st = state_mut(cx);
    if let Some(mgr) = st.hotkey_manager.as_ref() {
        let old = std::mem::take(&mut st.registered);
        let _ = mgr.unregister_all(&old);
    }
}

pub fn register_hotkeys(cx: &mut App) {
    let st = state_mut(cx);
    let Some(mgr) = st.hotkey_manager.as_ref() else { return };
    let old = std::mem::take(&mut st.registered);
    let _ = mgr.unregister_all(&old);
    let ids = st.hotkey_ids.clone();
    let mut ids = ids.lock().unwrap();
    ids.clear();
    st.hotkey_errors.clear();
    for hk in st.settings.hotkeys.clone() {
        if !hk.enabled {
            continue;
        }
        match HotKey::from_str(&hk.accel) {
            Ok(k) => match mgr.register(k) {
                Ok(()) => {
                    ids.insert(k.id(), hk.action);
                    st.registered.push(k);
                }
                Err(e) => st.hotkey_errors.push(format!("{}: {e}", hk.action.label())),
            },
            Err(e) => st.hotkey_errors.push(format!("{}: {e}", hk.action.label())),
        }
    }
}
