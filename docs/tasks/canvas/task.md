# タスク: canvas(PR7 = フェーズ 6.5 の残り、実装は停止中)

## 目的

コンポーネントの中で wgpu の shader アニメーションを描けるようにする。そのために必要な最小の口を開ける: `<Canvas>` 要素(taffy から矩形をもらい `on_paint(ui, rect)` を呼ぶ leaf)と、`examples/shader`(fullscreen triangle + fragment shader、state が uniform に流れる)。ランナー側の口 `Options.setup`(起動時に 1 回、pipeline を作って `callback_resources` に置く場所)は済んでいる。

もとは [docs/tasks/examples/](../examples/task.md) の PR C だった。PR A(#4)/ PR B(#6 → #8)が先にマージされ、C は `Options.setup` だけ実装した時点で作業を止めたので、残りを独立タスクに切り出した。core(`egui-react`、`egui-react-macros`)には手を入れない。

## 現状

- 済: `Options.setup: Option<Setup>`(`egui-react-app`)。`ReactApp::new` の先頭で呼ぶ。`wgpu = "30.0"` を workspace に pin。ARCHITECTURE 7 / 8 章更新。
- 済(判断): `wgpu` feature は置かない。eframe 0.36 は既定が wgpu で glow が opt-in。WebGL fallback も `egui-wgpu/default` 経由で入っている。
- 未: `<Canvas>` 要素、`examples/shader`、gallery への登録、テスト、README。

## スコープ

### 含む

- `<Canvas>`(`egui-react-elements`): `style` / `sense` / `on_paint` / `on_drag` / `on_hover`。`leaf_fill` で taffy がサイズを決める。egui-wgpu には依存しない。kittest 付き。
- `examples/shader`: `lib.rs`(App)/ `gpu.rs`(`setup(cc)`、`ShaderResources`、`ShaderCallback: CallbackTrait`)/ `shader.wgsl` / `main.rs`。Slider(speed)、Checkbox(pause)、drag で uniform を動かす。native と trunk で動く。gallery に登録し、gallery の `setup` で pipeline を登録する。
- escape-hatch example の painter 節を `<Canvas>` に置き換える(1 行で済むなら)。
- README の表に shader を足す。ARCHITECTURE 6 章に `Canvas`。

### 含まない

- shader の生 egui 版(差が出ない)。
- wgpu 以外の描画 API、egui_glow の callback。
- `Canvas` の可変 `Sense` 以上のイベント(ホイール、キー)。要望が出てから。

## 成果物

- `crates/egui-react-elements/src/canvas.rs` + `tests/canvas.rs`。
- `examples/shader/`。
- gallery / README / ARCHITECTURE の更新。plan.md 7 章に実装で判明した差分。

## 終了条件

- kittest: `Canvas` の `rect` が `w h` / `grow` に従う。`on_drag` / `on_hover` が発火する。shader の Slider が `speed` を変え、pause で `request_repaint` が止まる。
- `cargo run -p shader` でアニメーションし、Slider で速度が変わる(目視)。`trunk serve` でブラウザでも同じ(目視、WebGPU 非対応ブラウザは WebGL fallback)。
- gallery に shader が載り、他の example と同居して pipeline 登録が衝突しない。
- CI(fmt / clippy / test / wasm check / trunk ループ)が緑。

## 決めごと(着手時点での前提)

- `Options.setup` で pipeline を作り、hook / context に wgpu の型を出さない(`use_context` で `RenderState` を配る案は採らない)。
- `Canvas` は egui-wgpu を知らない。callback を `painter().add` するのは利用側。
- 閉包 prop は `impl for<'a> FnOnce(&'a mut egui::Ui, egui::Rect)` と higher-ranked bound を明示する。`#[component]` は prop の省略された lifetime を props 構造体のものに書き換えるため、省略形はコンパイルできない(`VirtualList` で確認済み)。
- `#[prop(default = ..)]` に式(`egui::Sense::hover()`)が通らなければ `Option<egui::Sense>` + `unwrap_or`。
- snapshot(C-3)は kittest の `WgpuTestRenderer` に `callback_resources` を差し込めるかで決める。無理なら落として目視のみ。
