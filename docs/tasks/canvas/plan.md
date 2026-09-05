# プラン: canvas

タスク定義は [task.md](task.md)。設計の根拠は [docs/ARCHITECTURE.md](../../ARCHITECTURE.md)。本書は [docs/tasks/examples/plan.md](../examples/plan.md) 3 章(PR C)を切り出したもので、3.1(`wgpu` feature)は不要と判明したので落とし、3.2 は実装済み。実装中にここから外れる判断をした場合は本書を更新し、設計上の意味があれば ARCHITECTURE.md も更新する。

## 0. 全体

1 PR。触るのは `react-egui-elements`(`Canvas`)、`examples/shader`、`examples/gallery`(登録と `setup`)、`examples/escape-hatch`(painter 節の置き換え、任意)、README、ARCHITECTURE。`react-egui-app` は済み(`Options.setup`)。core は無変更。

前提の訂正: eframe 0.36 は既定で wgpu(glow は opt-in)、WebGL fallback は `egui-wgpu/default` 経由で入っている。backend を選ぶ feature は置かない。

## 1. 設計(examples plan 3.2〜3.5 より)

### 3.2 `Options.setup`

```rust
pub struct Options {
    ..
    /// eframe が起動した直後に 1 回呼ぶ。wgpu の pipeline を作って
    /// `render_state.renderer.write().callback_resources` に置く場所。
    pub setup: Option<Box<dyn FnOnce(&eframe::CreationContext<'_>)>>,
}
```

`ReactApp::new` の先頭で呼ぶ。egui 公式 demo(`custom3d_wgpu`)と同じ形。`use_context` で `RenderState` を配る案は slot を作る手間に対して得るものが少ないので採らない。hook にも context にも wgpu の型は出ない。

### 3.3 `<Canvas>` 要素(`react-egui-elements`)

```rust
#[component]
pub fn Canvas(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    #[prop(default)] sense: egui::Sense,          // 既定 hover
    on_paint: impl FnOnce(&mut egui::Ui, egui::Rect),
    #[event] on_drag: egui::Vec2,                  // drag_delta
    #[event] on_hover: egui::Pos2,                 // 矩形内のポインタ位置
)
```

- `cx.leaf_fill(&style, |ui| { let (rect, resp) = ui.allocate_exact_size(ui.available_size(), sense); on_paint(ui, rect); resp })`。`leaf_fill` なので大きさは taffy が決める(`w h` / `grow`)。
- elements は egui-wgpu に依存しない。callback を `ui.painter().add(..)` するのは example 側。`egui::Painter` で線を引くだけの用途(plot 等)にも使える。
- テスト(kittest、headless): `on_paint` に渡る `rect` が `w h` で指定した大きさになる。`grow={1.0}` で残り全部になる。drag で `on_drag` が発火する。

### 3.4 `examples/shader`

```
examples/shader/src/
  lib.rs      App: <Canvas grow> + Slider(speed) + Checkbox(pause) + ComboBox(shader 選択)
  gpu.rs      setup(cc)、ShaderResources、ShaderCallback: CallbackTrait
  shader.wgsl fullscreen triangle + fragment。uniform { time, resolution, mouse, speed }
  main.rs     run(Options { setup: Some(Box::new(gpu::setup)), .. }, ..)
```

```rust
#[component]
fn App(cx: &mut Cx) {
    let mut speed = use_state(cx, || 1.0f32);
    let mut paused = use_state(cx, || false);
    let mut mouse = use_state(cx, || egui::Vec2::ZERO);
    let time = cx.ui().input(|i| i.time) as f32;
    if !*paused {
        cx.ui().ctx().request_repaint();
    }
    rsx! {
        <View direction="column" gap={8} grow={1.0}>
            <Canvas
                grow={1.0}
                on_drag={|d: egui::Vec2| *mouse += d}
                on_paint={|ui: &mut egui::Ui, rect| {
                    ui.painter().add(egui_wgpu::Callback::new_paint_callback(
                        rect,
                        gpu::ShaderCallback { time: time * *speed, mouse: *mouse },
                    ));
                }}
            />
            <View direction="row" gap={8} align="center">
                <Slider bind={speed.bind()} range={0.0..=4.0} label="speed"/>
                <Checkbox bind={paused.bind()} label="pause"/>
            </View>
        </View>
    }
}
```

- `ShaderCallback::prepare` で `queue.write_buffer` に uniform を書き、`paint` で 3 頂点を描く。viewport / scissor は egui-wgpu が `rect` に合わせる。
- `state` がそのまま uniform に流れるのが見せ所。`Slider` の `bind` → `speed` → uniform。
- `ShaderResources` は `callback_resources`(`TypeMap`)に型で置く。gallery で他の example と同居しても型が違えば衝突しない。
- 多重パス: 捨てられたパスの shape は egui が破棄する。callback が二重に走ることはない。
- gallery は `react-egui-app/wgpu` を on にし、`setup` で `shader::gpu::setup(cc)` を呼ぶ。
- 生 egui 版は作らない(wgpu 部分は同じコードになり、差が出ない)。

### 3.5 テスト

| # | 内容 |
|---|---|
| C-1 | `Canvas` の `rect` と `on_drag`(3.3、headless) |
| C-2 | shader: Slider を動かすと `speed` が変わり、pause で `request_repaint` が止まる(`harness` の repaint 要求を見る) |
| C-3 | snapshot(feature `snapshot`、ローカルのみ): time を 0 に固定して 1 枚。kittest の `WgpuTestRenderer` の render state に `setup` 相当を流し込めるかは 5 章 |


## 2. 手順

1. `<Canvas>` + kittest(1 の 3.3)。ARCHITECTURE 6 章。コミット。
2. `examples/shader`(native → trunk)。gallery に登録し、gallery の `Options.setup` で `shader::gpu::setup(cc)` を呼ぶ。README の表。コミット。
3. escape-hatch の painter 節を `<Canvas>` に置き換える(1 行で済む場合のみ)。コミット。
4. snapshot(C-3)を試す。無理なら 3 章に理由を書く。PR。

## 3. 実装で判明した差分

### 手順 1: `Canvas`

**閉包 prop の名前は `on_paint` ではなく `paint` にした。** `rsx!` は属性名が `on_` で始まればそれをイベントハンドラと見なし、`<要素名>Event` の variant(`on_paint` → `CanvasEvent::Paint`)を探しに行く(`crates/react-egui-macros/src/rsx/mod.rs` の属性の振り分け)。つまり `on_` で始まる普通の prop は rsx! からは書けない。逃げ道は「core を変える」か「名前を変える」かで、core 無変更が前提なので後者を採った。`VirtualList` の `render` と同じ命名になり、`on_*` = イベント、それ以外 = prop という読み方も保てる。3.4 の shader example のコードも `paint={..}` になる。

`#[prop(default = egui::Sense::hover())]` は通った(task.md の「通らなければ `Option<egui::Sense>`」は不要)。イベントは `Response` から発火する: `dragged()` なら `on_drag(drag_delta())`、`hover_pos()` があれば `on_hover(pos)`。kittest は `crates/react-egui-elements/tests/canvas.rs` に 4 本(`w`/`h` どおりの rect、`grow` で残り全部、drag の delta、hover の位置)。イベントハンドラは `move` で書けない(融合された閉包が `FnMut` なので、テストの `Rc` は借用で捕まえる)。

### 手順 6: `Options.setup`(と `wgpu` feature を置かない判断)

**3.1 の前提が間違っていた。「eframe は default(glow)のまま」は eframe 0.36 では成り立たない。** eframe 0.36.1 の `default` feature は `["accesskit", "default_fonts", "links", "wayland", "web_screen_reader", "wgpu", "winit/default", "x11"]` で、**`glow` は入っていない**。`Renderer::Glow` は `glow` feature が無いと存在すらせず、`Renderer::default()` は `Wgpu` を返す。つまり **このリポジトリは最初から wgpu で描いていた**。0.35 までとは逆で、今は glow の方が opt-in である。

そのため **`wgpu` feature は置かない**。一度は `wgpu = ["eframe/wgpu"]` を足したが、今日の eframe では何も変えない feature であり、API の雑音にしかならない。5 章の「wgpu を唯一の backend にするか」は、eframe 側が先に決めてくれた形になる。glow で動かしたい人は `eframe/glow` を明示する話で、それはこの crate の仕事ではない。判断の根拠は `crates/react-egui-app/Cargo.toml` のコメントと ARCHITECTURE 8 章に残した。

**WebGL fallback も何もしなくても入っている。** `eframe/wgpu` → `egui-wgpu/default` → `wgpu/webgl`。3.1 の「wasm は `wgpu` の `webgl` feature を on にする」は不要だった。`[workspace.dependencies]` には `wgpu = "30.0"`(eframe 0.36.1 が使う版)を pin だけしてある。shader example が pipeline を組むときに同じ wgpu へリンクするため。

**`Options.setup`** は 3.2 のとおり足した。型は `Option<Setup>`、`pub type Setup = Box<dyn FnOnce(&eframe::CreationContext<'_>)>`(clippy の `type_complexity` が生の型を蹴るので別名にした。API としてもこちらが読みやすい)。`ReactApp::new` の先頭で `take()` して呼ぶ。1 フレーム目に paint callback が追加されうるので、store を作るより前に走らせる。`ReactApp::new` の `options` 引数を `&Options` から `&mut Options` にし、native / wasm どちらの起動閉包も `options` を move で持って `take` する(閉包はどちらも 1 回しか呼ばれない)。

- テストは `crates/react-egui-app/src/lib.rs` の `#[cfg(test)] mod tests` に 1 つ、`setup` の既定が `None` であること。kittest は eframe を動かせないので、実際に wgpu で描かれることの確認は目視(`RUST_LOG=eframe=info`)に委ねる。
- ARCHITECTURE 7 章(`Options` の一覧と `setup`)と 8 章(バックエンドと WebGL fallback)を更新。
