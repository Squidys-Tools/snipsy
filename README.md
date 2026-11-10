# Snipsy

**Keyboard-first screenshots for Windows.** Snipsy is a screenshot tool inspired by CleanShot X. Press a hotkey, grab an area, a window, or the whole screen, then mark it up, pin it above your work, copy it, or save it. It lives quietly in the system tray.

> **Status:** early (0.1.0), Windows only. There's no installer yet, so you build it from source (see [Install](#install)).

## Features

- **Capture** any area, window, or the full screen. A pixel magnifier and crosshair help you line up the selection, and you can constrain it to 1:1, 4:3, or 16:9.
- **All-In-One**: one hotkey opens a HUD where you pick the capture mode.
- **Quick Access overlay**: after each capture, a thumbnail appears in the corner. Copy it, save it, drag it into another app, or click it to annotate.
- **Annotation editor**: arrows, rectangles, ellipses, lines, pencil, highlighter, text, numbered counters, spotlight, blur, pixelate, and crop.
- **Pinned screenshots**: float a capture above your other windows. Resize it, adjust its opacity, or lock it click-through.
- **Text capture (OCR)**: select part of the screen and copy the text in it, using Windows' built-in OCR.
- **Self-timer**: capture after a countdown, for example to catch a menu.
- **History**: recent captures are kept for 30 days by default, so you can reopen, copy, or pin them again.
- **Custom shortcuts**: every hotkey can be changed in Settings.

## Install

There's no prebuilt download yet, so build Snipsy from source.

**You need:** Windows 10 or 11, and the [Rust toolchain](https://rustup.rs) (stable).

```powershell
git clone https://github.com/Squidys-Tools/snipsy.git
cd snipsy
cargo run --release
```

To build without launching, run `cargo build --release`. The executable is at `target\release\snipsy.exe`.

Once Snipsy is running, find its icon in the system tray. Right-click it to see every capture mode, plus History, Settings, and Quit.

## Shortcuts

These are the default global shortcuts. They work from any app. Change them in **Settings → Shortcuts**.

| Action | Shortcut |
| --- | --- |
| All-In-One | `Ctrl` + `Shift` + `1` |
| Capture text (OCR) | `Ctrl` + `Shift` + `2` |
| Capture fullscreen | `Ctrl` + `Shift` + `3` |
| Capture area | `Ctrl` + `Shift` + `4` |
| Capture window | `Ctrl` + `Shift` + `5` |
| Capture previous area | `Ctrl` + `Shift` + `6` |
| Self-timer | `Ctrl` + `Shift` + `7` |
| Open capture history | `Ctrl` + `Shift` + `H` |
| Restore recently closed | `Ctrl` + `Shift` + `R` |
| Hide / show overlays | `Ctrl` + `Shift` + `O` |

## Quick start

1. Press **Ctrl + Shift + 4** and drag over the area you want.
2. Use the thumbnail that appears in the corner:
   - click it to annotate
   - hover it for **Copy** or **Save**
   - drag it straight into another app

Prefer a different default? In Settings, choose what happens after each capture: show the overlay, copy to the clipboard, save to disk, or open the editor.

## Command line

Snipsy accepts a few flags, which are handy for scripts and your own shortcuts. If Snipsy is already running, a second launch hands its arguments to the running copy and exits.

```powershell
snipsy --capture area      # also: full, window, text, aio, timer, history, settings
snipsy --open shot.png     # open an image in the editor (add --pin or --quick)
```

## Where files live

| What | Where |
| --- | --- |
| Settings | `%APPDATA%\Snipsy\settings.json` |
| Capture history | `%APPDATA%\Snipsy\history\` |
| Saved screenshots | Your Desktop by default (change it in Settings) |

## Built with

- [Rust](https://www.rust-lang.org/) and [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui), the UI framework from Zed
- Windows APIs for screen capture, global hotkeys, the tray icon, and OCR
