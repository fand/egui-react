# Plan: gallery code pane

The task is in [task.md](task.md), the numbers in
[measurements.md](measurements.md).

## 0. Where the time goes

`code_view_ui` is `highlight()` + `Label::new(LayoutJob).selectable(true)`.
Per frame, for `patch` (1661 lines, 66 KB):

| Piece | Cost | Why |
|---|---|---|
| `highlight()` | 0.19 ms | Hashes the whole source for its `FrameCache` and clones the `LayoutJob` (thousands of sections) |
| `Label::new(job)` | ~0.4 ms | Hashes the job again for egui's galley cache |
| painting | ~0 | The `ScrollArea` clip culls rows out of view at tessellation |
| selectable + accesskit | ~1.6 ms | `update_accesskit_for_text_widget` builds one `TextRun` node per row, with per-glyph position vectors, every frame. kittest and the web (`WebA11y`) have accesskit on; native does not |

`Label::new(WidgetText::Galley)` skips both hashes (`Label::layout_in_ui`
returns early). Step 1 uses that. Only the rows on screen need to reach
accesskit; step 2 does that.

## 1. Measurement protocol (after every step)

```sh
cargo test --release -p gallery --test bench -- --ignored --nocapture
cargo test -p gallery
cargo test -p gallery --features snapshot   # if a GPU is around
```

`tests/bench.rs` times `Harness::step` for the full gallery and for each pane
alone, with and without accesskit, for five examples. Add one "After step N"
table to measurements.md with the same columns as the baseline, and the commit
hash. The code-pane variants block stays as it is: it is the breakdown, not
the gate.

## 2. Step 1: one galley, kept across frames

### Change

In `Code` (`examples/gallery/src/lib.rs`), replace the `code_view_ui` leaf:

- Hold `Option<(GalleyKey, Arc<Galley>)>` in a `use_state`. The key is
  `(meta.name, plain, dark_mode, monospace font size, pixels_per_point,
  font_image_size)`.
- In the leaf, if the key differs from the stored one: run `highlight()`,
  set `job.wrap.max_width = f32::INFINITY` (the pane scrolls horizontally, as
  today), lay it out with `ui.fonts_mut(|f| f.layout_job(job))`, store through
  `state.bind()` (no repaint request, same as the plain examples).
- Draw with `ui.add(Label::new(galley.clone()).selectable(true))`.

`font_image_size` is in the key because a held galley stores atlas pixel
coordinates. Growing the atlas keeps them valid; a full atlas reset changes
the image size, and the key with it. `pixels_per_point` for the same reason.

### Expected

Native code pane for `patch`: 0.65 → ~0.06 ms. With accesskit: 2.25 → ~1.5 ms
(the `TextRun` rebuild stays). Gate: native numbers match the "galley
selectable" variant of the bench.

## 3. Step 2: one line per row, through `<VirtualList>`

### Change

- Highlight once per key (same key as step 1), then split the `LayoutJob` at
  each `\n` into one `LayoutJob` per line: clip every section's byte range to
  the line, keep its format. Splitting the job, not highlighting per line,
  keeps block comments and multi-line strings right.
- Lay each line out once into an `Arc<Galley>` (wrap off), stored as
  `Vec<Arc<Galley>>` with the key. 1661 layouts at switch time is a few ms,
  once.
- `row_h` = `ui.fonts(|f| f.row_height(&monospace_font_id))`.
- Draw the lines with `<VirtualList rows={lines.len()} row_h={row_h}
  grow={1.0} render={..}>`; each row is `cx.leaf(.., |ui|
  ui.add(Label::new(galley.clone()).selectable(true)))`.
- Horizontal scroll: `<VirtualList>` is vertical only. Keep the outer
  `<ScrollArea horizontal>` and give the list `w` = the widest galley, so
  long lines scroll sideways as they do today. If nested scroll areas fight
  over the wheel, fall back to `Extend` wrap and clip, and say so in
  measurements.md.

### Selection across rows

egui's `multi_widget_text_select` (on by default) lets a drag run across
several labels, and copy joins them in draw order. A selection whose primary
or secondary cursor scrolls out of view is dropped by egui
(`on_end_pass`: "we didn't see both cursors this frame"). That is how egui's
own `show_rows` behaves too; it is the trade-off of this step and goes in the
task's result. Selecting and copying a screenful of lines must work by hand.

### Expected

With accesskit, `patch` code pane: ~1.5 → under 0.2 ms (about 60 rows reach
accesskit instead of 1661). Native stays at ~0.06 ms. Gate: the completion
criteria in task.md.

## 4. Order

1. Step 1, measure, commit.
2. Step 2, measure, commit.
3. Result table in task.md, and the plain-egui comparison note in README if
   the gallery description mentions the code pane.
