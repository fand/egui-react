# プラン: examples

examples の拡充と、ブラウザで全 example を試せる gallery ページの計画。設計の根拠は [docs/ARCHITECTURE.md](../../ARCHITECTURE.md)。本書は実装手順と確認方法を定める。実装中にここから外れる判断をした場合は本書を更新し、設計上の意味があれば ARCHITECTURE.md も更新する。

## 0. 全体

狙いは 2 つ。

1. egui.rs と同じく、live example をブラウザで見られるまとめページ(gallery)を持つ。
2. gallery で example と実装コードを並べ、生 egui との差を見せる。「同じ UI を生 egui で書いた版」を並べて、状態管理とレイアウトの差をコードと行数で示す。

3 PR に分ける。順に積む。

| PR | 内容 | 触る場所 |
|---|---|---|
| A: gallery | example の lib / bin 分割、gallery、生 egui 版(counter / todo / layout)、snapshot 一致テスト、GitHub Pages | `examples/*`、`examples/gallery`、CI、README |
| B: examples | form / theme / clock / custom-hook / escape-hatch / list-10k / shell / showcase | `examples/*`、gallery への登録 |
| C: canvas | `react-egui-app` の `wgpu` feature と `Options.setup`、`<Canvas>` 要素、shader example | `react-egui-app`、`react-egui-elements`、`examples/shader`、gallery |

core(`react-egui`、macros)には手を入れない。入れる必要が出たら 7 章に書く。

追加する依存(`[workspace.dependencies]` に pin する)。

| crate | 用途 | 場所 |
|---|---|---|
| egui_extras(feature なし) | gallery のコード表示(`syntax_highlighting::code_view_ui`)。`syntect` は wasm が太るので使わない。組み込みの簡易ハイライタで足りる | examples/gallery |
| eframe `wgpu` feature | paint callback | react-egui-app(feature `wgpu` の裏) |
| wgpu(eframe 経由、web は `webgl` feature も) | shader example の pipeline | examples/shader |
| bytemuck | uniform の `Pod` | examples/shader |

## 1. PR A: gallery

### 1.1 example の lib / bin 分割

各 example を `lib.rs` + 薄い `main.rs` にする。gallery が lib を依存に取り、`cargo run -p counter` は今まで通り動く。

```
examples/counter/
  Cargo.toml        [lib] + [[bin]] counter (+ [[bin]] counter-plain、1.3)
  src/lib.rs        pub fn App(cx) と pub const META
  src/main.rs       run(Options{..}, |_cx| rsx!{ <App/> })
  src/plain.rs      生 egui 版(1.3)
  index.html / Trunk.toml
```

```rust
pub struct Meta {
    pub name: &'static str,     // "counter"
    pub summary: &'static str,  // 1 行
    pub hooks: &'static [&'static str],
    pub elements: &'static [&'static str],
    pub source: &'static str,   // include_str!("lib.rs")
    pub plain: Option<&'static str>, // include_str!("plain.rs")
}
```

gallery に埋め込む example の約束。

- `App` は渡された領域を埋めるコンポーネント。ルートは `<View grow={1.0}>`。
- `Panel` / `CentralPanel` を使わない(親から場所を切り取るため。ARCHITECTURE 6 章)。
- `use_persisted` のキーは `"<example>/<key>"`。gallery では全 example が同じ `STORAGE_KEY` を共有するので衝突を避ける。todo の `"todos"` は `"todo/todos"` に改める。
- `std::time::Instant` を使わない(wasm で panic)。時刻は `ui.input(|i| i.time)`。

fetch は既に main にある(PR3)。同じ形に分割して gallery に載せる。

### 1.2 gallery(`examples/gallery`)

1 つの wasm。example ごとに別ページを出すより、一覧 / 実行 / コードを 1 バイナリに置いた方が切替が速く、Pages へのデプロイも 1 つで済む。

画面は flex 3 列。`Panel` は使わない(埋め込む example と同じ制約に gallery 自身も従う)。

```
+----------+----------------------+-------------------------+
| 一覧     | 実行中の example     | コード                  |
| (w=200)  | (grow)               | (w=480, ScrollArea)     |
| タグで   | <View key={name}     | [react-egui | 生 egui]  |
| 絞り込み |   grow={1.0}>        | N 行 / M 行             |
|          |   {App}              | GitHub へのリンク       |
+----------+----------------------+-------------------------+
```

- 一覧は `Meta` から作る。hooks / elements のタグをクリックすると絞り込む。
- `key={name}` で example を切り替えると前の example の hook は sweep で消える。状態のリセットはこれで足りる。
- コードは `egui_extras::syntax_highlighting::code_view_ui`。`cx.leaf_fill` で描く。
- 生 egui 版がある example はトグルを出し、コードも切り替える。両方の行数を並べる(`source.lines().count()`)。
- wasm では `location.hash` で直リンク(`#todo`)。native は第 1 引数。`web_sys` の `Location` feature を gallery の wasm 依存に足す。
- 生 egui 版の実行は `use_state(cx, PlainState::default)` に状態を持ち、`cx.leaf_fill(.., |ui| plain::ui(ui, &mut *state))` で描く。

gallery の `Options.setup` は PR C で shader の pipeline 登録に使う(3.2)。

### 1.3 生 egui 版(`plain.rs`)

react-egui 版と同じ見た目を egui だけで書く。差が出る場所を残す。

| example | 生 egui 版で見える差 |
|---|---|
| counter | ほぼ同じ。`ui.horizontal` + `ui.centered_and_justified` で中央寄せに手間がかかる程度。「小さい例では差が小さい」と正直に見せる |
| todo | 状態の持ち方(`struct` の field を全部 `&mut self` で回す)、削除をループ中でできないので index を持ち越す、永続化を `eframe::App::save` に手で書く |
| layout | `justify="space-between"` / `grow` / `wrap` / grid を `ui.horizontal` + `allocate_space` + 手計算で書く。ここが最も長くなる |

形は `pub struct PlainState` + `pub fn ui(ui: &mut egui::Ui, state: &mut PlainState)`。単体でも動くよう `[[bin]] counter-plain` を足し、`eframe::App` を実装した薄い `main` から呼ぶ。trunk は react-egui 版だけ。

### 1.4 テスト

example が lib になるので kittest を置ける。`cargo test --workspace` で回る。

| # | 場所 | 内容 |
|---|---|---|
| A-1 | `examples/counter/tests/` | `+` を押すと表示が 1 増える。生 egui 版でも同じ操作で同じ結果 |
| A-2 | `examples/todo/tests/` | 追加 / toggle / 削除 / clear done。生 egui 版も同じ |
| A-3 | `examples/gallery/tests/snapshots.rs`(feature `snapshot`) | 各 example の react-egui 版と生 egui 版を同じサイズで描き、**同じ snapshot 名**で比較する。1 つの画像に両方が一致すれば「見た目が同じでコードだけ違う」が主張になる |
| A-4 | `examples/gallery/tests/` | 一覧から example を選ぶと `App` が出る。タグの絞り込み |

snapshot は既存と同じく CI では回さない(README の Testing 節に gallery を足す)。

### 1.5 CI と Pages

- `ci.yml`: trunk build を counter / fetch の 2 ステップから、`examples/*/Trunk.toml` のループに変える。gallery も含む。
- `pages.yml`(新規): `main` への push で `trunk build --release --public-url /react-egui/ --config examples/gallery/Trunk.toml`、`actions/upload-pages-artifact` + `actions/deploy-pages`。URL は `https://fand.github.io/react-egui/`。
- リポジトリ設定で Pages の source を GitHub Actions にする(手作業、1 回)。

### 1.6 README

examples 節を表にする。

| 列 | 内容 |
|---|---|
| name | `counter` |
| what | `Meta.summary` と同じ 1 行 |
| live | gallery の直リンク(`#counter`) |
| source | `examples/counter/src/lib.rs` |

先頭に gallery のリンクとスクリーンショット 1 枚(gallery で todo を開いた状態。snapshot の画像は小さすぎるので手で撮る)。

## 2. PR B: 追加 examples

今ある機能で書けるもの。優先順。

| example | 見せるもの | 使う hooks / elements | 生 egui 版 | gallery |
|---|---|---|---|---|
| `form` | 設定フォーム。全ウィジェットの `bind`。`on_change` は変更ログ(`Vec<String>`)に積む。`use_persisted("form/settings")` で保存、reset ボタン | `use_state` `use_persisted`、`TextEdit` `Checkbox` `Slider` `ComboBox` `Collapsing` | あり | ○ |
| `theme` | `provide_context` で dark / light と言語(ja / en)を配る。3 段ネストした子が `use_context` で読む。切替は `ctx.set_visuals` | `use_handle` `provide_context` `use_context` | なし | ○ |
| `clock` | 時計 + ストップウォッチ。`ctx.request_repaint_after(1s)` で秒更新、動作中は毎フレーム。ラップ一覧を `for` + `key`。子を toggle で出し入れし、`use_effect` の cleanup がログに出る | `use_state` `use_effect`(cleanup) `use_memo` | なし | ○ |
| `custom-hook` | `#[hook]` で `use_debounce(cx, value, ms)`、`use_previous(cx, value)`、`use_window_size(cx)` を切り出し、2 つのコンポーネントから使う | `#[hook]` `use_state` `use_effect` | なし | ○ |
| `escape-hatch` | rsx の中で生 egui を使う 3 通り。`view(\|cx\| ..)` 閉包、`cx.leaf` で未ラップの widget(`ProgressBar` / `Hyperlink` / `color_edit_button`)、`cx.ui().painter()` で線を引く。`Cx::new` で入れ子の `Ui` に hook を置く(`nested_ui` テストの形) | `view` `Cx::leaf` `Cx::ui` | なし | ○ |
| `list-10k` | `for` + `key` で 10k 行を `ScrollArea` に出す。件数 Slider、絞り込み TextEdit、FPS(`stable_dt`)表示。immediate mode + taffy のコストを正直に見せる。生 egui 版は `ScrollArea::show_rows` で仮想化した版で、差を数字で出す | `use_state` `use_memo`、`ScrollArea` | あり | ○ |
| `shell` | IDE 風の枠。左 `Panel` にツリー(`Collapsing`)、中央エディタ(`TextEdit multiline`)、下 `Panel` にログ、浮いた `Window` にインスペクタ | `Panel` `Window` `Collapsing` `Frame` | なし | ×(単体 bin。`Panel` を使うため。5 章) |
| `showcase` | 小さい実アプリ: ノート。左に一覧 + 検索、右に `TextEdit multiline`。`use_reducer` で追加 / 削除 / 更新、`use_persisted` で保存、`use_memo` で検索結果、`Window` で設定、`use_context` でテーマ。全部を組み合わせた「使える」例 | ほぼ全部 | なし | ○ |

各 example に kittest を 1 ファイル置く(主要操作 1〜3 本)。生 egui 版がある form / list-10k は A-3 の snapshot 一致に加える(list-10k は件数を 100 に固定して撮る)。

`examples/template`(新規アプリの雛形)はフェーズ 8(公開準備)に送る。

## 3. PR C: canvas(wgpu)

**PR C は [docs/tasks/canvas/](../canvas/task.md) に切り出した。** 以下は切り出し時点の記録として残す。

コンポーネントの中で wgpu の shader アニメーションを描く。core は無変更で済む。ランナー、elements、example の 3 段。

### 3.1 `react-egui-app` の `wgpu` feature

- eframe は default(glow)のまま。`react-egui-app` に feature `wgpu = ["eframe/wgpu"]` を足す。
- `wgpu` feature が on のとき `Options::default()` の `native.renderer` を `eframe::Renderer::Wgpu` にする。glow は残す(両方コンパイルされる。feature は workspace で unify されるので `--workspace` では全 example が wgpu で動く。害はない)。
- wasm: WebGPU 非対応ブラウザのため `wgpu` の `webgl` feature を on にする。`trunk build` で確認。
- `Cargo.toml` の「`wgpu` は意図的に外す」は kittest の話で、headless テストには影響しない。

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

## 4. 手順

1. PR A-1: example 4 つ(counter / todo / layout / fetch)を lib / bin に分割、`Meta` を足す。`use_persisted` のキーに prefix。`cargo run` と `trunk serve` を目視。コミット。
2. PR A-2: gallery(一覧 / 実行 / コード、hash 直リンク)。3 例で動くこと。コミット。
3. PR A-3: 生 egui 版 3 つ + トグル + 行数。A-1〜A-4 のテスト。snapshot をローカルで生成してコミット。コミット。
4. PR A-4: CI の trunk ループ、`pages.yml`、README。Pages の source 設定(手作業)。デプロイ後に URL を目視。PR。
5. PR B: form → theme → clock → custom-hook → escape-hatch → list-10k → shell → showcase。1 example 1 コミット。それぞれ gallery に登録し、テストを足す。README の表に追加。PR。
6. PR C-1: `wgpu` feature + `Options.setup`。counter を `--features wgpu` で動かして wgpu で描かれることを確認(`RUST_LOG=eframe=info`)。コミット。
7. PR C-2: `<Canvas>` + C-1 のテスト。コミット。
8. PR C-3: shader example(native → trunk)。gallery に登録。C-2 / C-3。ARCHITECTURE.md 反映。PR。
9. 各 PR の本文に、変更点、ARCHITECTURE.md の変更、落としたものを書く。`plan-overview.md` の PR 表に A / B / C を足す(PR4 mobile の前後は問わない)。

## 5. 判断が必要になりそうな点

- **react-egui 版と生 egui 版の snapshot 一致(A-3)**。taffy と egui の丸めで 1px ずれる可能性がある。ずれたら閾値(`SnapshotOptions::threshold`)を緩めるか、諦めて別名の snapshot にして「並べて見せる」だけにする。layout で最初に試す。
- **`impl FnOnce` の prop(3.3)**。`#[component]` の typed-builder が閉包 prop で推論に失敗する場合は `&mut dyn FnMut(&mut egui::Ui, egui::Rect)` に落とす。
- **`Panel` を gallery に埋め込めるか(shell)**。`SidePanel::show_inside` は子 `Ui` から場所を切り取るので、gallery の中央列の中でなら見た目は成立するかもしれない。試して成立すれば shell も gallery に載せる。
- **kittest で shader の snapshot(C-3)**。`WgpuTestRenderer` の `RenderState` に `callback_resources` を差し込む口が無ければ、C-3 は落として目視だけにする。
- **wgpu を唯一の backend にするか**。gallery が wgpu を要求するなら glow を残す理由は薄い。ただし今は eframe default のままにして、フェーズ 8 で決める。
- **list-10k の `ScrollArea`**。要素は `show_rows` の仮想化を持たない。生 egui 版との差が大きすぎるなら `ScrollArea` に `rows={(count, row_h)}` prop を足す(elements の変更。別 PR でもよい)。
- **gallery の wasm サイズ**。全 example + wgpu で数 MB になる。`wasm-opt = "z"` は既に入れている。重ければ shader だけ別 wasm に分ける。
- **fetch の gallery 上での動作**。Pages は https なので `http://` の URL は混在コンテンツで落ちる。既定 URL を https にしてある(現状 `https://httpbin.org/get`)。CORS で落ちる URL は注記する。

## 6. ARCHITECTURE.md に反映する変更

- 6 章 要素一覧: コンテナに `Canvas`(`sense` / `on_paint`, `on_drag` / `on_hover`)。「egui-wgpu に依存しない。callback は利用側が `painter().add`」。
- 7 章 クレート構成: `react-egui-app` の feature `wgpu`、`Options.setup`。examples は lib + bin で gallery が lib を依存に取る。
- 8 章 プラットフォーム: wgpu backend、web は WebGL fallback。Pages の URL。
- 9 章 テスト: examples に kittest。react-egui 版と生 egui 版の snapshot 一致(gallery の `snapshot` feature)。
- 11 章 決定ログ: gallery を 1 wasm にした理由、生 egui 版を並べる理由、`Options.setup` を `use_context` より優先した理由、`Canvas` が egui-wgpu を持たない理由。

## 7. 実装で判明した差分

### 手順 1(A-1)

- `Meta` は `examples/meta`(package `example-meta`)に置き、全 example と gallery が共有する。`[workspace.dependencies]` に登録。`Copy` を derive(全 field が `&'static`)。
- layout のルートは `<ScrollArea grow>` だったので `<View direction="column" grow={1.0}>` で包んだ(1.1 の約束に合わせる)。
- `META` は各 `lib.rs` の `use` の直後に置いた。`source` はファイル全体なので gallery のコード欄の先頭に出る。邪魔なら末尾に移す。
- todo の永続化キーは `"todo/todos"`。旧キー `"todos"` の移行は書かない。
- README の「`examples/counter` verbatim」の一文が lib / bin 分割で古くなった。手順 4(README)で直す。
- 目視(`cargo run` / `trunk serve`)は subagent が headless のため未実施。手順 4 のデプロイ確認と合わせて行う。

### 手順 2.5(バグ修正)

gallery を目視して見つかった、core 以外の 2 つのバグ。計画には無かったので 1 コミット足す。`crates/react-egui`(core)とマクロは無変更。

**バグ 1: taffy leaf の中でテキストが 1 文字ずつ縦に並ぶ。** `cargo run -p counter` の `reset` ボタンが 15x77(1 文字幅)になっていた。原因は egui_taffy の測り方で、leaf は「前回描いた時の `ui.min_size()`」だけを覚え(`ui_finite` が `min_size` と `max_size` に同じ値を入れる)、taffy にはそれを min-content としても max-content としても返す。最初の描画は幅 0 の `Ui` で起きるので、wrap する widget はそこで 1 文字幅を報告し、ノードはその細さで固定される。`grow` や `w` を持つ leaf は taffy が幅を決めるので無事で、そのため gallery の一覧ボタン(`grow={1.0}`)だけは正常に見えていた。修正は `react-egui-elements` で、テキストを持つ leaf を全て `TextWrapMode::Extend` にする(`Text` が既にやっていたこと)。`Button` / `Label` は widget の `wrap_mode` builder、`Checkbox` / `Slider` / `ComboBox` / `Collapsing` のヘッダは leaf の `Ui` の `style.wrap_mode`。`Label` には `wrap` 属性を足して egui 既定の折り返しに戻せるようにした(`Text` と同じ)。テストは `crates/react-egui-elements/tests/widgets.rs` の `a_label_in_a_taffy_leaf_stays_on_one_line`。

**バグ 2: ルートコンテナが窓を埋めない。** counter の `0` とボタンが中央ではなく左上に出ていた(gallery の中に埋めた同じ `App` は中央に出る)。`reserve_available_space()` は egui_taffy に available space を伝えて `ui.set_min_size` するだけで、ルートノード自身の `size` は `auto` のままなので、taffy はそのノードを中身の大きさにする。`<View grow={1.0} justify="center">` は広がる余地も中央寄せする先も持たない。修正は `react-egui-app` で、ルートの `ItemStyle` に `w("100%")` / `min_h("100%")` を入れる(縦だけ最小値にしたのは、中身が窓より高い時にそのまま伸ばすため)。ついでにルートの id とスタイルを `root_id()` / `root_style()` として公開し、テストが同じ枠を再現できるようにした(`crates/react-egui-app/tests/root_fill.rs`)。

- gallery の `Chip` は残す。`Button` は直ったが、タグは押された状態を見せたいのに elements にトグル要素が無く、`egui::SelectableLabel` には `wrap_mode` builder も無いため。leaf を手で書く側も `style.wrap_mode` を置く必要がある、という例になっている。
- elements にトグル要素(`SelectableLabel` / `RadioButton`)が無いのは今後の候補。

### 手順 2.6(gallery の 3 列がはみ出す)

バグ 2 と同じ原因の続き。gallery で layout を選ぶとコード列が画面外に出て、layout の `w="100%"` 行が 1600px 幅になっていた。

原因は「ルートノードが中身のサイズになる」ことのもう半分である。`min_w("100%")` はルートに下限を与えるだけで `size` は `auto` のままなので、中身が窓より広ければルートはそれに合わせて広がる。overflow が発生しないということは `flex-shrink` の出番も無いということで、真ん中の列は `grow={1.0} min_w={0.0}` を持っているのに縮まず、行がそのまま窓の外へ伸びる。`min_w` が taffy に届いていない訳ではない(`Length::Px(0.0)` → `Dimension::length(0.0)`)。

修正はルートの `w` を `100%` にして幅を確定させること(`react-egui-app`)。窓の幅は固定なので確定値でよく、それより広いものは横 `ScrollArea` に入れる話になる。縦は `min_h("100%")` のまま。core(`layout.rs`)に `overflow` を足す必要は無かった。

- gallery 側はコード列を `shrink={0.0}` にした。幅を確定させただけだと、layout のように中身が大きい example の時にコード列が `min_w` の 360 まで削られる。`shrink={0}` なら overflow は全部真ん中の列(`min_w={0}`)へ行き、列幅が example によって動かない。
- gallery のテストはランナーと同じ枠を使うよう `react_egui_app::root_id()` / `root_style()` に切り替えた。自前で組んだ枠のままでは、まさにこのバグをテストが見逃す。

### 手順 3(A-3)

生 egui 版 3 つ、gallery のトグル、A-1〜A-3 のテスト。

行数(`source.lines().count()`、META と doc コメントを含むファイル全体)。

| example | react-egui | 生 egui |
|---|---|---|
| counter | 32 | 84 |
| todo | 143 | 143 |
| layout | 144 | 278 |

todo が同数になるのは、`lib.rs` 側に `META`(12 行)と reducer の定義が入っているため。中身の差(`Msg` + `use_reducer` 対「index を持ち越して後で適用」、`use_persisted` 対 `save`/`load`)は 1.3 のとおり出ている。counter と layout は素直に 2.6 倍と 1.9 倍。

**snapshot の結果。** 3 つとも同じ名前で一致した。

| example | 実測の差 | 許容 |
|---|---|---|
| counter | 124 px | 200 |
| todo | 26 px | 100 |
| layout | 750 px | 1000 |

`threshold` は egui_kittest の既定(0.6)のまま。許容は `max_failed_pixels`(ピクセル数)にした。ずれているのは文字の縁だけで、taffy は float で位置を決め egui は point 単位に丸めるので、共有する辺が 0.数 pt ずれるとラスタライズが 1px 動く。本物のレイアウト崩れは桁が違う(下記の背景バグは 11 万 px、`nested` の作り間違いは 5001 px)ので、この許容でも落ちる時は落ちる。

ここに来るまでに直したもの。

1. **背景の塗り面積**(4 万〜11 万 px)。生 egui 側の harness で `ui.set_min_size(ui.available_size())` を呼び、react-egui のルートと同じだけ場所を取らせる。
2. **`<TextEdit grow={1.0}>` がノードを埋めない**(todo、773 px)。taffy はノードを 321pt に広げるのに、`egui::TextEdit` は自分の既定 `desired_width`(280pt)で描くので中に 40pt の空きが残っていた。`react-egui-elements` の `TextEdit` を直した: `desired_width` が明示されておらず、かつ `cx.in_taffy()` なら `ui.available_width()` を渡す。Ui モードでは埋める相手が無いので egui の既定のまま。テストは `a_growing_text_edit_fills_its_node`(`w={400}` のノードで 400pt になる)。todo の差は 773 → 26 px になった。
   - `Slider` と `ComboBox` は同じ問題を持つ(400pt のノードで両方 100pt のまま。`spacing.slider_width` / `spacing.combo_width` が既定)。`Button` も伸びない(28pt)。今回は直さず記録だけ。`Slider` には幅の builder が無いので `ui.spacing_mut().slider_width` を触ることになり、`ComboBox` は `.width()` がある。
3. **生 egui 版の `nested` が 1 行になっていた**。`ui.allocate_ui` は親の左右レイアウトを引き継ぐので、列ごとに `allocate_ui_with_layout(.., Layout::top_down(..))` を使い、`ui.set_min_width` で幅を主張する(そうしないと確保が中身の幅まで縮む)。
4. **`grow` の計算を flexbox と同じにした**。最初は幅全体を 1:2 に割っていたが、`grow` が配るのは *余り* である。各列の中身の幅を測り、残りを 1:2 で足す。これで `right top` の x が 218.0 対 217.9 になった。
5. **justify セクションの縦の間隔**。`gap={4}` + 各行の `mb={4}` は「行間 8、最後の行のあとに 4」。egui は `add_space` の周りにも item_spacing を足すので、`item_spacing.y = 0` にして 8 と 4 を明示した。これで全セクションの y が完全に一致した。

`crates/react-egui-elements/tests/snapshots/` の 2 枚を撮り直した。`row.png` は手順 2.5 の wrap 修正のあと撮り直されておらず、`right` が 1 文字ずつ縦に並んだ**バグのままの絵**が commit されていた(snapshot は feature の裏なので、あの手順では回っていなかった)。`widgets.png` は上の `TextEdit` 修正でフィールドが広がったぶん。**feature 付きのテストは、その feature が触る変更のたびに手で回す必要がある。**

snapshot の生成は react-egui 側を先に撮る(`UPDATE_SNAPSHOTS=1 cargo test -p gallery --features snapshot react_egui`)。同名の 2 テストを同時に update すると同じファイルを取り合う。

todo の絵は空リストでは何も言えないので、撮る前に両方を同じ手順で動かす(`milk` / `eggs` を入れて 1 つ done にする)。`done (1)` は両方とも既定の閉じた状態。

**その他の判断。**

- gallery の生 egui 版は `use_state(cx, PlainState::default)` + `cx.leaf_fill(.., |ui| plain::ui(ui, state.bind()))`。`&mut *state` だと毎フレーム dirty になって repaint を要求し続け、kittest の `run()` が `max_steps` で落ちる。`bind()` は bind 付きウィジェットと同じ理由でここでも正しい。
- トグルはコード列に置き、両方の行数をその下に並べる(`"32 lines"` と `"84 lines plain"`)。example を選び直すと react-egui 版に戻る。
- `PlainState` は example ごとに違う型なので、`Running` と同じく `match` で分岐する(マクロ 1 つで 3 つ生成)。
- todo の永続化は `serde_json` で JSON を作る `save()` / `load()` にし、eframe の `Storage` は `plain_main.rs` が触る。`plain.rs` を egui + serde だけに保つため。
- layout の生 egui 版の最後のセクション(`Grid` / `Vertical`)は両方ほぼ同じ長さになる。egui 自身のコンテナを両側で使っているので当然で、これも正直に見せる。

### 手順 4(A-4)

CI、Pages、README。

- `ci.yml`: counter / fetch の 2 ステップを `for config in examples/*/Trunk.toml` のループ 1 つにした。glob は 5 つ(counter / fetch / gallery / layout / todo)に当たる。`examples/meta` は Trunk.toml を持たないので入らない。Actions の `run:` は既定で `bash -e` なので、ループ中の失敗はその場で止まる(ローカルで確認済み)。`--config` を渡す理由のコメントと fetch のコメントはループの上にまとめた。末尾の snapshot に関するコメントに gallery の分を足した。
- `pages.yml`(新規): `main` への push と `workflow_dispatch`。build ジョブが `trunk build --release --public-url /react-egui/ --config examples/gallery/Trunk.toml` して `upload-pages-artifact@v3` に `examples/gallery/dist` を渡し、deploy ジョブが `deploy-pages@v4`。`pages: write` / `id-token: write` は deploy ジョブだけに付け、トップレベルは `contents: read`。`concurrency: pages` は `cancel-in-progress: false`(公開されるのはビルドが完走したコミットであってほしいため)。
  - `dist = "dist"` は Trunk.toml からの相対なので、出力は `examples/gallery/dist` で正しい。ローカルで確認。
  - `--public-url` は生成される `index.html` の `<link href>` を `/react-egui/gallery-….js` に書き換えるだけである。`#todo` の直リンクは wasm の中で `location.hash` を読むので、`--public-url` とは無関係。両方ローカルで確認した。
  - 依存の apt install は ci.yml と同じものを入れた。wasm だけのビルドには要らないはずだが、deploy で確かめる話ではない。
  - **手作業が 1 回だけ残る**: リポジトリの Settings → Pages → Source を "GitHub Actions" にする。pages.yml の先頭コメントにも書いた。
- `README.md`: 「`examples/counter` verbatim」を直した(手順 1 で挙げた宿題)。スニペットは lib.rs のコンポーネントと main.rs の `run(..)` を合わせたものだと明記し、中身は現在のファイルから写した。Examples 節を表(name / what / live / source / plain egui)にして gallery へリンクし、`cargo run -p <name>`、`--bin <name>-plain`、`trunk serve`、`cargo run -p gallery <name>` の走らせ方を並べた。Testing 節に `cargo test -p gallery --features snapshot` と、同名比較を先に react-egui 側で撮る手順を足した。行数は README には書いていない(手順 3 のとおり todo が同数で、説明抜きでは誤解を招くため)。
- スクリーンショットは未挿入。`<!-- TODO: gallery screenshot -->` を置いてある。

## 8. PR B の記録

### 手順 5-1: form

設定フォーム。`Settings`(name / notify / autosave / volume / theme)を `use_persisted(cx, "form/settings", ..)` に置き、`TextEdit` `Checkbox` ×2 `Slider` `ComboBox` を全部 `bind` で繋ぐ。`on_change` は `use_state` の `Vec<String>` にログを積み、`Collapsing` で出す(直近 8 行、新しい順)。reset ボタンで既定値に戻す。要約行(`"anon, dark, volume 50"`)を出しているのでテストが読める。

- **`on_change` が新しい値を読めない件**。`bind` の要素は widget が state の `&mut` を握っているので、同じ要素のハンドラから同じ state は触れない(6 章の約束)。だから log に積むのは widget が payload で渡せるものだけになる: `Checkbox` は新しい `bool`、`ComboBox` は新しい index、`TextEdit` と `Slider` は `()` なので「name edited」「volume changed」としか書けない。これは制約であって不便でもあるが、`bind` の意味がそのまま出ている場所なので、そのまま見せてコメントに書いた。
- **`Slider` / `ComboBox` の幅は直さなかった**。手順 3 で見つけた「grow のノードでも 100pt のまま」は残っている。ただし設定フォームでは、ラベルの隣にウィジェットが自然な幅で並ぶのが普通で、横いっぱいに伸びた ComboBox はむしろ変である。だから form は `grow` を使わず、ラベル列に幅(90pt)を与えて揃える形にした。伸ばしたい example(list-10k あたり)が出てきたら、その時に `TextEdit` と同じやり方で直す。
- **snapshot は完全一致**(diff 0 px、許容も 0)。counter / todo / layout と違って 1px も違わない。react-egui 側は `<Field>` がラベルに `w={90}` を与える行、生 egui 側は `egui::Grid::new(..).min_col_width(90)`。どちらも「ラベル列を作る」ことを 1 行で言っている。
  - 最初は 503 px ずれた。生 egui 側で `ui.add_sized([200, interact_size.y], TextEdit)` と高さを固定していたためで、`TextEdit::singleline(..).desired_width(200.0)` にして egui に高さを決めさせたら 0 になった。
- 行数は **react-egui 158 / 生 egui 123 で、react-egui の方が長い**。理由は 2 つあり、どちらも正直に見せる価値がある。(a) `Settings` と `THEMES` と `META` は `lib.rs` にあり、`plain.rs` は `use crate::Settings` で貰っている。共有する型のぶんだけ `lib.rs` が重い。(b) egui の `Grid` はラベル列の整列をやってくれるので、`<Field>` コンポーネントを書く react-egui 側の方が手数が多い。**フォームは egui が元々得意な領域で、ここで react-egui が勝つ話にはならない。** 差が出るのは state の持ち方(1 つの struct を `&mut` で回す)、ログを「行を描く前に集めておく」必要があること、永続化を手で書くことの 3 点で、それは 1.3 の todo と同じ種類の差である。
- gallery 一覧では counter / todo の次(form / layout / fetch の前)に置いた。

### 手順 5-2: theme

`provide_context` / `use_context`。`Themed` が `Theme { dark }` と `Locale` を `use_handle` で持って children に配り、`Page` → `Card` → `Greeting` / `ThemedButton` / `Swatch` の 3 段下で `use_context` が読む。間の `Page` と `Card` は props をひとつも取らない。provider の直下の `Toggles` は `use_context` で読んだ `Handle` に `set` して書き戻す(React の `useTheme()` が値と setter を返すのと同じ形)。`Themed` の外に置いた `Orphan` は `use_context` が `None` になり「outside: no theme provided」と出す。

- **`<Provide value={handle}>` は書けない**。これが今回いちばんの発見。`provide_context` が取る `Handle<'s, T>` はストアを借りているが、`props_builder` はコンポーネントに `for<'a, 's, 'u> Fn(&'a mut Cx<'s, 'u>, P)` を要求するので props の型は `'s` を名乗れない。実際に書くと `implementation of Fn is not general enough` で落ちる(elements に置いて確認し、消した)。同じ理由で `view(|cx| provide_context(cx, handle, ..))` も通らない(`View::show` も `'s` について higher-ranked で、外側の `Handle` と繋がらない)。
  - 書ける形は「値を自分で作って自分で配る provider コンポーネント」。handle を内側の `cx` から作れば `'s` が一致するので、`#[component(shares_ui)] fn Themed(cx, children: impl View)` はそのまま通る。React でも provider が state を持つのが普通なので、実用上の不自由は無い。ARCHITECTURE 6 章に書いた。core を触れば直せる話ではあるが、今回は触らない。
- **言語は ja / en ではなく en / fr にした**。egui の同梱フォントは Hack / Ubuntu-Light / NotoEmoji / emoji-icon-font で、CJK のグリフが無い(`epaint_default_fonts` の中身を確認した)。日本語を出すと豆腐になる。フォントを読み込む話は別の example の仕事なので、ラテン文字 2 つに替えた。plan 2 章からの意図的な逸脱。
- **`ctx.set_visuals` は gallery 全体を塗り替える**。`set_visuals` は `egui::Context` 単位で、Context は 1 つしか無いため。plan 1.1 の埋め込みルールには反しないが(`Panel` でも `Instant` でもない)、gallery で theme を開いて light にすると gallery も light になる。egui の API がそうなっているだけなので、隠さずコメントに書いて受け入れた。
- snapshot は生 egui 版が無いので 1 枚だけ。`single!` マクロを `same!` の隣に足した(既定の dark 状態で撮る)。
- gallery 一覧では form の次、layout の前。

### 手順 5-3: clock

時計 + ストップウォッチ + `use_effect` の cleanup。

- 時間は全部 `ui.input(|i| i.time)`(アプリ起動からの秒、f64)。`std::time::Instant` は使わない。
- **壁時計は `web_time::SystemTime`**。`std::time::SystemTime::now()` は wasm32-unknown-unknown で panic する。`web-time = "1.1.0"` を `[workspace.dependencies]` に足した。タイムゾーンは持てない(タイムゾーンデータベースが要る)ので UTC と明記して出す。`HH:MM:SS` の整形は手書き、日付ライブラリは 1 行のために大きすぎる。
- **repaint は明示**。走っている間は `ctx.request_repaint()`、止まっている間は `ctx.request_repaint_after(1s)`。kittest の `run()` は「遅延なしの repaint 要求」が無くなるまで回るので、`request_repaint_after` は `run()` を止める(遅延が 0 でないため)が、`request_repaint()` は止めない。走行中のテストは `step()` を使う。
- **cleanup と `Dispatch`**。`show ticker` チェックボックスが `Ticker` を出し入れし、`Ticker` の `use_effect(cx, (), || { .. ; move || .. })` が返す閉包が cleanup になる。cleanup は保存されるので `'static` で、ログの state を借りられない。だから log は `use_reducer` に置き、`Dispatch<String>` を prop で渡す(`Dispatch` は `Clone + Send + 'static` なので prop にできる。`Handle` は 5-2 のとおりできない)。unmount のメッセージは sweep の中で送られ、次に reducer を訪れた時に適用されるので、「ticker unmounted」は 1 フレーム遅れて出る。
- `use_memo` は lap の整形文字列に使った(deps は `laps.len()`)。60fps で毎フレーム整形するのは無駄で、増減した時だけ作り直せばよいので、わざとらしくない。
- **snapshot を安定させた方法**。kittest には `harness.input_mut()` があり、`RawInput::time = Some(x)` を置けば egui の `i.time` は固定できる(`let time = new.time.unwrap_or(self.time + predicted_dt)`)。ただし固定できるのは egui の時計だけで、**壁時計の `SystemTime` には効かない**。そこで `App` に `now: Option<u64>`(UTC 深夜からの秒、`None` は実時計)を prop で持たせ、snapshot とテストが固定値を渡す。ストップウォッチは止まった状態で `00:00.00` なので何もしなくても安定していて、`i.time` の固定は結局不要だった。
  - `single!` はプロパティを渡せないので、clock の snapshot だけマクロを使わず手で書いた。
- テストは kittest の `Role::Label` のラベルが `Node::label()` ではなく `Node::value()` に入る(accesskit の仕様)ことに注意。ストップウォッチの表示を読むヘルパーで踏んだ。
- gallery 一覧では theme の次、layout の前。

### 手順 5-4: custom-hook

`#[hook]` で書いた 3 つの hook を、それぞれ 2 つのコンポーネントから呼ぶ。パッケージ名 `custom-hook` / lib 名 `custom_hook`。

| hook | 中身 | 呼ぶ側 |
|---|---|---|
| `use_debounce(cx, &str, f64) -> String` | `use_state` 3 つ(最新値 / 変わった時刻 / 落ち着いた値)。時刻は `i.time`。待っている間は誰も次のフレームを要求しないので、hook 自身が `request_repaint_after(残り)` する | `SearchBox` / `Mirror` |
| `use_previous<T>(cx, T) -> Option<T>` | `use_state((現在, 直前))` の 3 行 | `SearchBox`(落ち着いたクエリの 1 つ前)/ `Counter` |
| `use_window_size(cx) -> Vec2` | `cx.ctx().viewport_rect().size()`。state を持たない hook | `Responsive`(幅で row / column を切り替える)/ `SizeReadout` |

- **`#[hook]` の効き目がそのまま example になる**。`SearchBox` と `Mirror` は同じ `use_debounce` を呼ぶが、片方に打ち込んでももう片方の表示は動かない。`#[hook]` が `Location::caller()` で呼び出し位置ごとにスコープを切るためで、テストがそれを固定している。
- `egui::Context` に `screen_rect()` は無い。`viewport_rect()` を使う。
- **gallery に埋めると `use_window_size` は gallery の窓の大きさを返す**(中央の列ではなく)。hook の意味としては正しい(窓の大きさを聞いているので)が、埋め込みでは `layout: row` 側に倒れる。単体で動かすと窓を狭めて切り替わるのが見える。
- state を持たない `use_window_size` に `#[hook]` を付けるかは迷ったが、付けた。hook は「`Cx` から読む再利用可能な関数」であって、state の有無は本質ではない。あとで state を足しても呼び出し側が変わらない。
- snapshot の名前は lib 名に合わせて `custom_hook.png`(`single!` が `stringify!` するため)。example 名は `custom-hook`。
- **テストで `use_debounce` の時間を止められる**。`harness.input_mut().time = Some(t)` は `RawInput::take()` が `time` を保つので次のフレームにも残る。0.1 秒では `settled` が動かず、5.0 秒にすると追いつくところまで固定した。
- gallery 一覧では clock の次、layout の前。

### 手順 5-5: escape-hatch

生 egui への出口を 4 通り、節ごとに並べる。パッケージ `escape-hatch` / lib `escape_hatch`。

1. `{view(|cx| ..)}` — rsx の途中に置く普通のコード。hook も動く(スロットは行で keying される)。
2. `cx.leaf(&style, |ui| ..)` — 要素が無いウィジェット(`egui::ProgressBar`、`ui.color_edit_button_srgba`)を taffy の item として置く。`ItemStyle::default().w(..)` がそのまま効く。
3. painter — `allocate_exact_size` + `ui.painter()` でスパークラインを描く。値は `use_state(Vec<f32>)`、`sin(i * 0.7)` で決定的。
4. 入れ子の `Cx` — `ui.group(..)` の中で `Cx::new(store, ui, scope)` を作り、`cx.scope("inner", ..)` の中で hook を使う。`Cx::new` / `cx.store` / `cx.scope_id()` / `cx.scope` はすべて公開 API で、prelude から届く。**4 節は書ける**。

実装で 3 つ踏んだ。どれも example そのものより価値がある。

- **`cx.ui()` は `<View>` の中では「今いる場所」ではない**。taffy モードの `cx.ui()` は taffy ツリーを開始した `Ui` なので、そこに描くとレイアウトの外、ツリーの左上に出る(最初に書いた 1 節がまさにそうなり、見出しに重なった)。`Cx::ui` の doc に既に書いてあるとおり。読む(`visuals()`、`input()`)ぶんにはどこでも安全で、描くときは `cx.leaf` を使う。**これが `leaf` の存在理由そのもの**なので、1 節をその形に書き直し、module doc に罠として明記した。
- **`leaf_fill` はサイズを与えなかった軸で窓全体を取る**。egui_taffy は `infinite` な leaf の max-content をルート矩形の大きさとして返すため。`w` だけ与えた ProgressBar の leaf が高さ方向に窓いっぱいになり、下の節が窓の高さぶん押し下げられて、テストのクリックがビューポート外に落ちていた(egui は範囲外のポインタを無視する)。`w` と `h` の両方を与えて解決。ARCHITECTURE 6 章の「`<View>` の中の `ScrollArea` には `grow` か `h` を与える」と同じ話が、`leaf_fill` 全般に当たる。
- **`ui.spinner()` はテストと相性が悪い**。アニメーションするので毎フレーム repaint を要求し、`Harness::run()` が `max_steps` で落ちる。1 節から外してコメントに理由を書いた(`Suspense` の fallback で使うのは別で、あちらは待っている間だけである)。
- `egui::ProgressBar` は accesskit に何も出さない(`ProgressIndicator` の label も value も `None`)。読めるように隣に `<Text>{format!("progress {:.2}", ..)}</Text>` を並べ、テストはそれを見る。
- snapshot は `single!` に drive 関数を渡せる形(2 引数版が 3 引数版に展開される)を足し、「add sample」を 2 回押した状態で撮る。1 点だけではスパークラインが線にならない。
- gallery 一覧では custom-hook の次、layout の前。

### 手順 5-6: list-10k

長いリストの値段を正直に見せる。パッケージ `list-10k` / lib `list_10k`、生 egui 版あり。

**測った数字**(`cargo test --release -p list-10k --test bench -- --ignored --nocapture`。`Harness::step` を 20 フレーム、600x800、GPU 無しなので「1 フレームの CPU 側」。M4 Max)。

| 行数 | react-egui | 生 egui(`show_rows`) |
|---|---|---|
| 100 | 0.84 ms | 0.18 ms |
| 1,000 | 5.03 ms | 0.14 ms |
| 10,000 | 86.82 ms | 0.17 ms |

react-egui は全行を描く。`rsx!` の `for` は本物のループで、1 行が `<View>` + 子 3 つ、10k 行で taffy ノードが 4 万個になる。生 egui 版は `ScrollArea::show_rows` で見えている 15 行前後しか描かず、残りは高さの予約だけなので、行数を 100 倍にしても frame time が動かない。**この example は生 egui が勝つ。** 数字は README には書かない(ここと example の module doc にある)。

- **`<ScrollArea>` の仮想化 prop は足さなかった**。plan 5 章の候補だが、`<ScrollArea>` は children を `impl View` という不透明な閉包で受け取るので、`for` ループの中身を切り出すことができない。`rows={(count, row_height)}` を意味あるものにするには「index を受け取って View を返す閉包」を prop に取る別の要素が要る。→ **手順 5-8 でその要素(`<VirtualList>`)を足した。** この節の「生 egui が勝つ」という結論はそこで更新される。
- **既定の行数**。`DEFAULT_COUNT = 10_000`(名前どおり)。ただし gallery は `initial_count={1_000}` を渡す。10k だと 1 フレーム 85ms で gallery 全体が 12fps になり、「react-egui が遅い」と読まれてしまう。スライダーは 10k まで届くので、押したい人は押せる。生 egui 版も gallery では 1,000 に揃える(仮想化されているので 10k でも平気だが、トグルで行数が変わると比較にならない)。
- **snapshot は別名**(`list_10k_react.png` / `list_10k_plain.png`)。同名で撮ると 9,373 px ずれる。中身は同じリストだが、片方は全行を描き、片方は見えている 12〜14 行を描いて残りを予約するので、行の中の 3px 程度のずれが行数ぶん繰り返される。詰めるには生 egui 版を taffy の計算に合わせて書くことになり、5 章の線を越える。form / counter / todo / layout と違ってここは構造が違う。
- **kittest: `ScrollArea` の中のボタンは `click()` では押せない**。シミュレートしたポインタ押下がスクロール領域に吸われて widget に届かない。`click_accesskit()` なら効く。行の削除テストで踏んだ。
- ベンチは `#[ignore]` のテストとして置いた(`tests/bench.rs`)。release でしか意味が無く、アサーションでもないため。
- gallery 一覧では escape-hatch の次、layout の前。

### 手順 5-7: shell(と list-10k の索引列の手直し)

**list-10k の手直し。** 生 egui 版の索引列が中身の幅になっていて、名前の開始位置が react-egui 版と揃っていなかった。`INDEX_W`(64pt)を lib に出し、両方がそれを使う。生 egui 側は `allocate_ui_with_layout` + `set_min_width`(`add_sized` だと中央寄せになり、最小幅を言わないと中身まで縮む)。snapshot を撮り直した。別名のままである。

**shell.** IDE 風の枠。上 / 左 / 下の `<Panel>`、`<CentralPanel>` のエディタ、浮いた `<Window>` のインスペクタ、左のツリーは `<Collapsing>` + `selectable_label`。

**バグ: `<Panel>` がランナーの下で docking しなかった。elements で直した。**

- 原因。`Panel` は `shares_ui` だが中身は `cx.leaf(&style, ..)` で、taffy モードではノードが 1 つ作られてその中を切り取る。ランナーは必ず `root_container` を開くので、4 つのパネルが 4 つの小さなノードを切り取り、全部が同じ左上に重なって描かれていた(実測: `save` / `files` / `log` が全部 (16,10) 付近)。ARCHITECTURE 6 章の「パネルはアプリのルートで使うことを想定する」が、ランナーの下では成立していなかった。
- 修正(`react-egui-elements`)。**パネルが場所を切り取る先は「最も近い egui の `Ui`」= 今の taffy ツリーを開始した `Ui`** と決めた。taffy モードなら `cx.leaf` ではなく `cx.ui()` に対して `show_inside` する。ツリーの外(Ui モード)では今までどおり。`CentralPanel` も同じ。ランナーの下ではこの `Ui` は窓なので、「ルートで使う」が自動的に成り立つ。
- 帰結として、`<View>` の奥に書いた `<Panel>` はその行の一部ではなく窓の端まで飛ぶ。docking の意味そのものなので、doc コメントに「不具合ではない」と明記した。テストは `a_panel_inside_a_view_docks_in_the_window`(`<View grow>` の中の左パネルが窓の左端に着き、残りと重ならない)。既存の `panels_written_as_siblings_dock` はそのまま通る。
- **gallery には載せられない**(この規則の下でも変わらない)。gallery の中央列に置いても、パネルが切り取るのは gallery のツリーを開始した `Ui` = 窓全体だからである。実際に 1280x800 で試したとき、gallery 自身のラベルは `CentralPanel` に塗り潰されて消えた。plan 5 章の「試して成立すれば gallery に載せる」は **不成立**。standalone のままにする。

**ついでに塞いだ elements の穴 2 つ**(shell がどちらも要る)。

- `<Window>` に `default_pos` と `default_size`(`egui::Window` の同名メソッド、最初のフレームだけ)。無いと egui の既定位置(左上)でツールバーとツリーを覆う。インスペクタは開いた状態で始め、右下寄りに置いた。
- `<TextEdit>` に `rows`(→ `desired_rows`)。加えて taffy の中の `multiline` は `ui.add_sized(ui.available_size(), ..)` でノードを縦横とも埋める。最初は `desired_rows = available_height / row_height` にしたが、行単位でしか合わず端数がノードからはみ出したので `add_sized` にした。テストは `a_growing_multiline_text_edit_fills_its_node` と `an_explicit_row_count_wins`。

**もう一度踏んだ「auto なノードは中身で測られる」**。エディタを入れた `<View grow={1.0}>` は `grow` だけでは中身のサイズになり、中の `TextEdit` は自分の中身で測られるので、両者が「2 文字ぶんの幅」で合意して固定された。`<View w="100%" h="100%">` と確定値を与えて解決。ランナーのルート(手順 2.6)と同じ話が 3 度目である。**`grow` は余りの分配であって、確定サイズの代わりにはならない。**

- なお `h="100%"` はツリーのルート矩形に対する 100% で、`CentralPanel` の中身の高さよりわずかに大きい。エディタは数 pt はみ出すが egui が切るので見た目に問題は無い。テストはそれを踏まえて「log がエディタの上端より下」を見る。
- snapshot は gallery ではなく `examples/shell/tests/snapshots.rs` に、shell 自身の `snapshot` feature で置いた。gallery に載らない example の絵のために gallery が shell に依存するのは筋が悪く、wasm も太る。`cargo test -p shell --features snapshot`。
- `<Window open={..}>` には `inspector.bind()` を渡す。`&mut *inspector` だと毎フレーム dirty になって repaint が止まらない(`Checkbox` の `bind` と同じ理由)。
- **kittest の続報**。`ScrollArea` の中(5-6)だけでなく、**入れ子の taffy ツリーの中の widget 全般**にシミュレートしたポインタのクリックが届かない。ここではパネルの中の `<View>` に入れたツールバーのボタンがそうだった。一方、同じパネルの中でも `cx.leaf` で素の `Ui` に直接描いたツリー項目は `click()` で押せる。**規則: `<View>` の内側は `click_accesskit()`**。

### 手順 5-8: `<VirtualList>` と list-10k の 3 つ目

`ScrollArea` + `for` が全行を描くのは事実だが、「だから生 egui が勝つ」で終わらせるのは正しくない。**react-egui でも仮想化はできる。`<ScrollArea>` 経由ではできないだけである。** 要素を足して、list-10k をその比較に作り替えた。

**`crates/react-egui-elements/src/virtual_list.rs`**

```rust
#[component]
pub fn VirtualList(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    rows: usize,
    row_h: f32,
    render: impl for<'a, 's, 'u> FnMut(&'a mut Cx<'s, 'u>, usize),
)
```

中身は escape-hatch 4 節そのもので、`cx.leaf_fill` の中で `egui::ScrollArea::show_rows` を呼び、返ってきた `Ui` に `Cx::new(store, ui, scope)` を組み直して、`cx.scope(i, |cx| render(cx, i))` を見えている行にだけ回す。行ごとに scope に入るので、`for` + `key={i}` と同じく行が hook を持てる。

- **閉包 prop は書ける。ただし bound を明示すること**(3.3 の心配ごとへの答え)。`render: impl FnMut(&mut Cx, usize)` は通らない。`#[component]` の `ElideToPropLifetime` が prop の省略ライフタイムを props 構造体のものに書き換えるので、`impl Trait` の中に未宣言のライフタイムが現れて `use of undeclared lifetime name` になる。`impl for<'a, 's, 'u> FnMut(&'a mut Cx<'s, 'u>, usize)` と自分で書けば、書き換える対象が無いのでそのまま通る。`&mut dyn FnMut` に落とす必要は無かった。5-2 の `Handle` prop とは別の問題で、あちらは props の型が `'s` を名乗ってしまうのが原因だった。
- 制約は「全行が同じ高さ」。`show_rows` が測らずに範囲を出せる条件で、要素側では検査できないので doc に明記した。
- `leaf_fill` なので `grow` か `h` を与える(手順 5-5 の教訓)。
- テスト(`crates/react-egui-elements/tests/virtual_list.rs`): 10,000 行を 300pt の harness に置くと木に載るのは 15 行前後だけ、`row 9999` は存在しない。スクロールすると先頭行が消えて後ろの行が入る。

**list-10k の 3 つ目。** `Checkbox "virtualise"` で `<ScrollArea>` + `for` と `<VirtualList>` を切り替える。行は `<Row>` コンポーネント 1 つで、どちらの経路も同じものを描く。既定は off で、gallery が最初に見せるのは「全部描く値段」のまま。

| 行数 | `<ScrollArea>` + `for` | `<VirtualList>` | 生 egui `show_rows` |
|---|---|---|---|
| 100 | 0.88 ms | 0.36 ms | 0.16 ms |
| 1,000 | 5.06 ms | 0.28 ms | 0.13 ms |
| 10,000 | 78.04 ms | 0.27 ms | 0.17 ms |

`<VirtualList>` は行数に対して平らである。生 egui との差(0.27 対 0.17)は、画面に出ている 15 行ぶんの taffy ノードの値段で、これは react-egui を使うことの値段そのものだから、そのまま見せる。**結論は「生 egui が勝つ」ではなく「`for` で 1 万行書くのが高い。長いリストには専用の要素がある」に変わった。**

- テスト: 切り替えても filter と削除の挙動が変わらないこと、両方の経路が先頭 10 行に同じものを出すこと。
- ARCHITECTURE 6 章の要素表に `VirtualList` を足し、`ScrollArea` が全部描くことと使い分けを書いた。README の list-10k の 1 行も差し替え。

### 手順 5-9: showcase(PR B 最後)

ノートアプリ。他の example が 1 つずつ見せたものを、アプリらしい形で組み合わせる。model と reducer は `src/notes.rs`(egui を知らない平らな関数なので、`Ui` 抜きで読める)、UI は `src/lib.rs`。

- 永続化は todo と同じ「reducer が持ち、`use_persisted` が写す」形。reducer は他人のスロットに reduce できないので、2 行のミラーがその値段である。理由をコメントに書いた。
- `use_memo` の deps は `(search, (len, next_id), updated の XOR)`。`next_id` が「追加された」、`len` が「消された」、`updated` の XOR が「本文が編集された(= 並び順が動いた)」を表す。
- `provide_context` は theme と同じ `Themed` ラッパー。間の 2 つの列は何も持ち回らない。
- 設定ウィンドウの「clear all」は 2 段確認。rsx の途中の `if` 1 つで書ける。
- `Msg::Add` のあと `*selected = None` にして、新しいノートが自分で開くようにした(「選択が無ければ先頭を開く」規則が拾う)。

**踏んだこと。**

- **`Option<T>` の prop は「省略可能な prop」であって「Option を渡す prop」ではない**。`#[component]` が `strip_option` を付けるので setter は `T` を取り、`selected={current}`(`Option<u64>`)は型が合わない。`&Option<u64>` にすれば参照型なので strip されず、そのまま渡せる。
- **埋めるウィジェットの後ろに置いたものは画面外に出る**。`<TextEdit multiline grow>` は「あるだけの高さ」を自分の content として報告するので、同じ列でその後ろに置いた語数の行が窓の下に押し出された。`min_h={0}` でも直らない(押し出しているのは列の側)。語数の行をエディタの **前** に移して解決した。「埋める leaf は列の最後に置く」が実用上の規則である。
- 途中でディスクが一杯になり(`ld: write() failed, errno=28`)、`target/debug/incremental`(6.9GB)を消して続けた。このセッションで target が 27GB まで育っている。

**gallery の並び替え。** `showcase` を先頭にした(訪問者が最初に見るべきもの)。以下 counter / todo / form / theme / clock / custom-hook / escape-hatch / list-10k / layout / fetch。`Running` の `match` の既定は `<ShowcaseApp/>` に変え、`"counter"` の腕を明示した。gallery のテストが「最初は counter」を前提にしていたので直した(トグルのテストは、showcase に生 egui 版が無いので counter を選んでから確かめる)。README の表も同じ順にし、導入の一文に「まず showcase を見て、他は 1 つずつの話」と書いた。

## 9. PR C の記録

### 手順 6: `Options.setup`(と `wgpu` feature を置かない判断)

**3.1 の前提が間違っていた。「eframe は default(glow)のまま」は eframe 0.36 では成り立たない。** eframe 0.36.1 の `default` feature は `["accesskit", "default_fonts", "links", "wayland", "web_screen_reader", "wgpu", "winit/default", "x11"]` で、**`glow` は入っていない**。`Renderer::Glow` は `glow` feature が無いと存在すらせず、`Renderer::default()` は `Wgpu` を返す。つまり **このリポジトリは最初から wgpu で描いていた**。0.35 までとは逆で、今は glow の方が opt-in である。

そのため **`wgpu` feature は置かない**。一度は `wgpu = ["eframe/wgpu"]` を足したが、今日の eframe では何も変えない feature であり、API の雑音にしかならない。5 章の「wgpu を唯一の backend にするか」は、eframe 側が先に決めてくれた形になる。glow で動かしたい人は `eframe/glow` を明示する話で、それはこの crate の仕事ではない。判断の根拠は `crates/react-egui-app/Cargo.toml` のコメントと ARCHITECTURE 8 章に残した。

**WebGL fallback も何もしなくても入っている。** `eframe/wgpu` → `egui-wgpu/default` → `wgpu/webgl`。3.1 の「wasm は `wgpu` の `webgl` feature を on にする」は不要だった。`[workspace.dependencies]` には `wgpu = "30.0"`(eframe 0.36.1 が使う版)を pin だけしてある。shader example が pipeline を組むときに同じ wgpu へリンクするため。

**`Options.setup`** は 3.2 のとおり足した。型は `Option<Setup>`、`pub type Setup = Box<dyn FnOnce(&eframe::CreationContext<'_>)>`(clippy の `type_complexity` が生の型を蹴るので別名にした。API としてもこちらが読みやすい)。`ReactApp::new` の先頭で `take()` して呼ぶ。1 フレーム目に paint callback が追加されうるので、store を作るより前に走らせる。`ReactApp::new` の `options` 引数を `&Options` から `&mut Options` にし、native / wasm どちらの起動閉包も `options` を move で持って `take` する(閉包はどちらも 1 回しか呼ばれない)。

- テストは `crates/react-egui-app/src/lib.rs` の `#[cfg(test)] mod tests` に 1 つ、`setup` の既定が `None` であること。kittest は eframe を動かせないので、実際に wgpu で描かれることの確認は目視(`RUST_LOG=eframe=info`)に委ねる。
- ARCHITECTURE 7 章(`Options` の一覧と `setup`)と 8 章(バックエンドと WebGL fallback)を更新。
