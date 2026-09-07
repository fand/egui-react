# Task: gallery code pane cost

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
