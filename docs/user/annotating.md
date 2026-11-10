# Annotating

The editor opens when you click a Quick Access card, press its edit button, choose **Annotate** as the after-capture action, or double-click a history entry. The default tool is Arrow.

## Tools

Pick a tool from the toolbar or press its key.

| Key | Tool |
| --- | --- |
| `V` | Select |
| `C` | Crop |
| `R` | Rectangle |
| `F` | Filled rectangle |
| `O` | Ellipse |
| `L` | Line |
| `A` | Arrow |
| `T` | Text |
| `P` | Pencil |
| `H` | Highlighter |
| `N` | Counter (auto-numbered 1, 2, 3...) |
| `S` | Spotlight (dims everything outside the box) |
| `B` | Blur |
| `X` | Pixelate |

Blur and pixelate destroy the detail underneath, so they are safe for hiding secrets. They always sit beneath other annotations.

## Drawing

- Drag to draw. Shapes smaller than a few pixels are discarded.
- `Shift` while dragging: arrows and lines snap to 45 degrees, boxes become squares, the highlighter stays horizontal.
- Pick a colour from the palette, or use **Pick color** and click the image. Picked colours are remembered (last six).
- `1` to `5` set the stroke width. Text size and counter size follow it.
- Text styles: Outline, Plain, Pill, Card. Click with the Text tool to type. `Enter` finishes, `Shift+Enter` adds a line.

## Selecting

With Select, click an annotation to select it, drag to move it, and change its colour or width with the colour and stroke controls. `Delete` or `Backspace` removes it. Double-click a text annotation to edit it. `Esc` deselects.

## Undo

`Ctrl+Z` undoes, `Ctrl+Shift+Z` or `Ctrl+Y` redoes. History holds the last 100 steps.

## Crop, resize, rotate, flip

- **Crop**: drag a region, choose Free, 1:1, 4:3, 16:9 or 9:16, and apply with `Enter` or **Apply Crop**. `Esc` cancels. The edges snap to the image border.
- **Resize** (bottom bar): 25%, 50%, 75%, 200%, or shrink so the longer side is at most 2560, 1920 or 1280 px. Annotations scale with the image.
- Rotate left, rotate right, Flip H and Flip V are in the bottom bar. Text and counters stay upright.

## Background

The background button in the title bar opens the Background panel and turns the background on.

- 20 gradient presets.
- Padding: None, S, M, L, XL.
- Corner radius: 0, 8, 16, 28.
- Aspect ratio: Auto, 1:1, 16:9, 4:3, 9:16.
- Shadow on or off.
- Auto Balance trims uneven solid-colour borders so the content sits centred.

## Output

| Action | Key |
| --- | --- |
| Copy to clipboard | `Ctrl+C` |
| Save to the export folder | `Ctrl+S` |
| Save As | `Ctrl+Shift+S` |
| Pin to screen | `Ctrl+P` |

**Drag me** in the title bar drags the edited image into any app as a PNG.

Save writes to the export folder and adds an "Edited" entry to [history](history.md). The file is named `<prefix> YYYY-MM-DD at HH.MM.SS.png`; if it exists, ` (2)`, ` (3)`... is appended.
