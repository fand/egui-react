#!/usr/bin/env bash
#
# The site with both halves live: `vitepress dev` for the pages (re-renders a
# page when its markdown or the example it includes changes) and `trunk serve`
# for the embed wasm (rebuilds when any Rust in the workspace changes, then
# reloads the iframe). The pages find the embed through VITE_EMBED_ORIGIN.
#
#   npm run dev:all        (from site/)
#
# Pages: http://localhost:5173   Embed alone: http://localhost:8080/#counter
# TRUNK_ARGS adds flags to trunk, e.g. TRUNK_ARGS=--release for the optimised
# wasm (slower to build, quicker to load).

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EMBED_PORT="${EMBED_PORT:-8080}"

trunk serve \
  --config "$ROOT/examples/gallery/Trunk-embed.toml" \
  --public-url / \
  --port "$EMBED_PORT" \
  --watch "$ROOT/crates" \
  --watch "$ROOT/examples" \
  --ignore "$ROOT/examples/gallery/dist" \
  --ignore "$ROOT/examples/gallery/dist-embed" \
  ${TRUNK_ARGS:-} &
TRUNK_PID=$!
trap 'kill "$TRUNK_PID" 2>/dev/null' EXIT

cd "$ROOT/site"
VITE_EMBED_ORIGIN="http://localhost:$EMBED_PORT/" npx vitepress dev
