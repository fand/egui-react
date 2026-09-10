---
title: Web とネイティブ
---

# Web とネイティブ

`run` を 1 回呼べば、ネイティブではウィンドウが開き、ブラウザでは`<canvas>` を乗っ取ります。ランナーは `egui-reactor-app` で、eframe を知っている唯一のクレートです。

## `run(Options, root)`

```rust
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

ランナーは毎フレーム、全体を `CentralPanel` で包み、ウィンドウを埋めるルートのレイアウトノード（column、幅 `100%`、最小高さ `100%`）の中にルートのビューを描きます。だからルートの子は、いきなり `grow` や `justify` を使えます。

ルートのクロージャの中でフックは使わないでください。そこで作ったガードはクロージャより長生きできません。フックはコンポーネントに置き、ルートは`|_cx| rsx! { <App/> }` のままにします。

## `Options`

| フィールド | 意味 |
|---|---|
| `title` | ネイティブのウィンドウタイトル。wasm では無視される |
| `max_passes` | 1 フレームで egui が走らせてよいパス数。既定は 3。ノードが増えたり消えたり動いたりすると、レイアウトがもう 1 パス要求する |
| `persist` | `use_persisted` が eframe のストレージを読み書きするかどうか |
| `canvas_id` | 取り付ける `<canvas>`。wasm のみ。既定は `"egui_reactor_canvas"` |
| `native` | `eframe::NativeOptions`。ネイティブのみ |
| `setup` | eframe がウィンドウと描画バックエンドを用意した直後に 1 回だけ呼ばれる |

```rust
Options {
    title: String::from("my app"),
    #[cfg(not(target_arch = "wasm32"))]
    native: eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1280.0, 800.0]),
        ..Default::default()
    },
    ..Default::default()
}
```

## `setup` と wgpu

`setup` はネイティブでも Web でも、何かが描かれる前に走ります。フックの API にwgpu の型は出てこないので、wgpu を使わないアプリがそれを目にすることはありません。

```rust
Options {
    setup: Some(Box::new(move |cc| {
        // パイプラインを作り、`cc.wgpu_render_state` の
        // `callback_resources` に型を鍵にして入れる。
        shader::gpu::setup(cc);
        cc.egui_ctx.add_plugin(WebA11y::new(canvas_id));
    })),
    ..Default::default()
}
```

描画は `<Canvas>` を通します。その `paint` クロージャは矩形を受け取り、`egui_wgpu::Callback::new_paint_callback(rect, ..)` をペインタに積みます。[shader](/ja/examples/shader) がその往復のすべてです。[patch](/ja/examples/patch) は、自分が走らせる WGSL を生成します。

## Web ビルド

`canvas_id` と一致する id を持つ canvas の入った `index.html` が要ります。

```html
<link data-trunk rel="rust" href="Cargo.toml" data-wasm-opt="z" />
<canvas id="egui_reactor_canvas"></canvas>
```

それに、canvas をページいっぱいに広げる CSS も要ります。あとは `trunk serve`、デプロイ用の `dist/` が欲しければ `trunk build --release` です。

レンダラは wgpu で、WebGL にフォールバックします。eframe の既定のままです。

### サイズ

egui のアプリは数 MB の wasm になります。`data-wasm-opt="z"` は外さず、`--release` でビルドし、自前のフォントを配るなら `default_fonts`（約 1.4 MB）を切ってください。[フォント](/ja/guide/fonts) を見てください。

### Web でのアクセシビリティ

egui は毎フレーム AccessKit の木を作ります。ネイティブでは eframe がそれを OS に渡すので、スクリーンリーダーが動きます。Web では eframe がそれを捨てます。`egui-reactor-app` には `a11y::WebA11y` が入っています。木を canvas の上の隠しDOM 要素に写すプラグインです。

```rust
setup: Some(Box::new(move |cc| {
    cc.egui_ctx.add_plugin(egui_reactor_app::a11y::WebA11y::new("egui_reactor_canvas"));
})),
```

wasm 以外のターゲットでは中身が空なので、条件を付けずに登録して構いません。これは AccessKit を有効にするので、毎フレーム、ウィジェット 1 つにつきノード1 つ分のコストがかかります。

スクリーンリーダーはこの写しを読めます。`<Button>` の `label` と `<Image>` の`alt` も読むので、埋めておいてください。ただしアプリはやはり canvas です。ページ内検索も、検索エンジンも、ブラウザの翻訳も、何も見えません。このドキュメントサイトが HTML で、wasm なのはサンプルだけなのは、そのためです。

## 動かして見る

どのサンプルページも同じ wasm ビルドで、どれを描くかは URL で伝えています。wgpu の道筋は [shader](/ja/examples/shader)、HTTP でのフォント取得は[font](/ja/examples/font) です。ランナーとプラットフォームの話は[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#7-crate-layout)の 7 章と 8 章にあります（英語）。
