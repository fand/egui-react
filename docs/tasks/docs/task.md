# Task: docs (website: docs site + landing, gallery becomes the docs)

## Goal

Turn https://fand.github.io/egui-react/ from "the gallery wasm and nothing else" into a documentation site with a landing page. The site is a VitePress site: a home page in the TanStack style (name + tagline + a live counter next to its code), Getting Started, a guide, reference pages (hooks, elements, layout attributes), and one page per example. The current gallery's three-column UI is absorbed by the docs: the example list becomes the sidebar, the code pane becomes the page body (with an egui-react / plain egui tab where a plain version exists), and the running example becomes an embedded wasm that does nothing but run one example full-bleed.

Core (`egui-react`, `egui-react-macros`) is not touched, except for nothing at all: the only Rust changes are in `examples/gallery` (a second bin) and one-line source markers in the example crates.

## Scope

One PR, everything below.

### In scope

- `site/`: the VitePress site. Home, Getting Started, guide (~9 pages), reference (hooks / elements / layout attributes), one page per example (17), an Architecture pointer page. English.
- `examples/gallery`: a new `embed` bin + `embed.html` + `Trunk-embed.toml`. It reads `location.hash` (`#<name>` or `#<name>/plain`) and draws that example filling the canvas. The existing `gallery` bin, its tests and `cargo run -p gallery` stay as they are.
- An `ExampleEmbed` Vue component: the iframe on `/embed/`, the egui-react / plain egui tabs with line counts, used by Home and the example pages.
- Deploy: `pages.yml` and `preview-cloudflare-pages.yml` build VitePress + the embed wasm + `cargo doc` into one dist. `ci.yml` builds all three without deploying.
- Redirects: the old `/#counter` deep links land on `/examples/counter`. README links updated.
- `docs/ARCHITECTURE.md`: a short Website section. `docs/adr/app/0001`: the decision (VitePress + one embed wasm, and what was rejected).

### Out of scope

- crates.io release and docs.rs (the Getting Started uses a git dependency until then).
- Translations, a blog, versioned docs.
- Splitting the embed into one wasm per example (revisit if the single wasm is too heavy; it is the same size as today's gallery minus the gallery UI).
- Removing the native gallery UI or its tests. It stays the native dev tool.
- Design tuning beyond VitePress's default theme plus a hero and the embed component.

## Deliverables

- `site/` (VitePress config, theme additions, all pages), `site/package.json` + lockfile.
- `examples/gallery/src/embed.rs`, `embed.html`, `Trunk-embed.toml`.
- Workflow updates, README updates, ARCHITECTURE section, `docs/adr/app/0001-site-is-vitepress-with-one-embed-wasm.md`.
- A gallery test that every `META` in `EXAMPLES` has a matching `site/examples/<name>.md`.

## Done criteria

- `npm run build` in `site/` and `trunk build --config examples/gallery/Trunk-embed.toml` succeed locally and in CI.
- The deployed site: Home shows the hero and a running counter next to its code; every example page (except shell) runs its example and shows its source; the plain egui tab switches both the code and the canvas; search finds guide text.
- Old links `https://fand.github.io/egui-react/#<name>` land on the matching example page.
- `cargo run -p gallery todo` and every existing test still work unchanged.
- CI (fmt / clippy / test / wasm check / trunk loop / site build) is green.

## Decisions (assumptions at the start)

- **The docs absorb the gallery, not the other way round** (option B). Writing Home and Getting Started inside the egui gallery (option A) was considered and rejected: accesskit-web covers screen readers and keyboard, but Ctrl+F, search engines, translation and first paint stay weak (`docs/tasks/a11y/plan.md` 1.5 records the same conclusion for Flutter), and Getting Started is exactly the page people reach from a search engine.
- **VitePress over mdBook.** The two pages we care most about — a hero home and example pages with synced code/canvas tabs — are custom CSS/JS in mdBook and stock features plus one Vue component in VitePress; Shiki highlights at build time; the cost is Node in the repo, contained in `site/`. mdBook's win (no Node, sub-second builds) does not outweigh that here. The Markdown body would survive a later move either way.
- **One embed wasm for all examples**, chosen by hash, so the browser caches one file across pages instead of fetching 17 builds; CI builds it once.
- **The example sources stay the single source of truth**: pages include them from `examples/*/src/` at build time rather than pasting copies.
