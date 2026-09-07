# Measurements: gallery code pane

`cargo test --release -p gallery --test bench -- --ignored --nocapture`,
1280x1000, `Harness::step` CPU time per frame in ms, 30 frames after 5 warm-up.
"full" is the gallery, the other columns are one pane alone with the other
two as empty boxes of the same size.

## Baseline (61e1137, `code_view_ui`)

With accesskit (kittest, like the web through `WebA11y`):

| example | lines | full | code | running | list |
|---|---|---|---|---|---|
| counter | 32 | 0.18 | 0.04 | 0.01 | 0.10 |
| list-10k | 177 | 0.54 | 0.19 | 0.24 | 0.10 |
| showcase | 341 | 0.48 | 0.35 | 0.02 | 0.10 |
| board | 894 | 1.39 | 1.10 | 0.17 | 0.10 |
| patch | 1661 | 2.81 | 2.25 | 0.22 | 0.10 |

Without accesskit (bare `Context`, like native):

| example | lines | full | code | running | list |
|---|---|---|---|---|---|
| counter | 32 | 0.11 | 0.02 | 0.01 | 0.08 |
| list-10k | 177 | 0.36 | 0.08 | 0.15 | 0.08 |
| showcase | 341 | 0.27 | 0.12 | 0.02 | 0.08 |
| board | 894 | 0.62 | 0.39 | 0.25 | 0.08 |
| patch | 1661 | 0.96 | 0.67 | 0.13 | 0.08 |

Code pane variants, `patch`:

| variant | accesskit | native |
|---|---|---|
| `code_view_ui` | 2.29 | 0.65 |
| highlight + Label `selectable(false)` | 0.60 | 0.70 |
| kept galley + Label selectable | 1.53 | 0.06 |
| kept galley + Label `selectable(false)` | 0.01 | 0.06 |
| plain monospace Label selectable | 1.62 | 0.07 |
| plain monospace Label, click sense, not selectable | 0.02 | 0.08 |
| `highlight()` only | 0.18 | 0.19 |
| empty pane | 0.01 | 0.00 |

Same, `list-10k`:

| variant | accesskit | native |
|---|---|---|
| `code_view_ui` | 0.21 | 0.07 |
| kept galley + Label selectable | 0.14 | 0.02 |
| kept galley + Label `selectable(false)` | 0.01 | 0.02 |

## After step 1 (one galley kept across frames, `Label::new(Arc<Galley>)`)

"code" is now the gallery's own `Code` component (made `pub` for the bench);
the baseline's "code" column was a copy of it built around `code_view_ui`.

With accesskit:

| example | lines | full | code | running | list |
|---|---|---|---|---|---|
| counter | 32 | 0.14 | 0.04 | 0.01 | 0.09 |
| list-10k | 177 | 0.58 | 0.16 | 0.27 | 0.09 |
| showcase | 341 | 0.43 | 0.25 | 0.02 | 0.09 |
| board | 894 | 1.21 | 0.79 | 0.20 | 0.09 |
| patch | 1661 | 2.08 | 1.56 | 0.20 | 0.09 |

Without accesskit:

| example | lines | full | code | running | list |
|---|---|---|---|---|---|
| counter | 32 | 0.09 | 0.02 | 0.01 | 0.08 |
| list-10k | 177 | 0.27 | 0.02 | 0.15 | 0.08 |
| showcase | 341 | 0.12 | 0.02 | 0.02 | 0.08 |
| board | 894 | 0.29 | 0.07 | 0.15 | 0.08 |
| patch | 1661 | 0.32 | 0.07 | 0.14 | 0.08 |

Native: the two hashes are gone and the pane no longer scales with the
source (patch 0.67 → 0.07). With accesskit the `TextRun` rebuild of every
row is what is left (patch 2.25 → 1.56); step 2 is for that.

## After step 2 (one galley per line, drawn through `<VirtualList>`)

With accesskit:

| example | lines | full | code | running | list |
|---|---|---|---|---|---|
| counter | 32 | 0.21 | 0.04 | 0.01 | 0.09 |
| list-10k | 177 | 0.39 | 0.07 | 0.23 | 0.09 |
| showcase | 341 | 0.18 | 0.07 | 0.02 | 0.09 |
| board | 894 | 0.35 | 0.09 | 0.17 | 0.09 |
| patch | 1661 | 0.40 | 0.09 | 0.21 | 0.09 |

Without accesskit:

| example | lines | full | code | running | list |
|---|---|---|---|---|---|
| counter | 32 | 0.11 | 0.03 | 0.01 | 0.07 |
| list-10k | 177 | 0.27 | 0.04 | 0.15 | 0.07 |
| showcase | 341 | 0.12 | 0.04 | 0.01 | 0.07 |
| board | 894 | 0.26 | 0.07 | 0.13 | 0.07 |
| patch | 1661 | 0.27 | 0.05 | 0.13 | 0.07 |

The pane costs the same whatever the source length: about 60 rows are on
screen, and only those are laid out, painted and reported to accesskit.
With accesskit, patch 1.56 → 0.09; the gallery frame for patch 2.81 → 0.40
over the two steps. Native is flat at 0.04–0.07 for every example.

What is left, native, list-10k: the list with rows that draw nothing costs
0.01; the other 0.03–0.04 is sixty selectable `Label`s (an interact and a
text-selection pass each), which is what egui's own `show_rows` of labels
costs too. Drawing the label straight into the row `Ui` instead of through
`cx.leaf` measured the same (0.05 both), so the row stays a leaf.

Selection: a drag from one line to the next selects across labels and copy
joins them (`tests/gallery.rs`, `a_drag_across_code_lines_copies_them`). A
selection whose end scrolls off screen is dropped by egui, as with any
`show_rows` list of labels.

## After the follow-up (own highlighter, trimmed source, sideways `<VirtualList>`)

Same protocol. The code pane is the same cost as after step 2; the row count
in view is what it costs, and that did not change.

| example | lines shown | full (a11y) | code (a11y) | full (native) | code (native) |
|---|---|---|---|---|---|
| list-10k | 104 | 0.40 | 0.07 | 0.26 | 0.05 |
| patch | 1578 | 0.42 | 0.08 | 0.27 | 0.05 |

Tried and dropped: a `<View>` per row to pin the row height (a bare leaf takes
the height of what it draws, and a blank line's galley had none) measured
twice the pane cost (0.10 native, 0.16 with accesskit). The blank line gets
its height from the job's `first_row_min_height` instead, at no cost.
