# AGENTS.md

## What Snipsy is

Snipsy is a CleanShot X-style screenshot tool for Windows, written in Rust with GPUI. It is a single binary crate that lives in the system tray. It captures areas, windows and screens, annotates them, pins them, runs OCR, and keeps a history.

Windows only. There is no scrolling capture, screen recording or cloud upload; do not claim them anywhere.

## Glossary

- **Capture**: one screenshot (`app::Capture`: RGBA image, preview, kind, id). Every capture goes through `app::finish`.
- **Mode**: how a capture is started (`app::Mode`: All-In-One, Area, Window, Ocr).
- **Command**: a message on the app bus (`app::Command`). Tray, hotkeys, CLI flags and HUD buttons all send commands.
- **Capture overlay**: the full-screen window per monitor that shows the frozen frame while you select. Lives in `src/overlay`.
- **All-In-One (AIO)**: the capture overlay with the HUD bar (mode buttons, aspect lock, size boxes).
- **HUD**: a small translucent graphite panel or window. As a window it is a non-activating, transparent, topmost popup (toasts, countdown, Quick Access).
- **Quick Access**: the stack of capture cards shown in a screen corner after a capture. The settings and tray call it "overlay" too; "Hide / Show Overlays" means Quick Access.
- **Pin**: a floating, always-on-top screenshot window (`src/pin.rs`).
- **Toast**: a short HUD notification ("Copied", "Saved"). The self-timer countdown is a toast in countdown mode.
- **Editor**: the annotation window (`src/annotate/editor`).
- **Doc**: the editor's document: base image, annotations, backdrop. Coordinates are base-image pixels.
- **Backdrop**: the gradient background, padding, corner radius and shadow around the screenshot. The UI calls it "Background".
- **Effect**: blur or pixelate annotation. Baked into the image, drawn beneath other annotations.
- **Frozen frame**: the per-monitor screenshot taken before the overlay opens. Area selections crop it.
- **Peer**: the overlay on another monitor during one capture session.
- **Physical pixels**: real desktop pixels. `src/capture`, `PRect`, saved images and HUD size boxes use them.
- **Logical pixels**: GPUI pixels, physical divided by the monitor scale. Window bounds and layout use them.
- **History**: PNGs plus `history.json` under the data directory. Pruned by retention days.

## Verifying

- `cargo build` and `cargo test` must pass.
- Visual and window behaviour is verified by running the debug build and capturing the windows, not by tests. See `docs/internals/testing.md`.
- Debug builds have their own single-instance identity, so they run beside an installed release build.

## Documentation

- Docs live in `docs/`, split into `user/`, `internals/` and `operations/`.
- Update the doc in the same change that alters the behaviour.
- Internals docs hold decisions, constraints and traps, not a tour of the code.
- Add each new doc to `docs/README.md`.
- User docs describe only what the code does. Verify keys and defaults against the code.

## Where code lives

- `src/main.rs`: entry point, CLI flag handling, single-instance startup.
- `src/instance.rs`: single-instance mutex and argv forwarding over `WM_COPYDATA`.
- `src/app/`: global `AppState`, the `Command` bus, hotkey registration, tray menu, and the post-capture pipeline.
- `src/capture/`: Win32 screen and window capture, monitor and window enumeration, physical coordinates.
- `src/overlay/`: the capture overlay, selection geometry, All-In-One HUD, multi-monitor sync.
- `src/quick.rs`: Quick Access card stack.
- `src/pin.rs`: pinned screenshot windows.
- `src/toast/`: toasts, the shared HUD window opener, self-timer.
- `src/annotate/`: `doc/` (document model), `editor/` (GPUI editor), `render/` (tiny-skia exporter).
- `src/history/`: history storage and the history window.
- `src/settings/`: settings storage, hotkey recorder, autostart, settings window.
- `src/platform/`: thin Win32 and WinRT helpers GPUI does not expose (window styles, clipboard, OCR, shell drag, sound).
- `src/ui/`: shared theme, controls, icons and image helpers.
- `assets/icons/`: embedded SVG icons. `scripts/sync-icons.mjs` regenerates them.

## Taste

- Comments: default to no comment. Write one only for a constraint, an external quirk, or an invariant the code can't show, in one line where you can and four at most. Don't restate names or signatures, narrate steps, or record history. No divider comments.
- Tests: test only behaviour that can break: parsing, persistence, geometry and other pure logic. Extend an existing test before adding one. Test at one layer. No sleeps or timeouts. Visual bugs get captures, not tests. Tests carry no comments.
- Shared controls (buttons, toggles, segmented controls, icons, theme colours) live in `src/ui`. Reuse them; do not restyle per window.
- Win32 calls that GPUI does not cover go in `src/platform`, not inline in views.
- Capture code stays in physical pixels; convert to logical only at the GPUI boundary.
- Anything the annotation exporter draws must match the live editor. See `docs/internals/annotation-rendering.md`.
- Slow work (PNG encode, render, OCR) runs on the background executor; results return through `cx.update`.
- Settings and history are written atomically (temp file, then rename).
- Icons are embedded in the binary. Do not add icons by hand-editing `assets/icons`; add them to the sync script.
