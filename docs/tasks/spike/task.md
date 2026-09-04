# タスク: spike(PR1 = フェーズ 0 + 1)

## 目的

[docs/ARCHITECTURE.md](../../ARCHITECTURE.md) の設計が Rust の借用規則と egui の実行モデルの上で成立することを、マクロ抜きの手書きコードとテストで確認する。ここで前提が崩れた項目があれば、実装を進める前に ARCHITECTURE.md を修正する。同時に、以降の PR が乗る Cargo workspace と CI を用意する。

## スコープ

### 含む

- Cargo workspace(4 クレート + examples)、`rust-toolchain.toml`、CI、LICENSE、README の骨組み。
- `react-egui` core の最小実装: `Store`、`Cx`、`State`、`Handle`、`use_state`、`use_handle`、`use_effect`、`hook_scope`、Id 衝突検出、sweep、repaint ポリシー。
- `provide_context` / `use_context` の最小実装(検証項目のために必要な範囲のみ)。
- マクロが生成するはずのコードを手書きした Counter と Dialog(2 つの callback props、`Handler` trait、`Emitter`)。
- ARCHITECTURE.md 10 章の検証項目すべてを egui_kittest のテストとして固定する。
- 目視確認用の eframe 実行例 1 つ。

### 含まない

- `rsx!` / `#[component]` / `#[hook]` マクロ(フェーズ 3)。`react-egui-macros` は空クレートとして置くだけ。
- `use_memo` / `use_reducer` / `Dispatch` / `defer` / `update_later` / `use_persisted` / `use_future`(フェーズ 2 以降)。
- elements(`View` / `Text` / ウィジェットラッパー)とレイアウト属性(フェーズ 4)。egui_taffy は多重パスの検証にのみ使う。
- 衝突検出の画面オーバーレイ(フェーズ 2)。spike ではストアに記録して `log::warn!` するまで。
- wasm の実行確認。CI では `cargo check --target wasm32-unknown-unknown` まで。

## 成果物

- `Cargo.toml`(workspace)、`crates/react-egui`、`crates/react-egui-macros`、`crates/react-egui-elements`、`crates/react-egui-app`、`examples/spike`。
- `.github/workflows/ci.yml`。
- `crates/react-egui/tests/` 配下の kittest テスト群(検証項目ごとに 1 ファイル)。
- ARCHITECTURE.md の更新(前提が崩れた場合のみ)。

## 終了条件

- ARCHITECTURE.md 10 章の検証項目が全て kittest テストで緑。項目とテストの対応は [plan.md](plan.md) の表を参照。
- `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test --workspace`、`cargo check -p react-egui --target wasm32-unknown-unknown` が CI で通る。
- `cargo run -p spike` で Counter と Dialog が動く。
- 検証中に発覚した設計の修正点が ARCHITECTURE.md に反映されている。修正が無ければ「修正なし」と PR 本文に書く。

## 決めごと(着手時点での前提)

- ライセンスは `MIT OR Apache-2.0`(egui と同じ)。変更する場合は着手前に指示する。
- Rust toolchain は stable。`rust-toolchain.toml` の channel は egui_taffy 0.14 の MSRV 以上にする(README では 1.95 と記載)。
- スロットの安定アドレスには `elsa::FrozenMap` を使う。使いにくければ代替を選んでよいが、`State` の guard が生きている間に別スロットを挿入できることが条件。
