---
title: Fonts
---

# Fonts

## Why fonts need saying at all

egui draws text itself. epaint rasterizes glyphs from font bytes handed to
`Context::set_fonts`; the browser's fonts and the OS's font matching are never
involved. egui's own four bundled fonts have no CJK glyphs, so Japanese draws
as boxes in a plain egui app on any platform — including one whose system fonts
have every glyph you need, because nothing looked at them.

`egui_react_app::fonts` is the answer: CSS-like fallback chains of named
sources, resolved through one `fontdb` database into egui's per-family fallback
lists.

## A chain

```rust
use egui_react_app::fonts::{FontSource, Fonts, Generic};

const SUBSET: &[u8] = include_bytes!("../fonts/NotoSansJP-Subset.ttf");

pub fn fonts() -> Fonts {
    Fonts::new()
        .stack(
            "ui",
            [
                FontSource::Bundled(SUBSET),
                FontSource::Generic(Generic::SansSerif),
            ],
        )
        .default_proportional("ui")
}
```

Apply it before the first frame, from `setup`:

```rust
let fonts = fonts();
run(
    Options {
        title: String::from("my app"),
        setup: Some(Box::new(move |cc| fonts.apply(&cc.egui_ctx))),
        ..Default::default()
    },
    |_cx| rsx! { <App/> },
)
```

Entries are tried in order, per glyph, the way a CSS `font-family` list is.
`default_proportional(name)` and `default_monospace(name)` make a chain the one
every widget uses; without them a chain is only used where it is named.

## The four sources

| Source | Where the bytes come from |
|---|---|
| `Bundled(&'static [u8])` | `include_bytes!`. Works identically on native and wasm; the cost is binary size |
| `System(String)` | A font installed on the device, by the family name the font declares. Native reads the font directories; on the web this needs the Local Font Access API |
| `Url(String)` | Fetched over HTTP. Pending until the bytes arrive, then the chain is applied again |
| `Generic(Generic::SansSerif)` | A CSS generic — `sans-serif`, `serif`, `monospace`, `cursive`, `fantasy` — resolved by fontdb, with egui's own font behind it |

A `System` name must match what the face declares, in any of its languages:
`"Hiragino Sans"` and `"ヒラギノ角ゴシック"` both work, `"hiragino sans"` does
not. `Fonts::report()` lists, per entry, the face it resolved to and the family
name that face declares — which is how you find the spelling to write.

Which source an app reaches for depends on the target. Natively, `System`:
the machine already has fonts and shipping more is waste. In a browser, `Url`
from your own origin, or `Bundled` when a subset is small enough that there is
nothing to wait for. `System` in a browser is the odd one out: Local Font
Access is Chromium-only, needs a permission prompt, and must be called from a
click.

## Picking a font per text

```rust
rsx! {
    <Text font="ui">"proportional, from the chain above"</Text>
    <Text font="monospace">"egui's built-in monospace"</Text>
}
```

`font` takes the name of a registered stack, or `"proportional"` /
`"monospace"` for egui's two built-in families. An unknown name would make
epaint panic at layout, so it is checked first: the text draws with the style's
own family instead and one warning is logged per name.

Only `<Text>` has the prop. A `<Button>` or `<Checkbox>` label follows
`default_proportional`, or takes a `RichText::new(..).family(..)` as its
children.

## The floor under every chain

egui's four embedded fonts (Ubuntu-Light, Hack, two emoji fonts, about 1.4 MB)
stand behind every generic and at the tail of every chain. They sit behind
`egui-react-app`'s `default_fonts` feature, which is on by default. Turn it off
and nothing panics — but a chain that resolves to nothing is an empty family,
and epaint draws it as zero glyphs. An app that turns it off must give every
stack a `Bundled` (or, natively, `System`) face and apply it before the first
frame, or draw a text-free loading screen until `Fonts::pending()` goes false.

## Web fonts: size and the wait

A web font is a real download, so keep two things in mind.

**Subset it.** The full Noto Sans JP used by the [font](/examples/font) example
is 4.5 MB; the subset it bundles — ASCII, Latin-1, kana, CJK punctuation,
fullwidth forms and about five hundred kanji — is 433 KB. The subset is small
enough to compile in; the full font is not.

**Decide what to draw while it loads.** The library only tells you whether a URL
is still in flight, through `Fonts::pending()`; the policy is yours. Putting a
bundled subset behind the `Url` entry in the same chain gives you CSS's `swap`:
readable text immediately, better text when the bytes land. Drawing a
placeholder until `pending()` is false gives you `block`. Both are shown in the
font example.

```rust
Fonts::new().stack(
    "web",
    [
        FontSource::Url("fonts/NotoSansJP-Regular.otf".into()),
        FontSource::Bundled(SUBSET),
        FontSource::Generic(Generic::SansSerif),
    ],
)
```

A same-origin relative URL is the normal case on the web; a cross-origin one
needs `Access-Control-Allow-Origin`. WOFF and WOFF2 need the `woff2` cargo
feature; without it such a response is reported as failed rather than
panicking. Prefer a static face over a variable font: epaint draws a variable
font's default instance, which is not always the weight you expected.

## See it running

[font](/examples/font) has all four chains, the three sources, the per-entry
report, and the `swap` versus `block` switch.

The resolver's design is in
[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#8-platforms)
section 8, and the decisions behind it in
[docs/adr/fonts/](https://github.com/fand/egui-react/tree/main/docs/adr/fonts).
