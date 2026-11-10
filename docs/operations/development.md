# Development

## Toolchain

- Windows 10 or 11. The crate uses Win32 and WinRT APIs and does not build elsewhere.
- Stable Rust (edition 2021) from [rustup](https://rustup.rs).
- Node.js, only for regenerating icons.

## Build and run

```powershell
cargo build              # debug
cargo run                # debug, with a console
cargo build --release
cargo run --release      # no console (windows subsystem)
cargo test
```

Dependencies include `gpui` 0.2.2, `tiny-skia` and `ab_glyph` for export, `global-hotkey`, `tray-icon`, `arboard` for the clipboard, and `windows` 0.61 for Win32/WinRT. New Win32 or WinRT APIs need their feature added to the `windows` dependency in `Cargo.toml`.

## Profiles

- `dev`: `opt-level = 1`, and all dependencies at `opt-level = 3`. Dependencies (GPUI, image, tiny-skia) stay optimised in debug builds; keep that when editing the profile.
- `release`: `opt-level = 3`, thin LTO, 4 codegen units.

## Debug beside release

Debug builds use their own mutex and IPC window class, so they run next to an installed release build. They share the settings and history folder and the default global hotkeys. See `docs/internals/testing.md`.

## Icons

Icons in `assets/icons` are the free [Hugeicons](https://hugeicons.com) set (MIT), plus two hand-drawn ones. See `assets/icons/NOTICE.md`. They are embedded in the binary, and the set of embedded icons is declared in `src/ui/assets.rs`.

`scripts/sync-icons.mjs` is the source of truth. It maps each app icon name to a Hugeicons name, defines the custom ones, and rewrites the whole `assets/icons` folder. Hand edits to an SVG are lost on the next run.

To regenerate or add an icon:

```powershell
npm i --prefix <scratch dir> @iconify-json/hugeicons
node scripts/sync-icons.mjs <scratch dir>\node_modules\@iconify-json\hugeicons\icons.json
```

Add the mapping in the script first. The script throws if a Hugeicons name does not exist.

## Data on disk

`dirs::data_dir()` is `%APPDATA%` (Roaming). Snipsy uses `%APPDATA%\Snipsy`:

| Path | Content |
| --- | --- |
| `settings.json` | Settings. Atomic writes; `settings.json.bak` on a parse failure. |
| `history.json` | History index. `history.json.bak` on a parse failure. |
| `history\<id>.png`, `<id>_t.png` | Stored capture and its 480 px thumbnail. |

Other locations:

- Saved screenshots go to the export folder, which defaults to the Desktop (then Pictures).
- `%TEMP%\Snipsy\<capture id>\` holds drag-and-drop files and is deleted at startup.
- Launch at login is the `Snipsy` value under `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.

To reset the app, quit it and delete `%APPDATA%\Snipsy`.
