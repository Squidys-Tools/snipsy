# Architecture

## Single instance

`instance::single_instance` takes a named mutex (`Local\SnipsySingleInstance`, with a `-dev` suffix in debug builds). A second launch does not start a UI. It calls `instance::forward`, which sends its argv to the first instance and exits.

- Transport is `WM_COPYDATA` to a hidden message-only window, owned by the `snipsy-ipc` thread of the first instance. Arguments are `\0`-joined. The thread pushes them onto a flume channel that `main` drains on the GPUI executor.
- No arguments means All-In-One.
- The sender resolves a relative `--open` path itself; the receiver's working directory is different.
- The sender calls `AllowSetForegroundWindow` so the receiver may raise its windows. The receiver allows `WM_COPYDATA` through the message filter so a lower-integrity sender still gets through.
- The sender retries for 5 s in case the first instance is still starting.
- `%TEMP%\Snipsy` (drag-and-drop files) is wiped at startup. That is only safe because of the mutex.

The window class and mutex names differ in debug builds. Keep that split when renaming either.

## Command bus

Everything that can start work sends an `app::Command` over the flume channel in `AppState`: tray menu, global hotkeys, CLI flags, HUD buttons. `app::run` is the single dispatcher.

- Tray and hotkey callbacks run on foreign threads. They must only send a `Command`; never touch GPUI from them.
- GPUI exits when the last window closes. `app::init` opens a hidden 1x1 host window far off-screen to keep the process alive.
- `AppState.capturing` guards re-entry. Fullscreen, previous area, self-timer and `overlay::start` all bail out while it is set.
- Adding a `HotkeyAction` touches: the enum, `ALL`, `label`, `default_hotkeys`, `Command::from_action`, and, by hand, the tray menu and the CLI flag match in `main.rs`. `Settings::load` backfills missing default hotkeys, so old settings files keep working.

## Capture flow

Hotkey, tray or CLI becomes `Command::Capture(mode)`:

1. `overlay::start` sets `capturing`, cancels a pending self-timer, hides Quick Access, closes toasts, and waits 60 ms if any were visible. DWM needs a frame to stop compositing removed windows, or they land in the shot.
2. `open_overlays` grabs one frozen frame per monitor (GDI `BitBlt` with `CAPTUREBLT`) and opens one overlay window per monitor.
3. The user selects on the frozen frame. The selection is cropped from it, not recaptured, so what you saw is what you get.
4. `close_all` (deferred, so it is safe inside an overlay's own event handler) removes the overlays, clears `capturing`, re-shows Quick Access, then runs the continuation. Every outcome, including Esc, must go through it or `capturing` stays stuck.
5. The continuation calls `app::finish` (OCR calls `recognize_and_copy` instead and skips history).

Window capture is the exception to "crop the frozen frame". It runs after the overlays close, asks the target window to paint itself with `PrintWindow`, and falls back to a crop of the frozen frame if that errors, hangs or returns black.

Fullscreen, previous area and the self-timer skip the overlay. They call `capture_rect_now` for a live grab and go straight to `finish`.

`app::finish`:

1. Optional shutter sound.
2. Build the `Capture` (id, 560 px preview).
3. `store_in_history` encodes PNG and thumbnail on a background thread. When done it pushes the entry and calls `quick::set_file` so a drag-out can copy the stored file instead of re-encoding.
4. Optional copy to clipboard.
5. Dispatch on `AfterCapture`: Quick Access, copy, save, or editor.

Capture ids are timestamp nanoseconds bumped to be strictly increasing. Quick Access cards, drag-out temp folders and history all key on them; never reuse one for a different image. History restores deliberately take a fresh id.

## Persistence

`settings.json` and `history.json` are written to a temp file and renamed, so a crash cannot truncate them. An unreadable file is copied to `.bak` before defaults (or, for history, a rebuild from the PNGs) replace it. Keep that behaviour; users lose data otherwise.

Settings fields use `#[serde(default)]`, so adding a field is backward compatible. Renaming one is not.

## Editor output

Saving from the editor renders on a background thread, saves through the same `save_to_export` as every other capture, and adds an `Edited` history entry. Copy does not touch history. See `annotation-rendering.md`.
