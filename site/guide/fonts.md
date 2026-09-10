---
title: Fonts
---

# Fonts

## Why fonts need setup

egui draws text itself, from font bytes handed to `Context::set_fonts`. The
browser's fonts and the OS font matching are never used. egui's four built-in
fonts have no CJK glyphs, so Japanese draws as boxes in a plain egui app, on
every platform.

`egui_react_app::fonts` fixes this with CSS-like fallback chains of named
sources, resolved through `fontdb` into egui's font families.

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

Entries are tried in order, per glyph, like a CSS `font-family` list.
`default_proportional(name)` and `default_monospace(name)` make a chain the
default for every widget. Without them, a chain is used only where named.

## The four sources

| Source | Bytes come from |
|---|---|
| `Bundled(&'static [u8])` | `include_bytes!`. Same on native and wasm. Costs binary size |
| `System(String)` | A font installed on the device, by its declared family name. On the web this needs the Local Font Access API |
| `Url(String)` | Fetched over HTTP. Pending until the bytes arrive, then the chain is applied again |
| `Generic(Generic::SansSerif)` | A CSS generic (`sans-serif`, `serif`, `monospace`, `cursive`, `fantasy`), resolved by fontdb, with egui's own font behind it |

A `System` name must match what the face declares, in any of its languages.
`"Hiragino Sans"` and `"ヒラギノ角ゴシック"` both work. `"hiragino sans"` does
not. `Fonts::report()` lists what each entry resolved to, which is how you
find the right spelling.

Which source to use depends on the target. Natively, `System`: the machine
already has fonts. In a browser, `Url` from your own origin, or `Bundled` when
the subset is small. `System` in a browser is Chromium-only, needs a
permission prompt, and must be called from a click.

## Picking a font per text

```rust
rsx! {
    <Text font="ui">"proportional, from the chain above"</Text>
    <Text font="monospace">"egui's built-in monospace"</Text>
}
```

`font` takes the name of a registered stack, or `"proportional"` /
`"monospace"` for egui's built-in families. An unknown name falls back to the
style's own family and logs one warning.

Only `<Text>` has this prop. A `<Button>` or `<Checkbox>` label uses
`default_proportional`, or takes `RichText::new(..).family(..)` as children.

## The fallback under every chain

egui's four built-in fonts (about 1.4 MB) stand behind every generic and at
the tail of every chain. They are behind the `default_fonts` feature of
`egui-react-app`, on by default. Turn it off and nothing panics, but a chain
that resolves to nothing draws no glyphs. An app that turns it off must give
every stack a `Bundled` or `System` face, or draw a text-free screen until
`Fonts::pending()` goes false.

## Web fonts: size and the wait

A web font is a real download.

**Subset it.** The full Noto Sans JP in the [font](/examples/font) example is
4.5 MB. The subset it bundles (ASCII, Latin-1, kana, CJK punctuation, about
five hundred kanji) is 433 KB. The subset can be compiled in. The full font
cannot.

**Decide what to draw while it loads.** `Fonts::pending()` tells you whether
a URL is still in flight. The policy is yours. A bundled subset behind the
`Url` entry gives you CSS's `swap`: readable text now, better text later. A
placeholder until `pending()` is false gives you `block`.

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

A cross-origin URL needs `Access-Control-Allow-Origin`. WOFF and WOFF2 need
the `woff2` cargo feature. Prefer a static face over a variable font: epaint
draws a variable font's default instance, which may not be the weight you
wanted.

## See it running

[font](/examples/font) has all four chains, the three sources, the per-entry
report, and the `swap` versus `block` switch. The resolver is in
[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#8-platforms)
section 8, and the decisions in
[docs/adr/fonts/](https://github.com/fand/egui-react/tree/main/docs/adr/fonts).
