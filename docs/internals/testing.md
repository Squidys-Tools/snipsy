# Testing

## What gets a test

The crate has no tests yet. `cargo test` must still pass, and the first tests should go where pure logic is easiest to break:

- Parsing and persistence: `Settings::load` (defaults, hotkey backfill, scale clamp, `.bak` on bad JSON), `History::load` (retention, rebuild from PNGs), `pretty_accel`, `hotkey_recorder::accel_from`.
- Geometry: `overlay::geometry` (`ratio_corner`, `clamp_into`), `PRect::intersect`, `annotate::doc` (`layout`, `balance_trim`, `Annotation::scale` and `translate`, `spotlight_dim_cells`, `smooth`).
- Image logic: `render::apply_effects` regions, `unique_path`.

Everything else (windows, overlays, GPUI views, Win32 calls) is verified by running the app. Follow the rules in `AGENTS.md`: extend an existing test first, one layer, no sleeps, no comments in tests, and visual bugs get captures rather than tests.

## Verifying visuals

GPUI windows are DirectComposition surfaces. GDI and `PrintWindow` grabs of a Snipsy window miss their content. Capture the desktop instead (a real screen capture, then crop to the window).

Check these cases for any window change:

- Two monitors, one at a different scale (125% or 150%). Most window bugs only show there.
- Light and dark desktop behind a transparent HUD (border or corner artefacts).
- An editor change: the editor next to the exported PNG, per `annotation-rendering.md`.

## Running beside a release build

Debug builds use a separate mutex and IPC window class (see `src/instance.rs`), so `cargo run` starts even when an installed release Snipsy is running. Both register the same global hotkeys by default; the second to start shows "Couldn't register ..." in Settings > Shortcuts. Change the debug build's hotkeys, or quit the release build, when testing hotkeys.

Debug builds keep a console, so `eprintln!` output is visible. Release builds use the Windows subsystem and have none.

## Driving a running instance

A second launch forwards its arguments to the running instance of the same build type and exits:

```powershell
target\debug\snipsy.exe --capture aio
```

`--capture` takes `area`, `full`, `window`, `text`, `aio`, `timer`, `history` or `settings`. Any other value starts an area capture. With no arguments, All-In-One starts.

`--open <file>` loads an image without capturing. Add `--pin` for a pinned window or `--quick` for a Quick Access card; otherwise it opens the editor. Relative paths resolve against the caller's directory.

Previous area, Restore Recently Closed, Hide / Show Overlays and Unlock Pins have no flag. Use the hotkeys or the tray menu.
