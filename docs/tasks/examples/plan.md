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
