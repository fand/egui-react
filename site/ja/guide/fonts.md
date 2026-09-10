---
title: フォント
---

# フォント

## フォントの設定が要る理由

egui はテキストを自分で描きます。素材は `Context::set_fonts` に渡されたフォントのバイト列だけです。ブラウザのフォントも OS のフォントマッチングも使いません。egui 組み込みの 4 フォントには CJK のグリフが無いので、素の egui アプリではどのプラットフォームでも日本語が豆腐になります。

`egui_react_app::fonts` はこれを、名前付きソースの CSS 風フォールバックチェーンで解決します。チェーンは `fontdb` を通して egui のフォントファミリに解決されます。

## チェーン

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

最初のフレームより前に、`setup` から適用します。

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

項目はグリフごとに、書いた順で試されます。CSS の `font-family` のリストと同じです。`default_proportional(name)` と `default_monospace(name)` は、そのチェーンをすべてのウィジェットの既定にします。指定しなければ、チェーンは名前で呼ばれたところでしか使われません。

## 4 つのソース

| ソース | バイト列の出どころ |
|---|---|
| `Bundled(&'static [u8])` | `include_bytes!`。ネイティブでも wasm でも同じ。バイナリサイズを食う |
| `System(String)` | 端末にインストール済みのフォント。フォントが名乗るファミリ名で指定する。Web では Local Font Access API が要る |
| `Url(String)` | HTTP で取得する。バイト列が届くまでは保留で、届いたらチェーンを適用し直す |
| `Generic(Generic::SansSerif)` | CSS の総称ファミリ（`sans-serif`、`serif`、`monospace`、`cursive`、`fantasy`）。fontdb が解決し、その後ろに egui 自身のフォントが控える |

`System` の名前は、そのフォントが名乗るとおりでなければいけません。言語はどれでも構いません。`"Hiragino Sans"` も `"ヒラギノ角ゴシック"` も通りますが、`"hiragino sans"` は通りません。各項目が何に解決されたかは `Fonts::report()`が並べてくれるので、正しい綴りはそこで探せます。

どのソースを使うかはターゲット次第です。ネイティブなら `System`。マシンにはすでにフォントがあります。ブラウザなら自分のオリジンからの `Url`、サブセットが小さいなら `Bundled`。ブラウザの `System` は Chromium 限定で、許可のプロンプトが要り、クリックから呼ぶ必要があります。

## テキストごとにフォントを選ぶ

```rust
rsx! {
    <Text font="ui">"proportional, from the chain above"</Text>
    <Text font="monospace">"egui's built-in monospace"</Text>
}
```

`font` は登録したスタックの名前を取ります。egui 組み込みのファミリなら`"proportional"` / `"monospace"` です。知らない名前を渡すと、そのスタイル本来のファミリに戻り、警告を 1 回だけ記録します。

この prop があるのは `<Text>` だけです。`<Button>` や `<Checkbox>` のラベルは`default_proportional` を使うか、子として `RichText::new(..).family(..)` を受け取ります。

## どのチェーンの後ろにもいるフォールバック

egui 組み込みの 4 フォント（約 1.4 MB）は、すべての総称ファミリの後ろと、すべてのチェーンの末尾に控えています。これは `egui-react-app` の`default_fonts` フィーチャの下にあり、既定でオンです。切っても panic はしませんが、何にも解決できなかったチェーンはグリフを描きません。切るアプリは、どのスタックにも `Bundled` か `System` のフェイスを与えるか、`Fonts::pending()` が false になるまでテキストの無い画面を描く必要があります。

## Web フォント: 大きさと待ち時間

Web フォントは実際にダウンロードが発生します。

**サブセットにしてください。** [font](/ja/examples/font) のサンプルにあるNoto Sans JP は、フルセットで 4.5 MB です。同梱しているサブセット（ASCII、Latin-1、かな、CJK の約物、漢字およそ 500 字）は 433 KB です。サブセットならバイナリに入れられます。フルセットは入れられません。

**読み込み中に何を描くか決めてください。** URL がまだ飛行中かどうかは`Fonts::pending()` が教えます。方針を決めるのはあなたです。`Url` の後ろに同梱サブセットを置けば CSS の `swap` になります。いますぐ読めるテキストが出て、あとでより良いテキストに変わります。`pending()` が false になるまでプレースホルダを出せば `block` です。

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

クロスオリジンの URL には `Access-Control-Allow-Origin` が要ります。WOFF とWOFF2 には `woff2` の cargo フィーチャが要ります。バリアブルフォントよりスタティックなフェイスを選んでください。epaint はバリアブルフォントの既定インスタンスを描くので、欲しいウェイトとは限りません。

## 動かして見る

[font](/ja/examples/font) には 4 つのチェーン、3 つのソース、項目ごとのレポート、`swap` と `block` の切り替えが入っています。リゾルバの説明は[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#8-platforms)の 8 章、決定の記録は[docs/adr/fonts/](https://github.com/fand/egui-react/tree/main/docs/adr/fonts)にあります（どちらも英語）。
