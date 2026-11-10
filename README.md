# Snipsy

Keyboard-first screenshots for Windows, inspired by CleanShot X. Press a hotkey, grab an area, a window or the screen, then annotate it, pin it above your work, copy it or save it. Snipsy lives in the system tray.

Early (0.1.0), Windows only, built from source.

## Features

- Capture an area, a window, the full screen, or the previous area. A pixel magnifier and crosshair help with the selection.
- All-In-One: one hotkey opens a HUD with the capture modes, aspect lock (1:1, 4:3, 16:9) and exact size entry.
- Quick Access: after a capture, a card appears in the corner. Copy it, save it, pin it, annotate it, or drag it into another app.
- Editor: rectangle, ellipse, line, arrow, pencil, highlighter, text, counters, spotlight, blur, pixelate, crop, resize, rotate, flip, and a gradient background tool.
- Pinned screenshots: float a capture above other windows. Resize it, change its opacity, or lock it click-through.
- Text capture (OCR): select part of the screen and copy its text, using Windows' built-in on-device OCR.
- Self-timer countdown capture.
- History: recent captures are kept for 30 days by default.
- Every hotkey can be changed in Settings.

## Default hotkeys

| Action | Hotkey |
| --- | --- |
| All-In-One | `Ctrl+Shift+1` |
| Capture text (OCR) | `Ctrl+Shift+2` |
| Capture fullscreen | `Ctrl+Shift+3` |
| Capture area | `Ctrl+Shift+4` |
| Capture window | `Ctrl+Shift+5` |
| Capture previous area | `Ctrl+Shift+6` |
| Self-timer | `Ctrl+Shift+7` |
| Open capture history | `Ctrl+Shift+H` |
| Restore recently closed | `Ctrl+Shift+R` |
| Hide / show overlays | `Ctrl+Shift+O` |

## Build and run

You need Windows 10 or 11 and the stable [Rust toolchain](https://rustup.rs).

```powershell
git clone https://github.com/Squidys-Tools/snipsy.git
cd snipsy
cargo run --release
```

The release binary is `target\release\snipsy.exe`. Right-click the tray icon for every capture mode, History, Settings and Quit.

A second launch hands its arguments to the running copy and exits:

```powershell
snipsy --capture area      # full, window, text, aio, timer, history, settings
snipsy --open shot.png     # add --pin or --quick
```

## Documentation

- [Docs index](docs/README.md)
- Using Snipsy: [capturing](docs/user/capturing.md), [annotating](docs/user/annotating.md), [Quick Access](docs/user/quick-access.md), [pinning](docs/user/pinning.md), [text recognition](docs/user/text-recognition.md), [history](docs/user/history.md), [settings](docs/user/settings.md)
- Working on Snipsy: [AGENTS.md](AGENTS.md), [architecture](docs/internals/architecture.md), [development](docs/operations/development.md)

## Built with

[Rust](https://www.rust-lang.org/) and [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui), plus Win32 and WinRT for capture, hotkeys, the tray icon and OCR. Icons are from the [Hugeicons](https://hugeicons.com) free set.
