# 0001: The site is a VitePress documentation site with one embed wasm

Date: 2026-09-09 · Status: accepted

## Context

`https://fand.github.io/egui-react/` was the gallery wasm and nothing else: a canvas with an example list, a running example and its source. There was no landing page, no getting started, no guide and no reference — and no way to add one, because everything a reader might search for was pixels on a canvas. A page drawn inside the canvas cannot be found by Ctrl+F, indexed by a search engine, translated by the browser, or shown before the wasm has loaded, and Getting Started is exactly the page people arrive at from a search engine. The task is `docs/tasks/docs/`.

## Decision

The published site is a VitePress site in `site/`, and it absorbs the gallery's three columns: the example list becomes the sidebar, the code pane becomes the page body (with an egui-react / plain egui tab where a plain version exists), and the running example becomes an iframe. Behind that iframe is **one** wasm — a second `embed` bin in the gallery crate that draws a single example full-bleed, picked by `location.hash` (`/embed/#todo`, `/embed/#counter/plain`). `site/build.sh` assembles the three parts into one directory: VitePress into `dist/`, the embed into `dist/embed/`, `cargo doc --no-deps` into `dist/api/`. The example sources stay the single source of truth: pages include them from `examples/*/src/` at build time.

## Rejected

- **Documentation pages inside the egui gallery** (`egui_commonmark`, or hand-built). `accesskit-web` (this repository's own crate) already mirrors the canvas into the DOM, so screen readers and keyboard navigation work — but find-in-page, search engines, translation and first paint do not, and no adapter fixes those: `docs/tasks/a11y/plan.md` 1.4 records the same conclusion for Flutter web, where Ctrl+F has been open since 2020 because lazy rendering means the engine only knows the part of the UI on screen. Documentation is text people search.
- **mdBook.** No Node, sub-second builds. But the two pages we care most about — a hero home and example pages with synced code/canvas tabs — are hand-written CSS and JS there, while in VitePress they are stock features plus one Vue component, with Shiki highlighting at build time. The Node cost is accepted and contained in `site/`; the Markdown body would survive a later move either way.
- **One wasm per example.** 17 trunk builds per deploy, and no caching across pages: a reader walking the examples would download a fresh multi-megabyte wasm per page. One build, cached once, serves all of them.
- **VitePress `<<<` snippet includes and `?raw` imports** for the example sources. `#region` markers cannot skip the `META` block in the middle of a file (and would mean adding markers to every example), and `?raw` renders client-side, which loses build-time Shiki. A markdown-it block rule that emits a `fence` token does both.

## Consequences

- Node and a lockfile live in the repository, confined to `site/`. CI gains a `setup-node` step and a `vitepress build`, which is what catches a dead link or a broken source include on a PR.
- `site/lib/example-source.js` is a port of `gallery::shown_source`. The two must show the same thing or the docs drift from the gallery; the port is ~40 lines and the Rust original keeps its unit test.
- The embed wasm is 16 MB after `wasm-opt -Oz` (every example, wgpu and naga included). That is the number to watch: if it gets in the way, the revisit is splitting it per example, which trades the cross-page cache for smaller pages.
- Old deep links keep working: an inline script in the VitePress head sends `/#<name>` to `/examples/<name>.html` before the router hydrates.
- The native gallery, `cargo run -p gallery <name>`, its tests and its snapshot set are untouched. It stays the development tool; the site is the published one.
- `examples/gallery/tests/site.rs` fails if an example is added to `EXAMPLES` without a page, in the same place adding it to the gallery is enforced today.

## Links

- [ARCHITECTURE section 12, Website](../../ARCHITECTURE.md#12-website)
- `docs/tasks/docs/task.md` and `docs/tasks/docs/plan.md`
- `docs/tasks/a11y/plan.md` 1.4, for what a canvas cannot do
