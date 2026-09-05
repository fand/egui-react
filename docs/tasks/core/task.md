# タスク: core(PR2 = フェーズ 2 + 3 + 4 + 5)

## 目的

spike(PR1)で検証済みの core の上に、ユーザーが実際に書く API を揃える。hooks の残り(`use_memo` / `use_reducer` / `defer` / `update_later` / `use_persisted`)、3 つのマクロ(`rsx!` / `#[component]` / `#[hook]`)、elements とレイアウト、ランナーである。終わった時点で examples が native と wasm で動き、以降の PR(非同期、モバイル)がランナー層と hook 1 つの追加だけで済む状態にする。

spike の手書き展開形はすべてマクロ版に置き換え、spike のテストがそのまま通り続けることでマクロの展開形を固定する。

## スコープ

### 含む

- フェーズ 2(core hooks)
  - `View` trait と `rsx!` の戻り値になる closure View。
  - `use_memo` / `use_reducer` + `Dispatch` / `cx.defer` + `update_later`。遅延キューの適用順。
  - `Cx` にレイアウトコンテキスト(egui `Ui` か egui_taffy `Tui` か)を持たせ、`leaf` / `container` で描画先を切り替える。
  - Id 衝突検出の画面オーバーレイ(debug ビルド)。
- フェーズ 3(マクロ)
  - `#[component]`: Props 構造体 + builder、`children`、`#[event]` からイベント enum と `Emitter`。
  - `#[hook]`: `#[track_caller]` と `hook_scope`。
  - `rsx!`: rstml でパース。要素、式埋め込み、文字列リテラル、`if` / `else` / `for` / `match`、`key`、`on_*` の融合、`events=` escape hatch、共通レイアウト属性の抽出。
  - trybuild でコンパイルエラーの文面を固定する。
- フェーズ 4(elements とレイアウト)
  - `react-egui-elements`: `<View>` / `<Text>` を egui_taffy 上に実装し、レイアウト属性を taffy style に変換する。
  - ウィジェット: `Button` / `Label` / `TextEdit`(`bind`)/ `Checkbox` / `Slider` / `ComboBox` / `Image` / `Separator`。
  - コンテナ: `ScrollArea` / `Collapsing` / `Frame` / `Window` / `SidePanel` / `TopBottomPanel` / `CentralPanel`。egui-native の `Vertical` / `Horizontal` / `Grid`。
  - kittest の操作テストと、レイアウトのスナップショットテスト。
- フェーズ 5(ランナーと examples)
  - `react-egui-app::run(Options, |cx| rsx!{..})`。native と wasm を同じ関数で吸収する。`Options::max_passes = 2` の明示設定。
  - `use_persisted`(eframe の `Storage` に保存)。
  - examples: `counter`、`todo`(`use_reducer`)、`layout`。`examples/spike` は削除する。
  - CI に wasm の `cargo check --workspace` と trunk ビルドを足す。

### 含まない

- `use_future`(PR3)。`Dispatch` が `Send + 'static` であることだけ本 PR で保証する。
- Android / iOS(PR4)。英語ドキュメント、API の見直し、crates.io 公開(フェーズ 8)。
- taffy Grid の詳細なトラック指定(`minmax`、`auto-fill`、名前付き領域)。`display="grid"` と等幅カラム、`col_span` / `row_span` までにとどめる。
- `Image` のローダー登録(`egui_extras`)。`Image` は `egui::ImageSource` を受け取るだけで、ローダーはアプリ側の責務とする。
- `use_persisted` の wasm 側(localStorage)の自動テスト。native の `Storage` 相当のモックでテストし、wasm は目視まで。
- スナップショットテストを CI の必須ステップにすること。wgpu のソフトウェアレンダリングが CI で安定しなければ、feature の裏に置いてローカル実行にとどめる(下記「決めごと」)。
- `rsx!` の IDE 補完やフォーマッタ対応。

## 成果物

- `crates/react-egui`: `view.rs`(`View`)、`hooks.rs` の追加分、`dispatch.rs`、`layout.rs`(`ItemStyle` / `ContainerStyle` / `Length`)、`Cx` の拡張、`Store` の遅延キューと永続化、衝突オーバーレイ。
- `crates/react-egui-macros`: `component.rs` / `hook.rs` / `rsx/`(パーサ、カスタムノード、展開)。
- `crates/react-egui/tests/ui/`(trybuild)と `crates/react-egui/tests/` の追加テスト。spike のテストと `tests/common` はマクロ版に置き換える。
- `crates/react-egui-elements`: 各要素と `tests/`、スナップショット画像。
- `crates/react-egui-app`: `run` / `Options`、eframe の `App` 実装、wasm ランナー。
- `examples/counter` / `examples/todo` / `examples/layout`(それぞれ `index.html` と `Trunk.toml` を含む)。
- `.github/workflows/ci.yml` の更新。
- `docs/ARCHITECTURE.md` の更新(着手時点で判明している変更点は [plan.md](plan.md) 7 章、実装中に判明したものはその都度)。
- README の使い方セクション(counter の例 1 つ)。

## 終了条件

- [plan.md](plan.md) の各フェーズのテスト表がすべて緑。spike のテスト(`sibling_handlers` / `custom_hook` / `fused_events` / `context_handle` / `multi_pass` / `collision` / `unmount` / `nested_ui` / `repaint` / `effect_deps`)がマクロ版のコンポーネントで通る。
- trybuild テストが緑で、`.stderr` がコミットされている。
- `cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace`、`cargo check --workspace --target wasm32-unknown-unknown`、`trunk build` が CI で通る。
- `cargo run -p counter` / `cargo run -p todo` / `cargo run -p layout` が native で動き、`trunk serve` で counter がブラウザで動く。
- `todo` を終了して再起動すると `use_persisted` の内容が残っている(native、目視)。
- ARCHITECTURE.md が実装と一致している。変更点は PR 本文に列挙する。

## 決めごと(着手時点での前提)

- 4 フェーズを 1 PR にまとめるが、コミットはフェーズ単位(最低 4 つ)に分け、各フェーズの終わりで CI が緑になっていること。フェーズをまたいで壊れた状態のコミットを積まない。
- `react-egui`(core)は `egui_taffy` に依存する。`Cx` がレイアウトコンテキストを持つ以上、core が `Tui` を知る必要があるため。wasm の `cargo check` は引き続き通すこと。
- Props の builder は `typed-builder` crate を使い、`react-egui` から `__private` で再エクスポートする(`#[builder(crate_module_path = ..)]`)。自前生成に切り替えるのは、再エクスポート経由で動かない場合だけ。
- `#[component]` の Props 構造体は `<Name>Props`。`rsx!` は `::react_egui::props_builder(&Name)` で関数の型から builder を引く(ユーザーは `Name` だけを `use` すればよい)。lifetime 付き props で推論が通らなければ [plan.md](plan.md) 6 章の代替案に切り替える。
- スナップショットテストは egui_kittest の `snapshot` + `wgpu` feature が要る。`react-egui-elements` の cargo feature `snapshot` の裏に置き、CI では `mesa-vulkan-drivers` を入れて別ステップで回す。2 回試して安定しなければそのステップを外し、PR 本文に書く。
- egui 0.36 の `Options::max_passes` の既定値は既に 2 である。ランナーは明示的に設定するが、これは将来の egui の変更に対する固定であって挙動の変更ではない。
- `update_later` / `defer` に渡す閉包は `'static`(`move` が必要)。パス末まで生きるキューに入るため。借用したい場合は `Dispatch` か値の clone を使う。
- `use_reducer` のメッセージは「パス末」ではなく「次に hook を訪問した時」に適用する(理由は plan.md 1.3)。
- `use_persisted` の Id はスコープではなく文字列キーだけから導出する。同じキーを 2 か所で使えば同じ状態を共有する。保存形式は JSON、eframe の `Storage` には `"react_egui"` の 1 キーにまとめて書く。
- 依存の追加: `syn` 2 / `quote` / `proc-macro2` / `typed-builder` / `trybuild` / `serde` / `serde_json` / `wasm-bindgen-futures` / `web-sys`。バージョンは着手時の最新を `[workspace.dependencies]` に pin する。
