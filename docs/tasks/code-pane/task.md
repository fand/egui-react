# Task: gallery code pane cost

## Result (2026-09-07)

Both steps done; numbers in [measurements.md](measurements.md).

| Completion criterion | Status |
|---|---|
| Code pane for `patch` under 0.2 ms with accesskit | **Pass.** 0.09 ms, from 2.25 |
| Code pane for `list-10k` under 0.03 ms native, flat in source length | **Flat: pass.** 0.04–0.07 for every example, from 0.08 (list-10k) and 0.67 (patch). **0.03: miss** at 0.04; the rest is sixty selectable labels, the same as plain `show_rows` |
| Selection and copy still work | **Pass**, by test (`a_drag_across_code_lines_copies_them`). One trade-off: a selection whose end scrolls off screen is dropped, as in any `show_rows` list |

## Follow-up (2026-09-07)

Three more things about the pane, after the two steps:

- **The end of the code was cut off.** Not the pane's doing: a `leaf_fill`
  reports the whole root height as its content size, and the root column was
  `min_h 100%`, so "header rows, then a list that fills the rest" measured
  taller than the window and the root grew to fit. `root_style` now fixes the
  height at 100%; the header keeps its content height and the list gets what
  is left. Two gallery snapshots (list-10k, showcase) changed at their bottom
  edge, where the list used to run past the window, and were updated. The
  patch example still overflows on its own (its centre column measures about
  2165 tall); that is patch's layout, not the pane's, and is left as is.
- **Plumbing hidden.** The pane shows `shown_source`: the `//!` crate doc
  dropped, the `META` block and its import dropped, and anything between
  `// gallery:hide` and `// gallery:show` (list-10k's `Compare` and `PlainApp`,
  the standalone binary's root). Line counts count what is shown. The crate
  doc is not shown anywhere else yet; the summary line above the example is
  the description for now.
- **Highlighting.** `egui_extras`' built-in highlighter split `use_state` at
  the underscores. `src/highlight.rs` is a Rust-only tokenizer in Catppuccin
  colours (Mocha on dark, Latte on light, by the style guide's language
  defaults), and `egui_extras` is no longer a dependency.

## Problem

The gallery draws the selected example's source in the right column with
`egui_extras::syntax_highlighting::code_view_ui`. That costs as much per frame
as the running example itself (list-10k), and up to 80% of the frame for a
long source (patch). It gets in the way of measuring the perf examples
(list-10k) in the gallery.

## Constraints

- The code stays selectable (`Label::selectable(true)`): drag-select and copy
  must keep working. Turning selection off is not an option.
- Highlighting, dark/light theme and the source link stay as they are.
- The gallery tests and snapshots keep passing; the a11y test still finds the
  code as a `Label` in the accesskit tree.

## Steps

1. Cache the galley across frames and draw it with `Label::new(Arc<Galley>)`.
2. Draw the code line by line through `<VirtualList>`, one cached galley per
   line, so only the lines in view are laid out, painted and reported to
   accesskit.

Each step gets its own commit and its own row in
[measurements.md](measurements.md). See [plan.md](plan.md).

## Completion criteria

- With accesskit on (kittest, and the web through `WebA11y`): the code pane
  for `patch` (1661 lines) costs under 0.2 ms per frame, down from 2.25 ms.
- Without accesskit (native): the code pane for `list-10k` costs under
  0.03 ms, down from 0.08 ms, and no longer scales with the source length.
- Text selection and copy still work on the code, in the gallery, by hand.
