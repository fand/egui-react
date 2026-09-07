#!/bin/sh
# Downloads the web font the `font` example's `web` stack fetches at run time
# (`Url("fonts/NotoSansJP-Regular.otf")`, served next to `index.html`).
#
# Run by the `pre_build` hooks in `examples/font/Trunk.toml` and
# `examples/gallery/Trunk.toml`. The file is the static Regular OTF from
# notofonts/noto-cjk (4.5 MB; the variable font from google/fonts would draw
# as its Thin default instance), so it is listed in `.gitignore`
# rather than committed, and only downloaded when it is missing. A failed
# download leaves no partial file behind, so the next build tries again.
set -e

dir=$(cd "$(dirname "$0")" && pwd)
out="$dir/NotoSansJP-Regular.otf"
url='https://raw.githubusercontent.com/notofonts/noto-cjk/main/Sans/SubsetOTF/JP/NotoSansJP-Regular.otf'

if [ -f "$out" ]; then
  exit 0
fi

echo "font example: downloading $url"
curl -fsSL -o "$out" "$url" || {
  rm -f "$out"
  echo "font example: download failed" >&2
  exit 1
}
