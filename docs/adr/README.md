# Decision records

One file per design decision: when it was made, what was decided, what was turned down and why, and what it costs. An ADR is a record of a moment and is not edited to keep up with the code.

`../ARCHITECTURE.md` is the other half. It says what is true now; an ADR says when and why. When they disagree, ARCHITECTURE is wrong about the code and should be fixed; the ADR stays as written.

## Domains

Numbering is per domain, from `0001`, in the order the decisions were made.

| Domain | What it covers |
|---|---|
| `core` | `egui-reactor`: `Cx`, `View`, `rsx!`, hooks, state handles, event props |
| `runtime` | The store, the pass, the sweep, multi-pass, async, `<Suspense>` |
| `layout` | The layout engine over taffy, the lite path, layout attributes |
| `elements` | `egui-reactor-elements`: the wrappers for egui widgets and containers |
| `fonts` | `egui_reactor_app::fonts`, and what text rendering forced on the engine |
| `a11y` | Accessibility: accesskit on native, the gap on web |
| `app` | `egui-reactor-app`: the runner, `Options`, platforms, packaging |

## File format

`docs/adr/<domain>/NNNN-<slug>.md`, and nothing more than this:

```markdown
# NNNN: <title>

Date: YYYY-MM-DD · Status: accepted | superseded by NNNN | supersedes NNNN

## Context
## Decision
## Rejected
## Consequences
## Links
```

Context is 2 to 5 sentences: what forced the choice. Decision is 1 to 3. Rejected lists each alternative with why it lost. Consequences carries the cost, the constraints, and the rules that follow. Links point at the ARCHITECTURE section that describes the current design, the `docs/tasks/` files, and the commits.

Where a decision was made before this directory existed and no date was recorded, the date reads `2026-08 or earlier`.

## Rules

- A new decision gets a new file. An existing file is never rewritten to say something else.
- A changed decision gets a new file that supersedes the old one. The old file keeps its text and gains `superseded by NNNN` in its status line; the new one says `supersedes NNNN`.
- ARCHITECTURE.md is updated to the current state and links the ADR.

## Index

**core**

- [0001: Rust only as the host language](core/0001-rust-only-host-language.md)
- [0002: `rsx!` expands directly, with no retained tree](core/0002-direct-expansion-no-reconciler.md)
- [0003: A hook's Id is its call-site stack, and collisions are reported](core/0003-hook-id-from-call-site.md)
- [0004: `use_state` returns a guard, with `Handle` as the helper](core/0004-state-guard-over-cell-handle.md)
- [0005: `on_*` props are fused into one event enum closure](core/0005-fused-callback-props.md)
- [0006: `use_effect` runs the body in place](core/0006-effects-run-at-the-call-site.md)
- [0007: Hook deps are compared by `Hash`](core/0007-deps-compared-by-hash.md)
- [0008: One `Handler` trait for both arities, told apart by a marker](core/0008-handler-marker-type-argument.md)
- [0009: A value prop and a handler over the same state stay the user's problem](core/0009-value-prop-vs-handler-borrow.md)

**runtime**

- [0001: A multi-pass frame needs no state rollback](runtime/0001-no-multi-pass-rollback.md)
- [0002: The store lives in the runner's `App`, not in egui memory](runtime/0002-store-in-the-runner-app.md)
- [0003: One thread and `pollster` as the native executor](runtime/0003-thread-plus-pollster-executor.md)
- [0004: `use_future` returns `std::task::Poll<T>`](runtime/0004-async-results-as-poll.md)
- [0005: `<Suspense>` draws children offscreen and counts pending futures](runtime/0005-suspense-offscreen-and-counter.md)

**layout**

- [0001: Layout runs on taffy, not egui_flex](layout/0001-taffy-over-egui-flex.md)
- [0002: Our own layout engine over taffy, replacing egui_taffy](layout/0002-own-engine-over-taffy.md)

**elements**

- none yet

**fonts**

- [0001: CSS-style font chains resolved through fontdb](fonts/0001-css-font-chains-through-fontdb.md)
- [0002: `<Text font>` takes the name of a stack, as a string](fonts/0002-text-font-takes-a-stack-name.md)
- [0003: A bitmap-only face is rejected as `Invalid`](fonts/0003-bitmap-only-fonts-invalid.md)
- [0004: Detect a new atlas by fingerprinting the fonts](fonts/0004-fingerprint-fonts-to-detect-atlas.md) — superseded by 0005
- [0005: A `<Text>` galley lives for one pass](fonts/0005-galley-lives-one-pass.md)
- [0006: Loading policy belongs to the app; the library exposes `pending()`](fonts/0006-loading-policy-belongs-to-the-app.md)
- [0007: egui's embedded fonts sit behind a `default_fonts` feature](fonts/0007-default-fonts-feature.md)

**a11y**

- none yet

**app**

- [0001: The site is a VitePress documentation site with one embed wasm](app/0001-site-is-vitepress-with-one-embed-wasm.md)
- [0002: The embed follows the page's theme through the hash](app/0002-embed-follows-the-page-theme.md)
- [0003: Persisted-state keys keep the pre-rename name](app/0003-storage-keys-keep-the-old-name.md)
