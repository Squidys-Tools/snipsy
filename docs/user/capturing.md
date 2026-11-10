# Capturing

Every capture mode is a global hotkey, a tray menu item, and a command-line flag. Defaults are in [Settings](settings.md). What happens afterwards is covered in [Quick Access](quick-access.md).

## Area

Press `Ctrl+Shift+4` and drag over the screen.

- Release to capture. Selections of 3 px or less are ignored.
- Click without dragging to capture the window under the cursor. On the desktop or taskbar, that captures the whole monitor.
- Hold `Shift` while dragging to lock a square.
- `Space` switches to window mode and back. `W` jumps to window mode.
- `F` captures the full monitor under the cursor.
- `Esc` or right-click cancels.

A magnifier and crosshair follow the cursor. Turn them off in Settings > Screenshots.

## Window

Press `Ctrl+Shift+5`, hover a window, and click. The title and size show above it.

The window is captured by Windows itself, so parts covered by other windows are included. If Windows cannot render it, Snipsy crops the visible screen instead. Corners are rounded, and the result gets a transparent drop shadow, or a gradient background if enabled in Settings > Screenshots.

## Fullscreen

Press `Ctrl+Shift+3`. Captures the monitor holding the cursor, with no selection step.

## Previous area

Press `Ctrl+Shift+6` to capture the last area you selected, with no selection step. With no previous area, it starts a normal area capture.

## Self-timer

Press `Ctrl+Shift+7`. A countdown appears in the middle of the monitor under the cursor, then the monitor is captured. The delay is 3, 5 or 10 seconds (Settings > Screenshots). Starting any overlay capture cancels the countdown.

From All-In-One, the Timer button counts down and then captures your current selection.

## All-In-One

Press `Ctrl+Shift+1`. The overlay shows a HUD at the bottom with:

- **Area**, **Fullscreen**, **Window**, **Timer** and **OCR** buttons. OCR is described in [Text recognition](text-recognition.md).
- **Aspect lock**: Free, 1:1, 4:3, 16:9. The selection keeps that ratio while you draw or resize it.
- **Size boxes**: width and height in real screen pixels.
- **Capture** and **Cancel** buttons.

All-In-One opens with your previous area selected.

Working with the selection:

- Drag to draw a new one. Drag inside to move it. Drag a handle to resize it.
- Arrow keys nudge by 1 px; `Shift` nudges by 10.
- `Enter`, double-click, or **Capture** takes the shot.
- `Esc` clears the selection; `Esc` again cancels. In window or OCR mode, `Esc` returns to the main view.

### Exact size

Click a size box, type up to five digits, and press `Enter`. `Tab` applies and moves to the other box. `Esc` stops typing. The selection resizes around its centre, keeps the aspect lock, and is created in the middle of the screen if none exists.

## Multiple monitors

Each monitor gets its own overlay. Only one selection exists at a time: starting a new one clears the others. Mode and aspect lock follow you across monitors.
