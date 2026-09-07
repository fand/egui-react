#!/bin/sh
# Downloads the web font the `font` example's `web` stack fetches at run time
# (`Url("fonts/NotoSansJP-Regular.ttf")`, served next to `index.html`).
#
# Run by the `pre_build` hooks in `examples/font/Trunk.toml` and
# `examples/gallery/Trunk.toml`. The file is 9.6 MB (the variable font, all
# weights; its default instance is Thin), so it is listed in `.gitignore`
# rather than committed, and only downloaded when it is missing. A failed
# download leaves no partial file behind, so the next build tries again.
set -e

dir=$(cd "$(dirname "$0")" && pwd)
out="$dir/NotoSansJP-Regular.ttf"
url='https://raw.githubusercontent.com/google/fonts/main/ofl/notosansjp/NotoSansJP%5Bwght%5D.ttf'

if [ -f "$out" ]; then
  exit 0
fi

echo "font example: downloading $url"
curl -fsSL -o "$out" "$url" || {
  rm -f "$out"
  echo "font example: download failed" >&2
  exit 1
}
