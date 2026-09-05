# プラン: board

タスク定義は [task.md](task.md)。設計の根拠は [docs/ARCHITECTURE.md](../../ARCHITECTURE.md)。実装中にここから外れる判断をした場合は本書を更新し、設計上の意味があれば ARCHITECTURE.md も更新する。

## 0. 全体

1 PR。触るのは `examples/board`(新規)、`examples/gallery`(登録)、README、必要なら `crates/react-egui-elements` と ARCHITECTURE 6 章。core は無変更。

```
examples/board/src/
  lib.rs        App と画面のコンポーネント(gallery が読むのはこれ)
  board.rs      データモデル、Msg、reduce、絞り込み(純関数、ユニットテスト付き)
  hooks.rs      use_undoable / use_debounced / use_dnd
  plain.rs      同じ UI の生 egui 版
  main.rs       run(..) の bin
  plain_main.rs 生 egui 版の bin
```

`board.rs` と `hooks.rs` を分けるのは `custom-hook` / `showcase` と同じ形。gallery が `include_str!("lib.rs")` で見せるのは `lib.rs` だけなので、**画面の組み立ては全部 `lib.rs` に置く**。読者が 1 ファイルで「React の書き方で UI を組む」を追えることを優先する。

## 1. データモデルと reducer(`board.rs`)

```rust
pub type CardId = u64;
pub type ColumnId = u64;

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Board {
    pub columns: Vec<Column>,
    next_id: u64,
    /// `use_memo` の deps に混ぜる版数。`reduce` が中身を変えた時だけ +1 する。
    pub rev: u64,
}

pub struct Column { pub id: ColumnId, pub name: String, pub cards: Vec<Card> }
pub struct Card { pub id: CardId, pub title: String, pub body: String, pub label: Label }
pub enum Label { None, Red, Yellow, Green, Blue }

pub enum Msg {
    AddCard { column: ColumnId },
    EditCard { card: CardId, title: String, body: String },
    SetLabel { card: CardId, label: Label },
    RemoveCard { card: CardId },
    /// 列間の移動と列内の並べ替えを 1 つで扱う。`to_index` は移動先の列から
    /// 自分を取り除いた後の添字。
    MoveCard { card: CardId, to_column: ColumnId, to_index: usize },
    RenameColumn { column: ColumnId, name: String },
}

pub fn reduce(board: &mut Board, msg: Msg);
/// 検索語とラベルで絞った、その列に見えるカードの id。
pub fn visible(column: &Column, search: &str, label: Option<Label>) -> Vec<CardId>;
```

`MoveCard` を 1 メッセージにするのが要点。「列から抜いて別の列の位置に挿す」が 1 手なら undo も 1 手になり、DnD の途中状態を reducer に持たせずに済む。

初期データは 4 列 12 枚程度をハードコードした `Board::demo()`。gallery で開いた直後に何か見えている必要がある。

## 2. コンポーネント構成(`lib.rs`)

```
App
└ BoardProvider            テーマと Dispatch を provide_context する(theme example と同じ形)
  └ View column
    ├ Toolbar              検索 TextEdit / ラベル絞り込み / undo / redo / dark
    └ View row grow        列を横に並べる
      └ Column (key=id)    名前のインライン編集、件数、"+ card"、ScrollArea
        └ Card (key=id)    ★ ローカル state を持つ本体
          ├ Chip           ラベル色の小片(style を受け取るだけの葉)
          └ IconButton     × と ▸(children と on_click を持つ最小の自作要素)
```

- **データは props で下ろし、`Dispatch` とテーマは context で配る。** `use_reducer` が返す `State<'s, Board>` は `'s` を持つので `Handle<T>` には入れられない(ARCHITECTURE 6 章)。一方 `Dispatch` は `Clone + Send + 'static` なので `use_handle(cx, || ctx)` に入れて配れる。結果として React で普通に採る形(state は props、更新関数は context)にそのままなる。この理由は `lib.rs` のコメントに書く。
- `<Card>` が持つローカル state はこの example の主張そのものなので、必ずカード自身の `use_state` に置く。

```rust
#[component]
fn Card(cx: &mut Cx, card: &Card, #[prop(default)] style: ItemStyle, #[event] on_move: DropTarget) {
    let mut editing = use_state(cx, || false);
    let mut draft_title = use_state(cx, || card.title.clone());
    let mut draft_body = use_state(cx, || card.body.clone());
    let mut expanded = use_state(cx, || false);
    ...
}
```

- `for` の中の `<Card key={card.id} .. />` / `<Column key={column.id} .. />` は key 必須(ARCHITECTURE 3.2)。**key を id にしていることが B-2 / B-3 のテストが通る理由**であり、コメントで明示する。
- `bind` を持つ要素(`TextEdit bind={draft_title.bind()}`)に同じ state を触る `on_change` を渡すと E0502 になる(ARCHITECTURE 6 章)。編集確定は `on_submit` かボタン側で `dispatch.send(Msg::EditCard { .. })` する。
- 列は `<View grow={1.0} w={0.0}>` で等幅に分配し、中のカードリストは `<ScrollArea grow={1.0}>`(`leaf_fill` なので高さを与える)。
- `use_memo(cx, (column.id, board.rev, &*search, label), || visible(..))` で列ごとの表示カードを作る。deps に `board.rev` を入れるのは `Board` 全体をハッシュしないため。

## 3. ドラッグ & ドロップ

最初に試す形(A):

- カードは `<View>`(taffy)で組み、**ヘッダ行を掴む場所**にする。ヘッダは `cx.leaf` の中に落ちるので `&mut egui::Ui` があり、`ui.dnd_drag_source(id, CardId, ..)` がそのまま使える。ゴーストの描画も egui が持っている。
- ドロップ先の判定は自前。各カードのヘッダの `Response::rect` と列の rect を、そのフレームのあいだだけ `use_dnd` が返すハンドルに登録する(`dnd.slot(rect, DropTarget { column, index })`)。ポインタが離れたフレームで、ポインタ位置を含む slot を選び `on_move` を発火する。
- 挿入位置のインジケータ(カードとカードの間の細い線)は、ドラッグ中に選ばれている slot の rect から `cx.ui().painter()` で引く。

`dnd_drag_source` が taffy の leaf の中で素直に動かない場合(B):

- `DragAndDrop::set_payload` + `ui.interact(rect, id, Sense::drag())` に落とし、ゴーストは `egui::Area` を 1 つ開いて自分で描く。判定側(slot)は A と同じなので影響は局所。

どちらでも DnD は **example の中の custom hook + 自作コンポーネントとして書け、ライブラリに手を入れない**。ここが崩れる(= `<View>` の rect が取れないと成立しない等)場合だけ、elements か `Cx::container` への最小の追加を検討し、8 章に記録して別途判断する。

`use_dnd` の形:

```rust
/// 掴んでいるものと、そのフレームに登録されたドロップ先。`Handle` なので
/// context に載せられ、列とカードが同じものを見る。
pub struct Dnd<T> { dragging: Option<T>, slots: Vec<(egui::Rect, T)>, .. }
#[hook] pub fn use_dnd<T: 'static>(cx: &mut Cx) -> Handle<'_, Dnd<T>>;
```

## 4. custom hooks(`hooks.rs`)

```rust
/// `use_reducer` を履歴で包む。`Undoable::Do(msg)` は past に present を積み、
/// future を捨てる。Undo / Redo は 3 つのスタックを回すだけ。
pub enum Undoable<M> { Do(M), Undo, Redo }
#[hook] pub fn use_undoable<S: Clone + 'static, M: 'static>(
    cx: &mut Cx, reduce: impl Fn(&mut S, M), init: impl FnOnce() -> S,
) -> (State<'_, History<S>>, Dispatch<Undoable<M>>);

/// 入力が `delay` 秒静まってから値を返す。`ctx.input(|i| i.time)` と
/// `request_repaint_after` だけで書く(wasm があるので `Instant` は使わない)。
#[hook] pub fn use_debounced(cx: &mut Cx, value: &str, delay: f64) -> String;
```

`use_undoable` が `use_reducer` の上に素直に乗ることが「undo をライブラリ機能にしなくてよい」の根拠になる。`History<S>` は `Serialize` にして `use_persisted` へ渡すのは `present` だけにする(履歴は保存しない)。

`use_reducer` のメッセージ適用は「次に hook を訪問した時」(ARCHITECTURE 4 章)なので、`send` の直後に `*state` を読んでも古い。ハンドラの中で読み直さない書き方にする。

## 5. 生 egui 版(`plain.rs`)

```rust
pub struct PlainState {
    board: Board,                       // 同じ board.rs を使う
    ui: HashMap<CardId, CardUi>,        // ← 差はここ
    history: Vec<Board>, future: Vec<Board>,
    search: String, search_debounce: Option<f64>, label: Option<Label>,
    drag: Option<(CardId, egui::Vec2)>, slots: Vec<(egui::Rect, DropTarget)>,
    dark: bool,
}
struct CardUi { editing: bool, draft_title: String, draft_body: String, expanded: bool }
pub fn ui(ui: &mut egui::Ui, state: &mut PlainState);
```

- 公平に書く。添字ではなく `CardId` をキーにし、**カードが消えた時に `ui` から掃除する**行も書く(これを書かないとリークする、というのが react-egui 版で sweep が担うもの)。
- レイアウトは `ui.columns(4, ..)` + `ScrollArea` で taffy 版と同じ絵にする(`layout` example の plain 版と同じ方針)。
- DnD と undo は react 版と同じ挙動にする。ロジックは `board.rs` を共有するので、差は「状態をどこに置くか」だけになる。これがこの example の見せ場なので、`plain.rs` の冒頭コメントに「共有しているもの / していないもの」を書く。

## 6. テスト(`tests/board.rs`)

`todo` / `form` と同じく react 版と plain 版の 2 つの harness を組み、同じヘルパで両方を叩く。

- **B-1** カードを追加すると、その列の件数が増える。
- **B-2(目玉)** カードの編集を開いて下書きを打ち、そのカードを別の列へ動かす。移動後もそのカードが編集中で、下書きが残っている。動かした先の隣のカードは編集中になっていない。
- **B-3(目玉)** 列内で 2 枚目を先頭へ動かす。展開していたカードだけが展開されたままで、位置ではなくカードに状態が付いていることを確認する。
- **B-4** undo / redo。追加 → 移動 → undo ×2 → redo で元に戻る。
- **B-5** 検索。debounce の待ち時間はハーネスで時間を進めて越える。絞り込みが列をまたいで効く。
- **B-6** 永続化。`Store::save_persisted` → 新しい `Store` に `load_persisted` → 同じボードが出る(`showcase` のテストに倣う)。
- **B-7** react / plain に同じ操作を流して同じ結果になる。plain 版は `CardUi` を直接読んで B-2 / B-3 と同じ性質を確認する(= plain でも正しく書けば同じことはできる。差は行数と、掃除を自分で書くかどうかである)。
- `board.rs` のユニットテスト: `reduce` の `MoveCard`(同一列内の前方 / 後方移動、列間移動、末尾)と `visible`。

DnD をどう叩くか: kittest のポインタ操作で足りなければ、`harness.input_mut().events` に `PointerMoved` / `PointerButton` を直接積んで 3 フレーム回す(押す → 動かす → 離す)。それも難しければ、テストは `<Card>` の `on_move` を UI 経由ではなく直接呼べる形(ヘッダの右クリックメニュー、もしくはテスト専用ではないキーボード操作)を用意して叩く。**B-2 / B-3 は「UI から動かして state が付いて回る」ことの確認なので、reducer を直接叩くのは代替にならない。** 手段が無い場合はここを 8 章に書いて相談する。

## 7. gallery / README / CI

- `examples/gallery/Cargo.toml` に依存を足し、`EXAMPLES` に `board::META` を(`showcase` の次に)入れ、`Running` の `match` を 2 つとも足す(react 版と plain 版)。
- `Meta`: `name: "board"`、`hooks: ["use_state", "use_reducer", "use_persisted", "use_memo", "use_effect", "use_handle", "provide_context", "use_context"]`、`elements: ["View", "Text", "TextEdit", "Button", "ScrollArea", "Frame", "Separator"]`、`plain: Some(include_str!("plain.rs"))`。
- snapshot: `examples/gallery/tests/snapshots.rs` に react / plain の対を足す。1 枚で一致させる。一致しない差が残るならその理由をテストのコメントと 8 章に書く。
- README の表に 1 行。`Trunk.toml` と `index.html` は既存 example からコピーし、CI の trunk ループは `examples/*` を舐めているので追加設定は不要(要確認)。

## 8. 実装で判明した差分

(実装しながら書く。ライブラリ側に足りないと判明したものは、ここに「何が書けなかったか / どう回避したか / 足すとしたら何か」を残す。core の変更が要ると判断した場合はこの PR では入れず、別 PR に切る。)
