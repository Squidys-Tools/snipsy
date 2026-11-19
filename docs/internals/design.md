# Design

Snipsy follows CleanShot X's dark appearance as closely as Windows allows. Reference stills from cleanshot.com live outside the repo; compare captures against them, not against memory.

Decisions:

- Dark only. No light theme, no following the system setting.
- Windows caption buttons (min, max, close) on the right of every title bar. No traffic lights.
- Inter (OFL), embedded in the binary, is the UI font. SF Pro cannot ship on Windows. Annotation text keeps its own fonts in `annotate/render/fonts.rs`.
- Hugeicons stroke icons, 1.5 px stroke look. New icons go through `scripts/sync-icons.mjs`.

## Tokens

Every colour, size and radius in a view comes from `src/ui`. A literal `rgb(0x..)`, `rgba(0x..)` or a radius or font size outside these scales in a view is a bug.

### Colour

| Token | Value | Use |
|---|---|---|
| `window_bg` | `#1E1E1E` | Window body |
| `sidebar_bg` | `#262626` | Settings inset sidebar, editor background panel |
| `toolbar_bg` | `#2A2A2A` | Editor top and bottom bars |
| `canvas_bg` | `#161616` | Editor canvas |
| `group_bg` | `#FFFFFF0A` | Settings row group |
| `hud` | `#262628E6` | HUD panels, toasts, popovers, tooltips |
| `hud_border` | `#FFFFFF1F` | 1 px border of every HUD surface |
| `border` | `#FFFFFF14` | Group and card borders |
| `separator` | `#FFFFFF14` | Hairlines between rows and toolbar groups |
| `control` | `#FFFFFF1A` | Secondary buttons, tool group track |
| `control_hover` | `#FFFFFF26` | |
| `control_pressed` | `#FFFFFF33` | |
| `hover` | `#FFFFFF14` | Ghost button hover |
| `selected` | `#FFFFFF24` | Ghost button active (HUD mode, sidebar item) |
| `field` | `#FFFFFF14` | Text and number fields |
| `text` | `#FFFFFFE6` | Primary text and icons |
| `text_dim` | `#FFFFFF99` | Secondary text, idle icons |
| `text_faint` | `#FFFFFF59` | Hints, disabled |
| `accent` | `#0A84FF` | Primary button, active tool, toggles, selection rings |
| `accent_hover` | `#409CFF` | |
| `on_accent` | `#FFFFFF` | |
| `danger` | `#FF453A` | |
| `close_hover` | `#C42B1C` | Caption close button (Windows convention) |
| `scrim` | `#00000073` | Capture overlay outside the selection |
| `light_pill` | `#F2F2F7E6` | Quick Access Copy / Save pills, text `#1C1C1E` |

Section icon tiles in settings keep their per-section colours; they are content, not chrome.

### Type

Inter, sizes in logical px. Nothing else.

| Token | Size / weight | Use |
|---|---|---|
| `caption` | 11 / 400 | Row subtitles, size labels, shortcuts hints |
| `small` | 12 / 500 | HUD mode labels, segment labels, toolbar text |
| `body` | 13 / 400 | Default text, row labels, buttons (500) |
| `heading` | 13 / 600 | Settings section headings |
| `title` | 15 / 600 | Window and pane titles |
| `display` | 20 / 600 | Size boxes in the capture HUD, About title |

### Shape

- Radius: `r_sm` 6 (segments, swatches tray, small chips), `r_md` 8 (buttons, fields, tool buttons, sidebar items), `r_lg` 12 (groups, cards, popovers, Quick Access cards, magnifier), `r_xl` 18 (capture HUD panels). Pills and toggles are fully round.
- Spacing scale: 2, 4, 6, 8, 12, 16, 24, 32.
- Control height 28. Segments are 24 inside a 28 track. Settings rows are at least 40. Editor top bar 52, bottom bar 44, other title bars 44.
- Shadows: HUD panels, popovers and Quick Access cards use the large shadow; nothing else has one.

## Shared controls

`src/ui` owns: `button` (primary, secondary), `icon_button` (ghost, 32 square, `r_md`), `tool_button` (28 square inside a `control` track; active is `accent` with white icon), `pill_button` (light pill), `toggle`, `segmented` + `segment`, `field`, `slider` (track, accent fill, white knob; reports 0..1 while dragging), `dropdown` (button with chevron that opens a `menu`), `menu` (HUD popover with rows, hover `accent` fill like the macOS menu), `title_bar`, `tip`. Views compose these; they never restyle them.

A popover rendered under an occluding dismiss layer must be a `menu` (or any stateful container that stops mouse down/up propagation): the occluder is part of the hit test, so without the container swallowing the event, the mouse-down closes the menu before a row's click can complete on mouse-up.

## Surfaces

### Capture HUD (All-In-One)

Two separate panels side by side, centred, 48 px above the monitor's work-area bottom, `hud` fill, `hud_border`, `r_xl`, padding 8, large shadow.

- Modes panel: items 76 × 60, icon 22 above a `small` label, `r_lg` hover and active fill. Hairline separator before OCR.
- Size panel: width field, a faint `×`, height field. Fields 72 × 36, `field` fill, `r_md`, `display` size, centred. Separator, then the aspect `dropdown` (crop icon + chevron) whose menu lists Free, 1:1, 4:3, 16:9.
- No Capture or Cancel buttons. Enter, double-click or clicking the active mode captures; Esc and right-click cancel.

### Selection, crosshair, magnifier

- Outside the selection: `scrim`. Selection border 1 px white at 90 %, with a 1 px dark halo outside.
- Handles: 8 px white circles with a dark 1 px ring, corners and edge midpoints.
- Size label: dark pill (`hud`, `r_sm`, `caption`), centred under the selection, flipped above when there is no room.
- Magnifier: 120 px square, `r_lg`, 1 px dark border, pixel grid crosshair; coordinates in `caption` beside the cursor with a dark shadow so they read on any background.
- Window mode: accent fill at 20 %, 3 px accent border, title and size in a `hud` card centred in the window.

### Quick Access

Cards `r_lg`, `border`, large shadow, no padding around the thumbnail. Hover: a dark wash, `pill_button` Copy and Save centred, 26 px round corner buttons (dark fill, white icon) for pin, close, edit and OCR. Selected card has a 2 px accent ring.

### Toast

`hud` card, `r_lg`, title in `body` 500, detail in `caption` `text_dim`.

### Editor

- Top bar 52 high, `toolbar_bg`, no title text (the window title stays for the taskbar). Left: crop, resize and background as `icon_button`s, each in its own `control` chip; crop is still the Crop tool, resize and background toggle their modes. Centre: every other tool as a `tool_button` in one `control` track, hairlines between groups, undo and redo at its right end. Then `dropdown`s: colour (swatch face) and stroke (line face) when the tool or selection uses them, text style when the Text tool or a text annotation is active. Right: Copy (secondary), Save (primary), then the caption buttons. Gaps between groups are drag areas.
- The colour menu holds the palette grid, custom colours and the eyedropper. The stroke menu lists the five widths as lines. The text style menu lists the four styles.
- Bottom bar 44 high, `toolbar_bg`. Left: `caption` `text_faint` label with image size and zoom percent; the canvas always fits, so there is no zoom control. Centre: the "Drag me" chip (`control`, `r_md`, drag icon both sides). While cropping or resizing, the centre shows those controls instead (ratio `segmented`, size, Cancel and Apply, or the resize presets and Done). Right: `icon_button`s for rotate left, rotate right, flip H, flip V, then pin.
- Background panel: left side, 280 wide, `sidebar_bg`, hairline on its right. Sections with `small` `text_dim` headings: preset tiles 44 square `r_md` with a 2 px accent ring when chosen; padding and corner radius as `slider`s; aspect as a `dropdown`; shadow and Auto Balance as `toggle` rows.

### Settings

- Window 760 × 600. Sidebar is an inset card: 8 px from the window edges, `sidebar_bg`, `r_lg`, `border`. App name block at the top, items 32 high with a 22 px colour tile (`r_sm`) and `body` label, active item `selected`. About pinned at the bottom.
- Pane: `title` at the top, then sections: `heading` 18 above, 8 below; groups `group_bg`, `border`, `r_lg`; rows at least 40, padding 12 × 8, hairline between rows; label `body`, subtitle `caption` `text_dim`; control right-aligned.

### History

Window with filter pills (`segment` style but fully round, active `accent`) across the top and a grid of `r_lg` cards. Hovered card gets the accent ring and Restore / Copy / Pin / Delete actions.

### Pin

1 px `hud_border` frame, accent while hovered. No shadow: the pin window's bounds are the image bounds (zoom, placement and DPI handling depend on it), and GPUI shadows clip at the window edge. Hover shows a `hud` toolbar centred along the top edge and a `hud` pill with zoom and opacity.
