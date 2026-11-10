# Windows

## HUD windows

Toasts, the countdown and Quick Access are opened by `toast::open_hud`: a `PopUp` window, transparent background, not focused, not shown. After creation it gets, in order:

1. `set_hud_frame`: removes the Windows 11 1 px DWM border and corner rounding. Without it a transparent HUD shows a visible frame. No-op on Windows 10.
2. `set_no_activate`: `WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`, so clicking a card never steals focus from the app you are working in, and the window stays out of Alt-Tab.
3. `place_window`: `SetWindowPos` to an exact physical rect, topmost.

Never rely on `window_bounds` alone for position. GPUI takes logical bounds; the code always forces the exact physical rect afterwards with `place_window`.

Pins are similar but must take focus (keys, scroll): `set_tool_window` instead of `set_no_activate`, then `force_foreground`.

### Sizing a HUD

GPUI only learns a window's new size when the resize arrives outside an app update. `open_hud` sizes the window while GPUI is still creating it, so the view first lays out for the default window size (thousands of pixels). A view that fills its window (`size_full`, centring) then draws its content off-screen and the HUD looks empty.

`open_hud` therefore spawns `platform::resync_size`, which changes the height by one pixel and back so GPUI receives real size messages. Keep that call. Do not strip `WS_CAPTION` from a HUD to get rid of the invisible resize border: the frame change reaches the DirectX renderer before it has resources and panics.

The client area of a HUD is 8 px narrower on the left, right and bottom than its window rect (GPUI keeps the resize border for title-bar-less windows). Lay out against the client size.

## DPI changes

Moving a window onto a monitor with another DPI makes GPUI's `WM_DPICHANGED` handler resize it to the system-suggested rect. Consequences:

- Place a window twice when the target monitor differs from the current one (`quick.rs`, `pin.rs`): the second call lands after the DPI switch.
- A pin's zoom is derived from the real window width, never from a stored value (`PinView::current_zoom`).

## Physical and logical pixels

- `src/capture` is physical throughout. `PRect` is a physical rect in virtual-desktop coordinates. Monitor `scale` is effective DPI / 96.
- GPUI layout is logical. `physical = logical * scale` for the monitor the window is on.
- The overlay converts at the boundary: `local_from_global`, `to_image_rect`, `global_point` in `overlay/mod.rs`. Selections are stored logical, rounded to the physical grid on crop.
- HUD size boxes show and accept physical pixels. `apply_size` snaps the origin to the physical grid so the crop is exactly the typed size.
- The overlay pairs `capture::monitors()` with `cx.displays()` by index, falling back to computed bounds. `place_window` with the monitor's physical rect corrects any mismatch.

## Capture overlay across monitors

One overlay window per monitor, each holding its own frozen frame. Constraints:

- Only the overlay under the cursor is activated (`force_foreground`). Activating each in turn leaves the last one foreground.
- At most one overlay holds a selection. A new selection calls `clear_peer_selections`. Mode and aspect are copied to peers by `sync_peers`.
- Enter, arrows, the Capture button and typed sizes may arrive at an overlay that has no selection. `with_selection_holder` routes them to the one that does.
- Those helpers use `cx.defer`. Updating another overlay's entity from inside an update of the current one is a re-entrancy error.
- `on_window_should_close` routes Alt+F4 to `close_all`. Closing a single overlay would leave `capturing` set.
- GPUI captures the mouse during a drag, so event positions can be outside the window. `on_move` clamps them.
- In All-In-One, the Window sub-mode parks the selection in `saved_selection` and restores it on return.

## Window capture and enumeration

`capture::list_windows` is called once per session, front to back (`EnumWindows` order). It skips:

- our own process, minimised and DWM-cloaked windows,
- `WS_EX_TRANSPARENT` windows,
- `Progman`, `WorkerW` and both taskbar classes,
- tool windows with no title,
- windows smaller than 8 px.

Hit-testing and labels use `DWMWA_EXTENDED_FRAME_BOUNDS`, not `GetWindowRect`, which includes invisible resize borders.

`capture_window` uses `PrintWindow` with `PW_RENDERFULLCONTENT` into a bitmap sized by `GetWindowRect`, then crops to the frame bounds. `PrintWindow` sends `WM_PRINT` with no timeout, so a hung target would block: `IsHungAppWindow` is checked first. Some GPU-composited apps return black; `is_mostly_black` samples the bitmap and the caller falls back to the frozen frame.

## Foreground rights

Windows refuses `SetForegroundWindow` from a process that did not receive the last input. `force_foreground` sends a synthetic Alt tap and attaches to the foreground thread's input queue around `SetFocus`. It is used for the focused overlay, pins and the editor. HUD windows are non-activating and never use it.

## Click-through pins

Lock mode sets `WS_EX_TRANSPARENT | WS_EX_LAYERED`. A click-through window cannot be clicked, so the unlock path is the tray command `UnlockPins`, not a button on the pin.

## Hotkeys

- `global-hotkey` events arrive on another thread; the handler only sends a `Command`.
- Registered hotkeys never reach window key events. The settings recorder unregisters all of them while listening and re-registers on finish, on window release, and on deactivation.
- GPUI reports Shift+digit as the shifted character with `shift` cleared. `hotkey_recorder::unshift` maps it back to the physical key, assuming a US layout.
- Print Screen produces no key-down event, so the recorder has a dedicated button.

## Other quirks

- Autostart writes `HKCU\...\Run` by shelling out to `reg.exe` with `CREATE_NO_WINDOW`. The state is cached in the settings view because querying spawns a process. It stores the path of the running exe, so a debug build registers itself.
- `drag_file_out` uses `SHDoDragDrop`, which runs a modal loop. Call it from a spawned task, not from inside GPUI event dispatch.
- OCR calls WinRT and must run off the UI thread. `recognize_and_copy` runs it on a fresh thread that initialises WinRT itself.
