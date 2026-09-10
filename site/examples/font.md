---
title: font
---

<script setup>
import { data } from '../examples.data.js'
</script>

<ExamplePage
  name="font"
  :has-plain="data.font.hasPlain"
  :react-lines="data.font.reactLines"
  :plain-lines="data.font.plainLines"
>

<template v-slot:summary>

CSS-style font chains: bundled, fetched and installed fonts, and what each entry
resolved to.

</template>

<template v-slot:code>

<div data-version="react">

::: example-source font
:::

</div>

</template>

egui draws text from font bytes it was handed: the browser's fonts and the OS's
font matching are never involved, and egui's own four fonts have no CJK glyphs,
so Japanese draws as boxes in a plain egui app. This example is the answer —
four named chains, one per kind of source. `bundled` is a 433 KB subset
compiled in with `include_bytes!`; `web` fetches the full 4.5 MB Noto Sans JP
from the app's own origin; `system` asks the machine for its installed fonts;
and `code` is the monospace chain, with the subset behind egui's Hack for the
kana and kanji Hack does not have.

The report table under the samples is the point of it. For every entry of every
chain it says which face it resolved to — with the family name that face
declares, which is how you find the spelling a `System` entry needs — or why it
did not. Pick the `system` stack natively and you can see fontconfig's answer;
pick it in a browser and you get the Local Font Access route, which is Chromium
only, needs a permission prompt, and must be triggered by a click, so every
other browser shows the button disabled with the reason.

**The embed above fetches a 4.5 MB font on demand.** Choosing the `web` stack
starts that download; until it lands, the bundled subset behind it in the chain
draws the text, and the entry flips from pending to loaded when the bytes
arrive. That is CSS's `swap`, and the `font-display` row switches it for
`block`, which draws a placeholder until `Fonts::pending()` goes false. The
library only reports whether a URL is still in flight; which policy an app wants
is the app's call. [Fonts](/guide/fonts) has the rest.

## Run it yourself

```sh
cargo run -p font
trunk serve --config examples/font/Trunk.toml
```

The web font is not committed: a pre-build hook in the example's `Trunk.toml`
downloads it the first time trunk builds. Natively the same relative URL has no
server to point at, so that entry is reported as failed and the chain draws with
the subset — the report shows that too.

The source is [`examples/font/src/lib.rs`](https://github.com/fand/egui-reactor/blob/main/examples/font/src/lib.rs).

</ExamplePage>
