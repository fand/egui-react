---
title: Architecture
---

# Architecture

<script setup>
// `withBase`, because the site is served from `/egui-react/` on GitHub Pages
// and from `/` on the preview; `/api/` is cargo doc's output, not a page of
// this site, so it is a plain link and not a router route.
import { withBase } from 'vitepress'
</script>

This site is the documentation. The design notes stay in the repository, where
they sit next to the code they describe and are reviewed with it. This page
says where they are.

## `docs/ARCHITECTURE.md`

[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md)
is the deep reference: what is true about the implementation right now. Goals
and non-goals, `Cx` and the `View` trait, what `#[component]` and `rsx!`
generate, the hook list with its exact semantics, the runtime (store, sweep,
multi-pass, repaint policy, `<Suspense>`), the layout engine over taffy
including the lite path for list rows, the element list, the crate layout, the
platform notes, and the testing strategy.

The guide on this site is written for someone building an app; ARCHITECTURE is
written for someone changing the library. When a guide page summarizes
something, it links the section there rather than repeating it.

## `docs/adr/`

[docs/adr/](https://github.com/fand/egui-react/tree/main/docs/adr) is the
decision log: one file per design decision, grouped by domain — `core`,
`runtime`, `layout`, `elements`, `fonts`, `a11y`, `app`. Each records when the
decision was made, what was decided, what was turned down and why, and what it
costs.

The discipline is deliberately narrow, and it is what makes the log worth
reading:

- An ADR is a record of a moment. It is never rewritten to say something else.
- A new decision gets a new file. A *changed* decision gets a new file that
  supersedes the old one; the old file keeps its text and gains "superseded by"
  in its status line.
- ARCHITECTURE.md says what is true now and links the ADR. When the two
  disagree, ARCHITECTURE is the one that is wrong about the code.

So if you want to know why `use_state` hands back a guard rather than a
`(value, setter)` pair, why hook deps are hashed instead of compared, why
`on_*` props are fused into one closure, or why layout runs on taffy rather
than egui_flex — the answer is a short file with a date on it, and it has not
been edited since.

## API documentation

<a :href="withBase('/api/')" target="_blank" rel="noreferrer">/api/</a> is `cargo doc` for
the four public crates: `egui-react`, `egui-react-elements`,
`egui-react-app` and `egui-react-macros`. It is generated into this site by the
same build, so it always matches the commit the site was built from. It is the
right place for exact signatures; this site is the right place for how the
pieces fit together.

## The repository

[github.com/fand/egui-react](https://github.com/fand/egui-react). Issues and
pull requests welcome; the tests to run before opening one are in the README.
