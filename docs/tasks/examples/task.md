# タスク: examples(PR5 + PR6 + PR7 = フェーズ 6.5)

## 目的

examples を「動くサンプル 4 つ」から「ライブラリの売りを見せる場」にする。egui.rs と同じく全 example をブラウザで試せる gallery ページを持ち、そこで example と実装コードを並べる。同じ UI を生 egui で書いた版を隣に置き、状態管理とレイアウトの差をコードと行数で示す。足りない機能領域(フォーム、context、effect の cleanup、カスタム hook、生 egui への出口、大量要素、パネル構成、実アプリ)に example を 1 つずつ足す。最後に、コンポーネントの中で wgpu の shader を描く example を足し、そのために必要な最小の口(ランナーの `wgpu` feature、`Options.setup`、`<Canvas>` 要素)を開ける。

core(`react-egui`、`react-egui-macros`)には手を入れない。

## スコープ

3 PR に分ける。詳細は [plan.md](plan.md)。

### 含む

- PR5(gallery)
  - 既存 example 4 つ(counter / todo / layout / fetch)を lib + bin に分割し、`Meta`(名前、要約、hooks / elements タグ、ソース)を持たせる。
  - `examples/gallery`: 1 wasm。一覧(タグで絞り込み)/ 実行中の example / コード表示の 3 列。react-egui 版と生 egui 版のトグルと行数。`location.hash` で直リンク。
  - 生 egui 版(`plain.rs`): counter / todo / layout。
  - テスト: 各 example の kittest、react-egui 版と生 egui 版の snapshot 一致(gallery の `snapshot` feature)。
  - CI の trunk build を全 example のループにする。GitHub Pages へ gallery をデプロイする workflow。
  - README の examples を表にし、gallery へリンクする。
- PR6(examples)
  - form / theme / clock / custom-hook / escape-hatch / list-10k / shell / showcase。それぞれ kittest 付きで gallery に登録(shell は `Panel` を使うため単体 bin)。
  - form / list-10k は生 egui 版も書く。
- PR7(canvas)
  - `react-egui-app` に feature `wgpu`(eframe の wgpu backend)。on なら `Renderer::Wgpu` を既定にする。
  - `Options.setup: Option<Box<dyn FnOnce(&CreationContext)>>`。pipeline を `callback_resources` に置く場所。
  - `<Canvas>` 要素(`react-egui-elements`): taffy から矩形をもらい `on_paint(ui, rect)` を呼ぶ leaf。`on_drag` / `on_hover`。egui-wgpu には依存しない。
  - `examples/shader`: fullscreen triangle + fragment shader。state(speed / pause / drag)が uniform に流れる。native と trunk で動く。gallery に登録。

### 含まない

- core の変更。必要が出たら plan.md 8 章に書き、別 PR で扱う。
- `examples/template`(雛形)、`cargo generate`。フェーズ 8。
- `ScrollArea` の仮想化(`show_rows`)。list-10k で差が大きすぎる場合の追加候補として plan.md 5 章に置く。
- shader example の生 egui 版(差が出ない)。
- gallery のデザイン調整(フォント、配色)。動くことと読めることまで。
- 英語ドキュメント、API の見直し、crates.io 公開(フェーズ 8)。Android / iOS(PR4)。

## 成果物

- `examples/*/src/lib.rs` + `main.rs`(+ `plain.rs`)、`examples/gallery/`。
- 新規 example 9 つ(`form` `theme` `clock` `custom-hook` `escape-hatch` `list-10k` `shell` `showcase` `shader`)。
- `crates/react-egui-app`: feature `wgpu`、`Options.setup`。
- `crates/react-egui-elements/src/canvas.rs`(`Canvas`)。
- `.github/workflows/ci.yml`(trunk ループ)、`.github/workflows/pages.yml`。
- README の examples 表。`docs/ARCHITECTURE.md` の更新(plan.md 6 章)。`plan-overview.md` の PR 表。

## 終了条件

- plan.md のテスト(A-1〜A-4、各 example、C-1〜C-3)が緑。既存テストがすべてそのまま通る。
- `cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo check --workspace --target wasm32-unknown-unknown`、全 example と gallery の `trunk build` が CI で通る。
- `https://fand.github.io/react-egui/` で gallery が開き、全 example(shell を除く)が動き、コードが読め、生 egui 版へ切り替えられる(目視)。
- `cargo run -p <example>` が全 example で動く。`cargo run -p shader` で shader がアニメーションし、Slider で速度が変わる(目視)。`trunk serve` でブラウザでも同じ(目視)。
- ARCHITECTURE.md が実装と一致している。変更点は各 PR 本文に列挙する。

## 決めごと(着手時点での前提)

- gallery は 1 wasm。example ごとの別ページにはしない(切替の速さ、デプロイの単純さ)。
- gallery に載る example は「渡された領域を埋めるコンポーネント」。`Panel` / `CentralPanel` を使わない。`use_persisted` のキーは `"<example>/<key>"`。`std::time::Instant` を使わない。
- 生 egui 版は react-egui 版と同じ見た目を目指し、同じ snapshot 名で比較する。1px の丸め差が出たら閾値で吸収し、無理なら別名にする(plan.md 5 章)。
- コード表示は `egui_extras::syntax_highlighting`(`syntect` なし)。
- wgpu は eframe の `wgpu` feature 経由。glow は残す。`Options.setup` で pipeline を作り、hook / context に wgpu の型を出さない。`Canvas` は egui-wgpu を知らない。
- web の wgpu は WebGL fallback を有効にする。
