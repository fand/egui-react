# タスク: board 改修(v2)

[board](../board/task.md) の example を、要望に沿って簡素で Trello に近い操作感に直す。詳細は [plan.md](plan.md)。

## 要望(原文の要約)

- card は title だけ。body は消す。
- "edit" はペンのアイコン。押すと title がその場で input になり、自動 focus + 全選択。Enter で確定、Esc でキャンセル。従来の編集画面(title / body / save / cancel)は消す。
- "+ card" を押すと、新規 card が edit 状態(title 空)で足される。
- card の左端の色ラベルをやめ、Done の checkbox にする。
- card に hover したら `cursor: pointer`。
- drag 中に、動かした範囲の背面のテキストが選択されるバグを直す。
- drag 中の挿入位置は青い線ではなく、挿入先に空の card を preview として出す。

## 含まない

- core / `egui-react-elements` の変更(必要なら plan.md 9 章に記録して別 PR)。
- アニメーション、複数ボード、ラベル編集 UI。

## 終了条件

- react 版と plain 版の両方が同じ操作で同じ結果(plan.md 8 章のテストが両方緑)。
- `cargo fmt --check` / `clippy --workspace --all-targets -- -D warnings` / `test --workspace` / `check --target wasm32-unknown-unknown` が緑。gallery の a11y テストも緑。
- `lib.rs` / `plain.rs` の冒頭コメント、`docs/tasks/board/task.md`、README の表が新しい仕様と一致している。
