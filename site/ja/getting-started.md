---
title: はじめる
---

# はじめる

ウィンドウの中のカウンタを作り、同じものをブラウザのタブで動かします。

## 前提

2024 エディションが使える stable の Rust。

```sh
rustup update stable
```

Web ビルドをするなら [trunk](https://trunkrs.dev/) と wasm ターゲットも入れます。

```sh
cargo install --locked trunk
rustup target add wasm32-unknown-unknown
```

## 依存を追加する

まだ crates.io には出していないので、git から取ります。

```toml
[package]
name = "my-app"
version = "0.1.0"
edition = "2024"

[dependencies]
egui-reactor = { git = "https://github.com/fand/egui-react" }
egui-reactor-elements = { git = "https://github.com/fand/egui-react" }
egui-reactor-app = { git = "https://github.com/fand/egui-react" }
egui = "0.36.1"
eframe = "0.36.1"
```

- `egui-reactor` が中核です。`Cx`、フック、`rsx!`、レイアウトエンジン。
- `egui-reactor-elements` は `rsx!` の中に書くもの。`<View>`、`<Text>`、`<Button>` などです。
- `egui-reactor-app` がウィンドウを開くか、canvas を乗っ取ります。
- `egui` と `eframe` は、自分のコードがその型を書くので必要です。

## カウンタ

リポジトリの `examples/counter` そのものです。

### コンポーネント

```rust
use egui_reactor::prelude::*;
use egui_reactor_elements::prelude::*;

#[component]
pub fn App(cx: &mut Cx) {
    let mut count = use_state(cx, || 0i32);

    rsx! {
        <View direction="column" align="center" justify="center" gap={12} grow={1.0}>
            <Text size={32.0} strong>{format!("{}", *count)}</Text>
            <View direction="row" gap={8}>
                <Button on_click={|| *count -= 1}>"-"</Button>
                <Button on_click={|| *count = 0}>"reset"</Button>
                <Button on_click={|| *count += 1}>"+"</Button>
            </View>
        </View>
    }
}
```

`#[component]` を付けると、その関数を `<App/>` として使えます。第 1 引数はつねに `cx: &mut Cx` で、残りの引数はすべて props です。

`use_state(cx, || 0i32)` はフックストアのスロットを指すガードを返します。Deref するので `*count += 1` がすべてです。`set_count` はありません。3 つのハンドラはそれぞれ `count` を `&mut` で順に借ります。ハンドラはこのフレームの中で走り、すぐ捨てられるので、これで通ります。

`rsx!` はその場で egui の呼び出しに展開されます。`<Button on_click={..}>` は「いまボタンを描く。クリックされていたら、いまこのクロージャを走らせる」という意味です。

- テキストはつねにクォートした文字列です。
- `{expr}` で式を埋め込みます。
- `direction`、`gap`、`grow` はレイアウト属性です。[レイアウト](/ja/guide/layout)を見てください。

### ランナー

```rust
use egui_reactor::prelude::*;
use egui_reactor_app::{Options, run};

fn main() -> eframe::Result {
    run(
        Options {
            title: String::from("egui-reactor: counter"),
            ..Default::default()
        },
        |_cx| rsx! { <App/> },
    )
}
```

`run` はフックストアを持ち、毎フレーム、ルートのビューを描きます。ネイティブでも wasm でも同じです。ルートのクロージャの中でフックは使わないでください。そこで作ったガードはクロージャより長生きできません。フックはコンポーネントに置きます。

## ネイティブで動かす

```sh
cargo run
```

## ブラウザで動かす

`Cargo.toml` の隣に `index.html` を置きます。canvas の id は`Options::canvas_id` と一致させます。既定値は `egui_reactor_canvas` です。

```html
<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1, user-scalable=no" />
    <title>counter</title>
    <link data-trunk rel="rust" href="Cargo.toml" data-wasm-opt="z" />
    <style>
      html,
      body {
        margin: 0;
        height: 100%;
        overflow: hidden;
        background: #101010;
      }
      canvas {
        display: block;
        width: 100%;
        height: 100%;
      }
    </style>
  </head>
  <body>
    <canvas id="egui_reactor_canvas"></canvas>
  </body>
</html>
```

あとは:

```sh
trunk serve
```

`data-wasm-opt="z"` は外さないでください。egui のアプリは最適化前で数 MB のwasm になります。[Web とネイティブ](/ja/guide/web-and-native) を見てください。

## Cargo フィーチャ

`egui-reactor-app` には 2 つあります。

- **`default_fonts`**（既定でオン）は egui 組み込みの 4 フォント、約 1.4 MB を残します。切ればその分を削れますが、代わりに自分でフォントを同梱し、最初のフレームより前に適用する必要があります。さもないと何も描かれません。
- **`woff2`**（既定でオフ）は HTTP で取得した WOFF / WOFF2 フォントをデコードします。無いときは、その取得が panic ではなく失敗として扱われます。

[フォント](/ja/guide/fonts) を見てください。

## 次に読むもの

- [仕組み](/ja/how-it-works): イミディエイトモードが React の習慣の何を変えるか。
- [rsx!](/ja/guide/rsx)、[コンポーネントとイベント](/ja/guide/components-and-events)、[状態とフック](/ja/guide/state-and-hooks): 文法。
- [フック](/ja/reference/hooks) と [要素](/ja/reference/elements): 一覧。
- [サンプル](/ja/examples/counter): どれも自分のページで動きます。
