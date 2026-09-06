# 開発計画

設計の決定事項は [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) を参照。本書は作業の分割と進め方を定める。

## 進め方

- 複数のフェーズをまとめて 1 つの PR で進める。PR とフェーズの対応は下表のとおり。
- 各 PR は次の手順で進める。
  1. `docs/tasks/<name>/task.md` にタスク定義(目的、スコープ、終了条件)を書く。
  2. `docs/tasks/<name>/plan.md` に詳細プラン(作業項目、確認方法、テスト)を書く。
  3. docs をコミットする。
  4. 実装作業は Opus5 subagent に行わせる(`/sub` コマンド)。subagent は task.md と plan.md に沿って実装する。
- 実装中に設計上の前提が崩れた場合、コードより先に `docs/ARCHITECTURE.md` を更新する。プランが変わった場合は plan.md を更新する。
- CI(fmt / clippy / test / `wasm32-unknown-unknown` の check)は PR1 から回す。
- egui_kittest によるテストは PR1 から書き、以降のフェーズで同じテストが通り続けることを確認する。

## PR とフェーズの対応

| PR | フェーズ | タスク定義 |
|---|---|---|
| PR1 | 0, 1 | `docs/tasks/spike/` |
| PR2 | 2, 3, 4, 5 | `docs/tasks/core/` |
| PR3 | 6 | `docs/tasks/async/` |
| PR4 | 7 | `docs/tasks/mobile/` |
| PR5, PR6 | 6.5 | `docs/tasks/examples/` |
| PR7 | 6.5(canvas) | `docs/tasks/canvas/` |
| PR8 | 6.6 | `docs/tasks/board/`, `docs/tasks/patch/` |

フェーズ 8(公開準備)は PR4 の後に別途計画する。web のアクセシビリティ(`docs/tasks/a11y/`)はその後の候補で、時期未定。

## フェーズ

### フェーズ 0: ワークスペース準備

- Cargo workspace に `egui-react` / `egui-react-macros` / `egui-react-elements` / `egui-react-app` と `examples/` を作る。
- egui 0.36、rstml 0.13、egui_taffy 0.14 を pin する。
- `rust-toolchain.toml`、CI、LICENSE、README の骨組み。

### フェーズ 1: スパイク

- `egui-react` core に最小限の `Store` / `Cx` / `State` / `use_state` / `use_effect` / `hook_scope` / sweep を書く。
- マクロなしで Counter と Dialog(2 つの callback props)の手書き展開形を examples に置く。
- ARCHITECTURE.md 10 章の検証項目を egui_kittest のテストとして 1 つずつ固定する。
- 終了条件: 検証項目が全てテストで緑。崩れた項目があれば設計を修正し ARCHITECTURE.md を更新する。

### フェーズ 2: core hooks

- `use_memo` / `use_reducer` + `Dispatch` / `provide_context` + `use_context` / `defer` + `update_later`。
- repaint ポリシーの実装。Id 衝突検出の警告表示。

### フェーズ 3: マクロ

- `#[component]`: Props 構造体、children、イベント enum、`Emitter`。
- `#[hook]`: `#[track_caller]` と `hook_scope` の付与。
- `rsx!`: rstml でパース。要素、式埋め込み、`if` / `for` / `match`、`key`、`on_*` 融合、`events=` escape hatch、共通レイアウト属性の抽出。
- trybuild でコンパイルエラーの文面を固定する。スパイクの手書き展開形をマクロ版に置き換え、同じテストが通ることを確認する。

### フェーズ 4: elements とレイアウト

- `<View>` / `<Text>` を egui_taffy 上に実装し、レイアウト属性を taffy style に変換する。
- Button / Label / TextEdit(`bind`)/ Checkbox / Slider / ComboBox / Image / Separator。
- ScrollArea / Collapsing / Frame / Window / Panel 群。egui-native の Vertical / Horizontal / Grid。
- kittest スナップショットで見た目を固定する。

### フェーズ 5: ランナーと examples

- `egui-react-app::run`。`Options::max_passes = 2` の設定。`use_persisted`。
- native と trunk による wasm ビルドを CI で回す。
- examples: counter、todo(`use_reducer`)、layout デモ。

### フェーズ 6: 非同期

- `use_future`(native は thread / tokio、wasm は wasm-bindgen-futures)。完了時の `request_repaint`。
- fetch example。

### フェーズ 6.5: examples と gallery

- 既存 example を lib + bin に分割し、全 example をブラウザで試せる gallery(1 wasm)を GitHub Pages に置く。
- gallery で example と実装コードを並べ、生 egui 版と切り替えて差を見せる。
- examples を足す: form、theme、clock、custom-hook、escape-hatch、list-10k、shell、showcase。
- `egui-react-app` の `wgpu` feature と `Options.setup`、`<Canvas>` 要素、shader example。

### フェーズ 6.6: 複雑な UI の example

既存 example が示していない React の利点 — 動的に増減・並べ替えされる要素が各々ローカル state を持つこと、その state が key に付いて回ること、自作コンポーネントと custom hook で組み上げられること — を 2 つの example で示す。

1 PR にまとめる。board を先に仕上げ、その custom hook が固まってから patch に入る。

- board: Trello 風のカンバン。DnD、undo、debounce を custom hook として書き、生 egui 版と並べて差を出す。
- patch: TouchDesigner 風のノードエディタ。シェーダノードを繋ぐと 1 本の WGSL が生成され、プレビューが変わる。board の custom hook を再利用する。生 egui 版は書かない。

### フェーズ 7: モバイル

- Android: eframe で examples をビルドする。
- iOS: `egui-winit` + `egui-wgpu` のランナーを `egui-react-app` に書き、cargo-mobile2 でビルドする。
- タッチ / IME / safe area の調整。

### フェーズ 8: 公開準備

- ドキュメント(英訳を含む)、API の見直し、crates.io への公開。
