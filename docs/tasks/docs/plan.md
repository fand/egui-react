# Plan: docs

The task definition is in [task.md](task.md). If we decide to deviate from this
during implementation, update this document, and ARCHITECTURE.md too if the
change matters for the design. The tooling decision itself is recorded as
[adr/app/0001](../../adr/app/0001-site-is-vitepress-with-one-embed-wasm.md)
(written as part of this task).

## 0. Overview

One PR. The published site stops being the gallery wasm and becomes a VitePress
site; the gallery's three columns are redistributed: the example list becomes
the sidebar, the code pane becomes the page body, and the running example
becomes a single "embed" wasm that draws one example full-bleed, picked by
`location.hash`. We touch `site/` (new), `examples/gallery` (a second bin),
the three workflows, README, ARCHITECTURE, and one ADR. Core is unchanged.

What was rejected, in one line each (the ADR carries the detail):

- Pages *inside* the egui gallery (egui_commonmark or hand-built): accesskit-web
  covers screen readers and keyboard, but Ctrl+F, search engines, translation
  and first paint stay weak — and Getting Started is the page people reach from
  a search engine.
- mdBook: no Node, but the hero home and the synced code/canvas tabs are all
  hand-written CSS/JS there; in VitePress they are stock features plus one Vue
  component, and Shiki highlights at build time.
- One wasm per example: 17 trunk builds per deploy and no cross-page caching.

## 1. URL and directory layout

Published URLs (GitHub Pages under `/egui-reactor/`, Cloudflare preview under `/`):

| URL | What | Built by |
|---|---|---|
| `/` | Home: hero + live counter next to its code | VitePress |
| `/getting-started` | Install, counter, `cargo run`, `trunk serve`, features | VitePress |
| `/guide/*` | ~9 guide pages (section 4) | VitePress |
| `/reference/*` | Hooks, elements, layout attributes | VitePress |
| `/examples/<name>` | One page per example: canvas + tabs + source | VitePress |
| `/embed/#<name>`, `/embed/#<name>/plain` | The example, full-bleed, nothing else | trunk (`Trunk-embed.toml`) |
| `/api/` | `cargo doc --no-deps` for the four public crates | cargo |

In the repository:

```
site/
  package.json, package-lock.json      Node stays contained in site/
  .vitepress/config.ts                 nav, sidebar, base, local search, markdown-it plugin
  .vitepress/theme/                    default theme + ExampleEmbed.vue + a small CSS file
  lib/example-source.js                the shown-source rules, ported (section 3)
  index.md, getting-started.md, guide/*.md, reference/*.md, examples/*.md
  build.sh                             VitePress + embed + cargo doc into one dist (section 5)
examples/gallery/
  src/embed.rs, embed.html, Trunk-embed.toml
```

`.gitignore` gains `node_modules/`, `site/.vitepress/dist/`,
`site/.vitepress/cache/`.

## 2. The embed wasm (`examples/gallery`, bin `embed`)

- A second `[[bin]]` in the gallery crate, so it reuses the gallery's example
  imports and `EXAMPLES` and adds no workspace member. `Running` becomes `pub`
  (it already has the `match` from name to `App`); `find` already is.
- `embed.rs`: parse `location.hash` into `(name, plain)` — `#todo`,
  `#counter/plain`; unknown or missing falls back to `EXAMPLES[0]`, a `plain`
  request for an example without one falls back to the egui-reactor version (the
  gallery's own rule). The root closure re-reads the hash every pass and mounts
  `<Running>` under `key={name}` inside a `grow` column, so a hash change
  remounts cleanly. A `hashchange` listener (a `wasm_bindgen` closure leaked
  once at startup) calls `Context::request_repaint`, because egui repaints on
  input and a hash change is not input. No list, no code pane, no header.
- `embed.html`: the gallery's `index.html` minus nothing but the title —
  same canvas CSS, same `data-wasm-opt="z"`, same web-font `copy-file` link and
  the same pre-build hook in `Trunk-embed.toml` (the font example runs in the
  embed too).
- `setup` registers the shader and patch pipelines and `WebA11y`, exactly as
  the gallery bin does.
- The gallery bin, `cargo run -p gallery <name>`, and the snapshot / bench /
  a11y tests are untouched.

## 3. Getting the example source onto the pages

The pages must show the same "source minus plumbing" the gallery shows, from
the same files, or the docs drift. `gallery::shown_source` (drop the `//!`
header, the `META` block and its import, `// gallery:hide`..`show` regions,
fold blank runs) is ~30 lines; port it to `site/lib/example-source.js` and use
it from a markdown-it container registered in `.vitepress/config.ts`:

```markdown
::: example-source counter
:::
```

expands at build time to a fenced ` ```rust ` block of
`shown_source(examples/counter/src/lib.rs)`, which Shiki then highlights like
any other fence; `::: example-source counter plain` does the same for
`plain.rs`. The module also exports the line counts, which the example pages
pass to `ExampleEmbed` for the tab labels.

Rejected: VitePress `<<<` snippet imports (their `#region` markers cannot skip
the `META` block in the middle of a file, and we would be adding markers to
every example), and `?raw` imports rendered client-side (loses build-time
Shiki). A parity test between the JS port and the Rust original is not worth
the harness; the port is small and the gallery test in section 6 keeps the
page list honest.

`ExampleEmbed.vue` (theme component):

- Props: `name`, `hasPlain`, `reactLines`, `plainLines`.
- Renders the tab row (labels `egui-reactor · N lines` / `plain egui · M lines`,
  hidden when `hasPlain` is false), the iframe (`src` built with `withBase` on
  `/embed/`), and a default slot the page puts the code fences in; the tabs
  toggle a class that shows one fence and hides the other, and switch the
  iframe by assigning `src` with the new hash (a fresh assignment so the wasm
  restarts; a same-document hash change would need no reload, but a reload is
  simpler than proving both examples' state can coexist, and the wasm is cached).
- The iframe gets a fixed aspect-ratio box, `title="running example: <name>"`,
  and `loading="lazy"` so 17 pages do not each boot wasm on prefetch.

## 4. Pages

All English. Sources for the content are named per page; writing is mostly
lifting and reshaping, not new material.

- **Home** (`index.md`): VitePress home layout. Hero: name, tagline ("Write
  egui apps the way you write React" or similar), actions Get Started /
  Examples / GitHub. Below: the counter code next to the live embed
  (`ExampleEmbed` + `::: example-source counter`), then four feature cards (no
  reconciler and `&mut` handlers; hooks; flexbox/grid first-class; native +
  wasm). From the README intro.
- **Getting Started**: git dependency `Cargo.toml`, the counter walked through,
  `cargo run`, web with trunk (`index.html`, `canvas_id`), the `default_fonts`
  and `woff2` features. From README and `egui-reactor-app`.
- **Guide** — Thinking in egui-reactor (no reconciler, handlers borrow with
  `&mut`, differences from React: no `memo`/`useCallback`, effects run in
  place, one-frame delay) · `rsx!` (elements, attributes, `{expr}`,
  `if`/`for`/`match`, `key`) · Components and events (`#[component]`, props,
  `#[event]`, `shares_ui`) · State and hooks (`use_state`, `bind`,
  `update_later`, `Handle`, `Dispatch`, the two borrow pitfalls) · Layout
  (`<View>`, flex/grid, paint, the `ScrollArea` height and wrap traps) · Async
  (`use_future`, `<Suspense>`, `spawn`) · Escape hatches (`cx.leaf`,
  `view(|cx| ..)`, plain egui inside) · Fonts (chains, `<Text font>`, web
  fonts) · Web and native (`Options`, `canvas_id`, `setup` + wgpu). From
  ARCHITECTURE §2–§6 and §8, rewritten for users (ARCHITECTURE explains the
  implementation; the guide tells the reader what to type).
- **Reference** — Hooks (§4's table, one row per hook, expanded a sentence
  each) · Elements (the five families from §6's list; per element a props table,
  a short snippet, and links to the examples whose `META.elements` name it) ·
  Layout attributes (`ItemStyle` / `ContainerStyle` / `PaintStyle` from §6).
- **Examples**: 17 pages, each generated by hand from `META` (summary line,
  `ExampleEmbed`, the source container(s), a "run it yourself" footer with the
  `cargo run` / `trunk serve` lines). `shell` gets code only and a note that it
  docks panels into the window and cannot be embedded.
- **Architecture**: a short page linking `docs/ARCHITECTURE.md` and
  `docs/adr/` on GitHub, plus `/api/` — these stay in the repository, the page
  just says where they are.
- Nav bar: Guide, Reference, Examples, API (external to `/api/`), GitHub.
  Local search (`provider: 'local'`) on.

## 5. Build, deploy, redirects

`site/build.sh` is the one place the assembly lives; workflows and local runs
call it. With `SITE_BASE` defaulting to `/`:

1. `npm ci && npx vitepress build` (base from `SITE_BASE`) → `.vitepress/dist`.
2. `trunk build --release --public-url ${SITE_BASE}embed/ --config
   examples/gallery/Trunk-embed.toml` → copied to `dist/embed/`.
3. `cargo doc --no-deps -p egui-reactor -p egui-reactor-elements -p egui-reactor-app
   -p egui-reactor-macros` → copied to `dist/api/`.

- `pages.yml`: add a Node setup step (`actions/setup-node`, npm cache), run
  `SITE_BASE=/egui-reactor/ site/build.sh`, upload `site/.vitepress/dist`. The
  system-deps and Rust steps stay (cargo doc and trunk want them).
- `preview-cloudflare-pages.yml`: same with `SITE_BASE=/`, deploy the same dist.
- `ci.yml` wasm job: the trunk loop's glob is `examples/*/Trunk.toml`, so add
  the embed config explicitly; extend the wasm-opt sed to cover `embed.html`;
  add Node setup + `npm ci` + `npx vitepress build` (fast, and it is what
  catches a broken include path or a dead page reference on PRs). cargo doc is
  not run in ci.yml; a doc warning is not worth a third toolchain step there.
- Redirects: an inline script in the VitePress head (config `head` entry, so it
  runs before hydration): if `location.pathname` is the site root and
  `location.hash` matches `#<name>` (optionally `/plain`) for a known example
  name, `location.replace` to `examples/<name>.html`. The name list is baked in
  by the config from the same module that walks `site/examples/`. README's
  gallery table links change to `/examples/<name>`, and the README intro gains
  the docs URL.

## 6. Tests and checks

- `examples/gallery/tests/site.rs`: for every `Meta` in `EXAMPLES`,
  `site/examples/<name>.md` exists (via `CARGO_MANIFEST_DIR/../../site`), plus
  `shell.md` — so adding an example without its page fails CI, in the same
  place adding it to the gallery is enforced today.
- The existing gallery tests, snapshot set and bench are untouched and must
  stay green.
- Visual checks before merge (Cloudflare preview): home hero + counter, one
  example page per family (board for plain tabs, shader for wgpu, font for the
  web font in the embed), search, dark/light, the `#counter` redirect, `/api/`.

## 7. Order of work

1. `embed` bin + `Trunk-embed.toml` + `embed.html`; `Running` made `pub`;
   confirm with `trunk serve` that `#board` and `#board/plain` run.
2. `site/` skeleton: VitePress init, config, theme, `example-source` plugin,
   `ExampleEmbed.vue`; Home and the counter example page as the proof.
3. The remaining pages: Getting Started, guide, reference, examples.
4. `build.sh`, the three workflows, redirects, README.
5. The site test, ARCHITECTURE's Website section, the ADR and its index entries.

## 8. Differences found during implementation

Steps 1 to 4:

- **`embed.rs`'s root reads the hash one level in.** `rsx!` expands to a
  non-`move` closure, so reading the hash beside it made the returned view
  borrow the root closure's frame (E0373). The root is
  `view(|cx| { let (name, plain) = requested(); rsx!{..}.show(cx); })`, which
  keeps the borrow inside the pass and still re-reads every pass.
- **Home's feature cards are markdown, not frontmatter.** VitePress's home
  layout renders the page body *after* `features`, and section 4 wants the live
  counter above the cards, so the four cards are HTML in `index.md` with a
  small grid in the theme's CSS. The hero stays stock frontmatter.
- **Line counts reach the pages through a data loader.** `site/examples.data.js`
  (a VitePress loader, so it runs in Node at build time) exports
  `hasPlain` / `reactLines` / `plainLines` per example; the pages pass them to
  `ExampleEmbed`. Hardcoding the numbers in markdown would drift.
- **`::: example-source` is a markdown-it block rule, not a container.** A
  container's `render` emits raw HTML and would bypass Shiki; the rule pushes a
  `fence` token instead, so the block is highlighted at build time and gets the
  copy button like any other fence. It also keeps the dependency list to
  vitepress + vue.
- **The code panes are shown and hidden from global CSS.** The page owns the
  fences and passes them through `ExampleEmbed`'s slot, so a `<style scoped>`
  cannot reach them; the component toggles a class and
  `.vitepress/theme/custom.css` hides the version that is not selected.
- **`/api/` needs an index of its own.** `cargo doc` writes one directory per
  crate and no root `index.html`, so `site/build.sh` writes a redirect to
  `egui_reactor/index.html`.
- **`Trunk-embed.toml`'s output needs its own ignore entry.** `.gitignore`'s
  `dist/` matches only directories named exactly `dist`, so `dist-embed/` was
  added.
- **The site build prints a chunk-size warning.** The example sources are
  inlined into their page chunks (board alone is 826 lines), so rollup warns
  about chunks over 500 kB. It is a warning, not an error; revisit if the
  deployed size starts to matter.
