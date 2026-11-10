# Annotation rendering

## Two renderers

Annotations are drawn twice, by different engines:

- **Live**: `annotate/editor/canvas.rs` paints into the editor canvas with GPUI (`paint_quad`, `paint_path`). Text and counters are GPUI `div` elements overlaid on the canvas, not paths.
- **Export**: `annotate/render/` rasterises the whole `Doc` on the CPU with tiny-skia (shapes) and ab_glyph (text). Copy, Save, Save As, Pin and Drag me all use it.

They must look the same. A change to how a shape looks is two changes, and the check is to compare the editor against an exported PNG.

Things deliberately kept equal:

- Geometry shared in `annotate/doc`: `arrow_polygon`, `smooth` (Chaikin smoothing), `spotlight_dim_cells`, `layout` and `balance_trim`. Put new geometry there, not in either renderer.
- Drop shadow under strokes: offset `width * 0.35`, about 22% black (`0.22` in the canvas, alpha 60 in the exporter).
- Highlighter: 43% alpha (`0.43` in the canvas, alpha 110 in the exporter), width `Annotation::highlighter_width`.
- Backdrop gradient: the exporter reimplements GPUI's `linear_gradient` shader (aspect-scaled direction, `t` normalised by the dominant axis). If GPUI's shader changes, gradients drift apart.
- Sizes derive from `Annotation`: `text_size`, `counter_radius`, `highlighter_width`. Do not recompute them in a renderer.

Where they differ and need care:

- Text. The editor uses GPUI's text system with `Segoe UI`; the exporter loads fonts from `%WINDIR%\Fonts` (Segoe UI, falling back to Arial, then symbol, emoji, CJK and Korean fonts for missing glyphs). If no font loads, export silently draws no text. `Annotation::bounds` for text is an estimate (`0.58 em` per char), used only for hit-testing.
- Text styles (Outline, Plain, Pill, Card) and counter colours are implemented in both places. `render::contrast_on` is shared.

## Document model

`Doc` is `{ base: Arc<RgbaImage>, annotations, backdrop }`. Coordinates are base-image pixels.

- Cloning a `Doc` is cheap, so undo is a `Vec<Doc>` of snapshots (capped at 100).
- `Doc::same_content` compares the base by `Arc::ptr_eq`. Any operation that changes pixels (crop, resize, rotate, flip) must install a new `Arc`.
- Crop, resize, rotate and flip transform the annotations with the image. `Annotation::scale` also tracks `floor_scale`, so the minimum text, counter and highlighter sizes follow the image rather than staying at fixed pixels.
- Rotate and flip move text and counters but keep them upright and unmirrored.

## Effects

Blur and pixelate change pixels, so they are baked into a display image (`Editor::display`) rather than painted as shapes. The key is `Doc::effects_key`. While drawing, the canvas shows only a dashed frame; re-baking on every mouse move is too slow, so `refresh_display` is skipped during drags.

- Effects are inserted before the first non-effect annotation, so vector annotations always draw over them.
- Export order is: effects, spotlight dim, annotations, backdrop.
- Blur pixelates first, then smooths, and pixelate jitters block colours with a seed derived from the region. Both are meant to be irreversible. Keep them that way.

## Backdrop and Auto Balance

`layout` computes the canvas size and where the image sits. Both renderers call it.

With Auto Balance on, `Doc::trim` returns a sub-rectangle of the base. The canvas then draws a pre-cropped copy (`display_trim`) so the rounded corners are cut from the trimmed image, as the exporter does. Keep `display_trim` keyed on the base, effects and trim rect.

Window captures with "Add background" go through the same renderer: the overlay builds a `Doc` with a backdrop and calls `render::render`. "Add shadow" uses `render::window_shadow` instead and produces a transparent PNG.

## Undo

`push_undo` snapshots before a change. Gestures that may turn out to change nothing (click-to-select, starting a text edit) use `push_undo_tentative`, and `drop_noop_undo` removes the snapshot if the document is unchanged, restoring the redo stack. Do not call `push_undo` for those; every stray click would clear redo.
