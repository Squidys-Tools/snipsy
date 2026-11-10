# Settings

Open from the tray menu (**Settings...**) or run `snipsy --capture settings`. Changes save immediately.

## General

- **Launch at login**: starts Snipsy when you sign in (current user only).
- **Play sounds**: shutter sound after each capture. On by default.
- **After capture action**: what happens after a screenshot.
  - Overlay: show [Quick Access](quick-access.md) (default).
  - Copy: copy to the clipboard.
  - Save: save to the export folder.
  - Annotate: open the [editor](annotating.md).
- **Also copy to clipboard**: copy in addition to the action above. Off by default.
- **Export location**: where saves go. Defaults to your Desktop.

## Shortcuts

Global hotkeys work from any app. Click a shortcut, press the new combination, `Esc` cancels. A toggle disables a shortcut. **Print Screen** has its own button because it cannot be recorded by pressing it. **Restore Defaults** resets all.

If another app owns a combination, Snipsy shows "Couldn't register ..." under the list.

| Action | Default |
| --- | --- |
| All-In-One | `Ctrl+Shift+1` |
| Capture Text (OCR) | `Ctrl+Shift+2` |
| Capture Fullscreen | `Ctrl+Shift+3` |
| Capture Area | `Ctrl+Shift+4` |
| Capture Window | `Ctrl+Shift+5` |
| Capture Previous Area | `Ctrl+Shift+6` |
| Self-Timer | `Ctrl+Shift+7` |
| Open Capture History | `Ctrl+Shift+H` |
| Restore Recently Closed | `Ctrl+Shift+R` |
| Hide / Show Overlays | `Ctrl+Shift+O` |

## Quick Access

Position, size, and auto close. See [Quick Access](quick-access.md).

## Screenshots

- **Show magnifier** and **Show crosshair** (both on).
- **Window screenshots**: add a shadow (on), or add a gradient background (off) with Small, Medium or Large padding.
- **Self-timer**: 3, 5 or 10 seconds (default 5).
- **Keep capture history**: 1, 7, 14 or 30 days (default 30).

## About

Shows the version. **Open Data Folder** opens the folder below.

## Files on disk

Settings and history live in `%APPDATA%\Snipsy`:

- `settings.json`: all settings. If it cannot be read, Snipsy copies it to `settings.json.bak` and starts with defaults.
- `history.json` and `history\`: capture history.

The file name prefix for saved screenshots (default `Snipsy`) has no setting; change `file_prefix` in `settings.json` while Snipsy is not running.
