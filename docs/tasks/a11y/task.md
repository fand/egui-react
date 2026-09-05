# タスク: a11y(web のアクセシビリティ、時期未定)

## 目的

web(wasm)で動く react-egui アプリを、スクリーンリーダーとキーボード操作の支援技術から使えるようにする。native では egui が AccessKit 経由で OS のアクセシビリティ API にウィジェットツリーを渡しているが、web ではそのツリーが捨てられている。描画は canvas のまま、アクセシビリティツリーだけを DOM に鏡写しにする(Flutter web の semantics 層と同じ方式)。

これは react-egui 固有の問題ではなく egui / AccessKit / eframe の未完成部分なので、成果は上流(AccessKit の web adapter、eframe の差し込み口)に出すことを前提にする。react-egui は「上流に入れば自動で恩恵を受ける」位置にあり、本タスクは試作でその形を確かめ、上流に持ち込むところまでを範囲とする。

## 背景(2026-09 時点、egui / eframe 0.36)

| 層 | 状態 |
|---|---|
| egui → AccessKit ツリー | ある。`Context::enable_accesskit()` を呼ぶと毎フレーム `PlatformOutput.accesskit_update: Option<TreeUpdate>` に差分が出る。web でも動く |
| AccessKit → OS(native) | ある。macOS / Windows / Unix(AT-SPI)/ Android。egui-winit が `enable_accesskit` を呼び、adapter に渡す |
| AccessKit → DOM(web) | **無い**。AccessKit に web adapter が存在しない |
| eframe web でツリーを受け取る口 | **無い**。`eframe/src/web/app_runner.rs` が `accesskit_update: _, // not currently implemented` と捨てている。`App::update` からは `FullOutput` に触れない |
| web の代替 | `web_screen_reader` feature(既定 on)。`Options.screen_reader = true` のときだけ、起きたイベントの説明文を `speechSynthesis` で読み上げる。ツリーもフォーカス移動も無い |
| ツリーの保持 | `accesskit_consumer` が `TreeUpdate` を適用して歩ける(`Tree::new` / `update_and_process_changes`、`Node::role() / label() / value() / bounding_box() / is_focused()`)。kittest が同じ経路を使っている |
| 逆方向 | 支援技術からの操作は `accesskit::ActionRequest`(Click / Focus / SetValue など)。egui は `Event::AccessKitActionRequest` として受け取って処理する |

react-egui の要素は egui の widget をそのまま使っているので、native の対応はそのまま享受している(kittest がラベルで要素を探せているのがその証拠)。

## スコープ

### 含む

- **調査**: AccessKit に web adapter の議論 / 実装が無いかの確認(issue、ブランチ、他プロジェクトの試み)。Flutter web の semantics 層の構造(要素の種類、フォーカス同期、イベントの戻し方、既知の弱点)を読む。
- **試作(react-egui 内で閉じる)**:
  - eframe を fork するか自前の web ランナーを書き、`accesskit_update` を受け取る。
  - `TreeUpdate` を `accesskit_consumer` で保持し、canvas の上に透明な DOM 要素(`role` / `aria-label` / `aria-valuenow` / 絶対座標)として並べる。
  - フォーカスの同期(egui → DOM の `focus()`、DOM → egui の `ActionRequest::Focus`)。
  - DOM の click / keydown / input を `ActionRequest` に変換して egui に戻す。
  - 対象ウィジェット: Button / Checkbox / Label / TextEdit / Slider / ComboBox(react-egui-elements の全ウィジェット)。
  - gallery で VoiceOver(macOS Safari / Chrome)から操作できることを目視。
- **上流化**: 試作で確かめた形を、AccessKit の web adapter(新 crate)と eframe の差し込み口(`WebOptions` へのコールバック等)として PR に切り出す。
- **react-egui 側の整備**(上流と独立に今できること):
  - 要素の `label` を必須に近づける(`Image` の代替テキスト、アイコンだけの `Button` の `aria-label` 相当)。
  - ARCHITECTURE.md 1 章の非ゴールに「web の a11y は egui / AccessKit の web 対応に依存する」と明記し、README の gallery の説明に一文を足す。

### 含まない

- **DOM で描画し直す backend**(egui を web では使わない)。reconciler を持たない設計(ARCHITECTURE 2.1)、ハンドラがその場で `&mut` を借りる設計、生 egui への出口のすべてと衝突する。それをやるなら Dioxus を使う。
- テキスト選択、ページ内検索、翻訳、SEO。canvas 描画の限界で、Flutter web も解決していない。
- native 側の改善(egui / AccessKit の範囲)。
- IME の改善(別問題)。

## 成果物

- `docs/tasks/a11y/plan.md`(調査結果を反映した詳細プラン。着手時に書く)。
- 試作コード(fork した eframe か自前ランナー、adapter の原型)。react-egui の main には入れず、ブランチか別リポジトリに置く。
- 上流への issue / PR(AccessKit、eframe)。
- ARCHITECTURE.md / README の一文。

## 終了条件

- gallery(web)の counter / todo / form を、VoiceOver でボタン名・チェック状態・テキスト欄の値が読み上げられ、Tab で移動し、Enter / Space で操作できる(目視)。
- 上流に web adapter と eframe の差し込み口の提案が出ている(マージは条件にしない)。
- ARCHITECTURE.md に web の a11y の現状と方針が書かれている。

## 決めごと(着手時点での前提)

- 方式は「canvas 描画 + DOM の鏡写し」。描画の置き換えはしない。
- adapter は egui 非依存(AccessKit の `TreeUpdate` だけを見る)で書き、AccessKit の他の adapter と同じ形にする。egui / react-egui 固有のものは入れない。
- 試作は react-egui-app の wasm ランナーで閉じて行い、動いたら上流に切り出す。react-egui の main に fork した eframe を依存として入れない。
- 優先度はフェーズ 8(公開準備)の後。それまでは README に制約を書くだけにする。

## 見積り

- adapter の試作(Button / Checkbox / Label / TextEdit、フォーカスとクリックの往復): 数百行、数日。
- 実用(Slider / ComboBox / リスト / live region / スクロール / IME との整合): Flutter web の semantics 層が数千行なのが目安。上流での作業。
