# progress: フェーズ 6.6(board + patch)の引き継ぎ

PR: https://github.com/fand/react-egui/pull/10(ブランチ `claude/complex-ui-demo-idea-lmigbs`、base `main`)
タスク定義: `docs/tasks/board/`、`docs/tasks/patch/`(各 plan.md の 8 章に実装で判明したことが全部ある)

## 状態(2026-09-06)

| 項目 | 状態 |
|---|---|
| board(`examples/board`、plain 版・gallery 登録・tests 込み) | 済 `6e8b8a8` |
| patch(`examples/patch`、gallery 登録・tests 込み) | 済 `12ba0fb` |
| `origin/main`(a11y PR #5)の merge | 済 `cdf4246`。conflict は `examples/gallery/src/main.rs` の `setup` 閉包のみ(shader/patch の `gpu::setup` と `WebA11y` plugin を両方呼ぶ形で解決) |
| `cargo fmt --check` / `clippy --workspace --all-targets -D warnings` / `check --target wasm32-unknown-unknown` | merge 後も緑 |
| `cargo test --workspace` | **1 件赤**: `gallery/tests/a11y.rs::every_focusable_widget_has_a_name`(下記「次にやること 1」) |
| `trunk build` / GPU snapshot / 実機での目視 | **未**(このコンテナには trunk も GPU も無い) |

## 次にやること

### 1. a11y テストを緑にする(必須。CI が赤)

main で入った `examples/gallery/tests/a11y.rs` は、全 example のフォーカス可能ノードに名前があるかを見て、無いものを `KNOWN_UNNAMED` と厳密比較する。board / patch が新しい無名ノードを足したので落ちる。出ているもの:

| example | role | 正体 | 直し方 |
|---|---|---|---|
| board | `TextInput` ×1 | 検索の `<TextEdit>`(`lib.rs:291`) | 要素では名前を付けられない(showcase / todo と同じ)。`KNOWN_UNNAMED` に `"board: TextInput"` |
| board | `Label` ×12 | カードのタイトル `egui::Label::new(..).sense(click_and_drag)`(`lib.rs:502`) | egui は `WidgetType::Label` のテキストを accesskit の **value** に入れ、label には入れない(`egui/src/context.rs` の `fill_accesskit_node_from_widget_info`)。sense 付きで focusable になるので無名扱い。`ui.add(label)` の後に `ui.ctx().accesskit_node_builder(response.id, \|n\| n.set_label(card.title.as_str()))`(main の `Button label` と同じ手)で名前を付ける。掴めるものなので role も `Button` に寄せてよい |
| patch | `Label` ×7 | ノードヘッダの名前ラベル(`lib.rs:725`、インスペクタ側も同じコンポーネント) | 同上 |
| patch | `MultilineTextInput` ×2 | 手書きの `SourceEdit`(`lib.rs:1077`) | 手書き leaf なので `accesskit_node_builder` で `"<node> shader source"` と名前を付けられる |
| patch | `ComboBox` ×2 | `MixParams` / `GrayParams` の `<ComboBox>`(`lib.rs:974`, `997`) | `label` prop を渡すと隣に文字が描かれる。form と同じ理由で `KNOWN_UNNAMED` にするか、`label="mode"` / `"method"` を描いてしまうか。どちらでも可、後者が親切 |
| patch | `Unknown` ×1 | パッチキャンバスの `leaf_fill`(`lib.rs:539`、`Sense::drag` でパン) | shader の `<Canvas>` と同じ性格。`response.widget_info(\|\| WidgetInfo::labeled(WidgetType::Other, true, "patch canvas"))` で名前を付けるか、shader に倣って `KNOWN_UNNAMED` |

`KNOWN_UNNAMED` は「木を歩く順」で厳密比較なので、足す場合は `EXAMPLES` の並び(showcase, board, patch, counter, ..)に合わせて挿す位置に注意。

### 2. GPU のある機械で確認する(必須。この環境では一切できていない)

```sh
cargo run -p board && cargo run -p board --bin board-plain
cargo run -p patch          # プレビューが動く / スライダで絵が変わる / Shader ノードの式を書き換えると再コンパイル / 結線のワイヤ
cargo run -p gallery board  # plain 切替
trunk serve --config examples/patch/Trunk.toml   # WebGL fallback でも naga → pipeline が通るか
UPDATE_SNAPSHOTS=1 cargo test -p gallery --features snapshot react_egui && cargo test -p gallery --features snapshot
```

board の snapshot 画像 2 枚(`board_react` / `board_plain`)は未生成。patch に snapshot は無い(shader と同じ扱い)。
特に見るべきは patch のワイヤ描画(`painter.add(Shape::Noop)` で予約したスロットに後から `set` する方式)と、`leaf_fill` の高さ問題(board plan §8.3)で列や右ペインが窓からはみ出していないか。

### 3. PR 本文

UI から作られた本文の末尾に claude.ai のセッションリンクが入っている。この repo の `CLAUDE.md` は commit / PR 本文への Claude 署名・セッションリンクを禁止しているので消す。また「Both examples include snapshot tests in the gallery」は誤り(patch には無い)。

### 4. 別 PR に切るライブラリ側の宿題(両 plan.md の 8 章より)

1. `use_keyed(cx, key, init)` / `Store::keyed_slot`: 位置に依らず identity に付く非永続 state。board は `Cx::new` でスコープを張り直す `use_identity` をユーザー land で書いて回避した(board §8.1)。`key=` は兄弟を区別するだけで、親をまたぐ移動では別スロットになる(React と同じ)。Compose の `movableContentOf` 相当。
2. `Handle::with_mut`(dirty にしない書き込み): 毎フレームの登録(DnD の slot、ポート位置)に必要。今は `Rc<RefCell<_>>` + `Handle::with` で回避(board §8.2、patch §8.2)。
3. `leaf_fill` が「残り」ではなく「ルート矩形の全高」を申告する: ツールバー + `<ScrollArea grow>` が窓からはみ出す。`showcase` の sidebar と gallery の左列も同じ形(board §8.3、patch §8.3)。ランナーのルートに窓の高さを固定する道が要る。
4. `<View>` が rect を返さない: 掴む・落とす・線を引く場所は全部 leaf にしないといけない(board §8.4、patch §8.2)。`on_rect` イベント案。
5. `bind` 要素の `on_change` に payload が無い(patch §8.4)。`TextEdit` の中身を報告させたい時に手書き leaf になる。
6. 行数で差を見せる主張は成立しない(board §8.6): コードのみ react 435 / plain 460。差は「`HashMap<CardId, CardUi>` とその `retain` を誰が書くか」。README / gallery の見せ方はそれに合わせる。

## 設計上の要点(引き継ぎ先が最初に読む 3 つ)

- **board**: `<Card>` の下書き・展開状態は `hooks::use_identity`(カード id にスコープを張り直した `use_state`)。`key=` では列をまたぐと消える。データは props で下ろし、`Dispatch` / テーマ / DnD セッションは context。undo / debounce / DnD は全部 `examples/board/src/hooks.rs` の custom hook で、ライブラリは無変更。
- **patch**: `Graph` は `topology_rev` と `param_rev` を別に持ち、WGSL 生成(+ naga 検証)は前者だけの `use_memo`、uniform 詰めは後者。pipeline は `CallbackTrait::prepare` でソースハッシュが変わった時だけ作り直す。ノードは `leaf_fill` の中でノードごとに `ui.scope_builder(max_rect)` → `Cx::new` → `cx.scope(node.id, ..)`(これが `key=` の正体)、ノード内部は普通の `<View>`。`use_dnd` / `use_undoable` は `board::hooks` を再利用(`patch` が `board` に依存)。
- **検証**: このコンテナはディスクが ~38 GB で debug ビルドが溢れるため、cargo は `CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0` を付けて回していた。手元の機械なら不要。
