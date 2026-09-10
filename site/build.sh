#!/usr/bin/env bash
#
# Build the whole published site into `site/.vitepress/dist`.
#
# Called by .github/workflows/pages.yml (SITE_BASE=/egui-react/), by
# .github/workflows/preview-cloudflare-pages.yml (SITE_BASE=/), and by anyone
# who wants to see locally what will be deployed. It is the one place the
# assembly lives, so the two deploys cannot drift apart.
#
# What lands where:
#
#   .vitepress/dist/          VitePress: home, guide, reference, example pages
#   .vitepress/dist/embed/    the gallery's `embed` bin, built by trunk — the
#                             wasm every example page runs in an iframe
#   .vitepress/dist/api/      `cargo doc --no-deps` for the four public crates
#
# Requirements on PATH: node and npm, trunk, cargo with the
# wasm32-unknown-unknown target. Nothing here installs a toolchain; the
# workflows do that with the actions they already use.

set -euo pipefail

# Everything runs from the repository root, whatever the caller's directory is.
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

SITE="$ROOT/site"
DIST="$SITE/.vitepress/dist"

# Where the site is served from. GitHub Pages puts it under a subdirectory,
# the Cloudflare preview at the root. VitePress wants both slashes, and so
# does trunk's `--public-url`.
SITE_BASE="${SITE_BASE:-/}"
case "$SITE_BASE" in
  # `/` is both slashes at once; anything else needs one at each end.
  / | /*/) ;;
  *)
    echo "build.sh: SITE_BASE must start and end with '/' (got '$SITE_BASE')" >&2
    exit 1
    ;;
esac

step() {
  echo
  echo "==> $*"
}

step "1/4 npm ci (site/)"
npm --prefix "$SITE" ci

step "2/4 vitepress build (base $SITE_BASE)"
(cd "$SITE" && SITE_BASE="$SITE_BASE" npx vitepress build)

# The example pages load this in an iframe as `<base>embed/#<name>`, so the
# generated script and .wasm URLs need the same prefix.
step "3/4 trunk build: the embed wasm -> ${SITE_BASE}embed/"
trunk build --release \
  --public-url "${SITE_BASE}embed/" \
  --config "$ROOT/examples/gallery/Trunk-embed.toml"
rm -rf "$DIST/embed"
mkdir -p "$DIST/embed"
cp -R "$ROOT/examples/gallery/dist-embed/." "$DIST/embed/"

step "4/4 cargo doc: the four public crates -> ${SITE_BASE}api/"
cargo doc --no-deps \
  -p egui-reactor \
  -p egui-reactor-elements \
  -p egui-reactor-app \
  -p egui-reactor-macros
rm -rf "$DIST/api"
mkdir -p "$DIST/api"
cp -R "$ROOT/target/doc/." "$DIST/api/"

# rustdoc writes one directory per crate and no index of its own, so `/api/`
# would be a 404 (or a directory listing). Send it to the core crate, which is
# where a reader starting from the nav bar wants to be.
if [ ! -f "$DIST/api/index.html" ]; then
  cat > "$DIST/api/index.html" <<'HTML'
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <title>egui-reactor API documentation</title>
    <meta http-equiv="refresh" content="0; url=egui_reactor/index.html" />
    <link rel="canonical" href="egui_reactor/index.html" />
  </head>
  <body>
    <p><a href="egui_reactor/index.html">egui-reactor API documentation</a></p>
  </body>
</html>
HTML
fi

echo
echo "==> done: $DIST"
