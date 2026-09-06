# プラン: board 改修(v2)

タスク定義は [task.md](task.md)。元の設計は [board/plan.md](../board/plan.md)(以下「v1 plan」)。本書は v1 からの差分だけを書く。実装中にここから外れたら本書を更新する。

## 0. 全体

- 触るのは `examples/board/`(`board.rs` / `lib.rs` / `hooks.rs` / `look.rs` / `plain.rs` / `tests/board.rs`)、`examples/gallery/tests/a11y.rs`、docs、README の表。core と `react-egui-elements` は無変更。足りないものが出たら 9 章に書いて回避する。
- react 版と plain 版は同じ挙動にし、同じテストで叩く(v1 の B-7 方式)。**両方を同じコミットで直す。**
- example の主張(card のローカル state が card に付いて回る)は残す。body と `expanded` が消えるので、付いて回る state は **`editing` と `draft`** の 2 つになる。だから **編集中でも drag できる**ことが必須(3 章)。
- 文体は既存に合わせる(コメントは英語、docs は日本語)。

## 1. データモデル(`board.rs`)

```rust
pub struct Card { pub id: CardId, pub title: String, pub done: bool }

pub enum Msg {
    /// title 付きで足す。空 title の card はボードに入れない(4 章)。
    AddCard { column: ColumnId, title: String },
    SetTitle { card: CardId, title: String },
    SetDone { card: CardId, done: bool },
    RemoveCard { card: CardId },
    MoveCard { card: CardId, to_column: ColumnId, to_index: usize },
    RenameColumn { column: ColumnId, name: String },
}

/// `done`: `None` なら全部、`Some(true)` なら完了だけ、`Some(false)` なら未完了だけ。
pub fn visible(column: &Column, search: &str, done: Option<bool>) -> Vec<CardId>;
```

- `Label` enum、`body`、`EditCard`、`SetLabel` は消す。`Theme::label` も消す。
- `Board::demo()`: 同じ 12 枚を title だけで。"done" 列の 3 枚は `done: true`、他は `false`。
- `SetTitle` は title が同じなら `changed = false`(履歴に積まない)。`AddCard` は呼び出し側が空 title を弾くが、reducer でも `title.trim().is_empty()` なら何もしない。
- ユニットテストを新仕様に合わせる(`visible` は title だけ見る、`done` 絞り込み、`AddCard` の空 title)。
- `use_persisted("board/board")` に旧形式(`body` / `label` 付き)の JSON が残っていると `serde_json::from_str` が失敗し `use_persisted` は `init` に落ちる(`hooks.rs` の `restored.unwrap_or_else(init)`)。それでよい。キーは変えない。plain 版の `STORAGE_KEY` も同じ。

## 2. card の描画(`lib.rs` の `<Card>`)

1 行だけの card にする: `[☐] title ........ [✎] [×]`。

```
<Card key={id} card=.. column=.. next=.. on_title on_done on_remove/>
  cx.leaf(..)                         ← card 全体が 1 つの leaf(矩形が要るため)
    ui.interact(rect, id, Sense::drag())   背面。drag と cursor(2.2)
    egui::Frame::show(..)                  角丸の枠(今の <Frame> と同じ見た目)
      Cx::new(store, ui, scope) で rsx:
        <View row align="center" gap={6}>
          <Checkbox bind=.. />                 done(2.3)
          if editing { TitleEdit } else { title の Label }   (3 章)
          <IconButton name="edit">"✎"</IconButton>
          <IconButton name="remove">"×"</IconButton>
        </View>
```

### 2.1 なぜ leaf 1 つにするか

v1 plan §8.4 のとおり `<View>` / `<Frame>` は矩形を返さない。今回は「card 全体を掴む」「card 全体を drop 先にする」「card に hover で cursor を変える」の 3 つが card の矩形を要求する。`<Frame>` の実装(`containers.rs`)は `cx.leaf` の中で `egui::Frame::show` して子を `Cx::new(store, ui, scope)` で描いているので、同じことを `<Card>` の中で手書きすれば矩形が取れる。elements には手を入れない。

### 2.2 背面の drag 面

- `let rect = ui.max_rect();` を **子を描く前に** `ui.interact(rect, ui.id().with("bg"), egui::Sense::drag())` に渡す。`max_rect` は taffy が leaf に与えた矩形で、初回フレームだけ高さが不正確だが 2 フレーム目から正しい(footer の `leaf_fill` と同じ扱い)。
- **先に登録する理由**: egui の hit test は click 候補と drag 候補を別々に「一番上のもの」から選ぶ(`hit_test.rs`)。背面を先に置けば、Checkbox / IconButton(click だけ)は click を取り、TextEdit(click+drag)は自分の中の drag を取り、それ以外の場所の drag は背面に落ちる。押した後に動かせば click 候補は捨てられ drag になる(`interaction.rs`)ので、**checkbox の上で押して動かしても card が drag される**。テストはこれを使う(8 章)。
- `bg.drag_started()` で `dnd.pick_up(card.id)`。title の Label からは `Sense` を外す(3 章)。
- **cursor**: `bg.contains_pointer()` なら `ui.ctx().set_cursor_icon(CursorIcon::PointingHand)`。`hovered()` ではなく `contains_pointer()`: 子 widget の上でも card の上なので pointer にする。drag 中(`dnd.carrying().is_some()`)は `Grabbing`。
- drop slot は **card 全体の矩形**を上下に割る(v1 の title 行 ± `SLOT_PAD` をやめる。`SLOT_PAD` は消す)。`dnd.slot(top, before: Some(card.id))`、`dnd.slot(bottom, before: *next)`。
- 掴まれている card(`carried`)は今までどおりその場に薄く描く(消すと `use_identity` のスロットが sweep されて draft が消える。v1 plan §8.1)。

### 2.3 done checkbox

- `<Checkbox bind={done.bind()} on_change=..>` は `bind` が `&mut bool` を要求する。card の `done` は props(`&CardData`)なので直接 bind できない。**`cx.leaf` で `egui::Checkbox::without_text(&mut local)` を描き、`changed()` なら `on_done.emit(!card.done)`** が一番短い(`local` はその場のコピー)。elements の `<Checkbox>` は使わない。
- a11y: `ui.ctx().accesskit_node_builder(response.id, |n| n.set_label(format!("done: {}", card.title)))`。**テストは card をこの名前で探す**(8 章)。
- done の card は title を `weak()` + `strikethrough()`。

### 2.4 IconButton

- `<IconButton name="edit">"✎"</IconButton>`: `name: Option<&str>` prop を足し、あれば `accesskit_node_builder` で label を上書きする。表示は絵、木の名前は言葉。テストは今までどおり `get_all_by_label("edit")` で探せる。"×" も `name="remove"`。
- "✎"(U+270E)は egui 同梱の emoji-icon フォントにある。無ければ "✏" か "edit" の文字に戻す(実機で確認できないので最初のビルドで `egui::FontDefinitions` を疑う前に、まず gallery のスクリーンショット無しでもテストは通る。見た目は 10 章の手順で確認)。

## 3. インライン編集(`TitleEdit`)

card の title、column の rename、新規 card(4 章)の 3 か所で同じ部品を使う。`lib.rs` に `#[component] fn TitleEdit(cx, style, bind: &mut String, name: &str, #[event] on_commit: String, #[event] on_cancel: ())`。

- `cx.leaf` の中に `egui::TextEdit::singleline(bind)`。`desired_width(ui.available_width())`。
- **自動 focus + 全選択**: mount 直後の 1 回だけ。`let mut fresh = use_state(cx, || true);` で、`*fresh` なら
  ```rust
  response.request_focus();
  let mut state = egui::TextEdit::load_state(ui.ctx(), response.id).unwrap_or_default();
  let end = egui::text::CCursor::new(bind.chars().count());
  state.cursor.set_char_range(Some(egui::text::CCursorRange::two(egui::text::CCursor::new(0), end)));
  state.store(ui.ctx(), response.id);
  *fresh = false;
  ```
  `TitleEdit` は編集を閉じると unmount されるので、次に開いた時はまた `fresh`。card が列をまたいで動くと remount されて再 focus + 再選択になる(draft は `use_identity` なので残る)。これは許容し、コメントに書く。
- **Enter で確定**: `response.lost_focus() && input.key_pressed(Enter)` → `on_commit.emit(bind.clone())`。
- **Esc でキャンセル**: `input.key_pressed(Escape)` かつこの field が focus を持っていた(egui は Esc で focus を手放す。`lost_focus() && key_pressed(Escape)`)→ `on_cancel.emit(())`。
- **それ以外で focus を失った**(別の場所をクリック)→ **確定**(Trello と同じ)。ただし mount 直後の `request_focus` が効くフレームより前に `lost_focus` が立つことは無いので順序は気にしない。
- a11y: `accesskit_node_builder` で `set_label(name)`(`"title"` / `"column name"` / `"new card"`)。gallery の a11y テストが無名 `TextInput` を数えるため。

呼び出し側:

- **`<Card>`**: `editing` / `draft` は v1 と同じ `use_identity`。ペンで `*draft = card.title.clone(); *editing = true`。`on_commit`: `on_title.emit(title)` して `*editing = false`。空 title(`trim().is_empty()`)の確定は **キャンセル扱い**(title は変えない)。`on_cancel`: `*editing = false`。
- **`<Column>` の rename**: `<TextEdit on_submit>` を `TitleEdit` に置き換える(Esc で戻せるようになる)。"rename" ボタンはそのまま。
- v1 の `Draft` 構造体、`save` / `cancel` ボタン、`expanded`、本文の `<Text wrap>` は消す。

## 4. 新規 card("+ card")

**ボードには入れず、column のローカル state で編集する。** `AddCard { column, title }` は確定時に 1 回送る。

- `let mut adding = use_state(cx, || false); let mut new_title = use_state(cx, String::new);` を `<Column>` に置く。
- "+ card" で `*new_title = String::new(); *adding = true`。
- `*adding` なら、card リストの末尾(footer の直前)に card と同じ枠(`Frame`、同じ余白)で `TitleEdit name="new card"` を描く。`on_commit`: title が空でなければ `send(AddCard { column, title })`、どちらでも `*adding = false`。`on_cancel`: `*adding = false`。
- この形にする理由: 空 title の card をボードに入れてから消すと、履歴が「追加」「削除」の 2 手になり、`use_persisted` にも空 card が残りうる。column のローカル state なら undo は 1 手、保存されるのは確定した card だけ。`Column` の `on_add: ()` イベントは `on_add: String` に変わり、`BoardView` が `AddCard` を送る。
- 新規 card の枠は drop slot を持たない(footer が「末尾」を担当したまま)。

## 5. ドラッグ(`lib.rs` / `hooks.rs`)

### 5.1 テキスト選択のバグ

原因: title の `egui::Label` に `Sense::click_and_drag` を付けても Label は `selectable_labels`(既定 true)のままなので、押した瞬間にその Label 上で文字選択が始まり、`LabelSelectionState` が drag 中に通った他の Label へ選択を伸ばす(`label_text_selection.rs`)。

直し: title の Label を `.selectable(false)` にし、`Sense` を外す(掴むのは 2.2 の背面)。押下が Label で始まらなければ選択は始まらない。**同じ直しを plain 版にも入れる。** 直った確認は 8 章 B-12(`egui::text_selection::LabelSelectionState::load(ctx).has_selection()` が drag 後に false)。それでも選択が起きるなら、`BoardProvider` の `use_effect` で `ctx.style_mut(|s| s.interaction.selectable_labels = false)` にする(gallery の他の example にも効くので最後の手段)。

### 5.2 挿入先の preview

青い線(`hline`)を **空 card の placeholder** に置き換える。`Card` の `before_me` / `Column` の `hovered_end` の `hline` は消す。

- `Column` が描く。`let target = dnd.hovered().filter(|t| t.column == column.id);` を見て、card リストの中で `target.before == Some(card.id)` の card の **直前**、`before == None` なら footer の直前(4 章の新規 card 枠の後ろ)に `<Placeholder>` を 1 つ入れる。
- **掴んでいる card の元の位置と同じなら出さない**: `target.before == Some(carried)` または `target.before == after[i]`(carried が i 番目)は無視。動かない drop に preview は要らない。
- `Placeholder` は `cx.leaf` で card と同じ幅、`PLACEHOLDER_H`(= card 1 行の高さ。`ui.spacing().interact_size.y + 12.0` を目安に定数)を `allocate_exact_size(.., Sense::hover)` し、角丸の破線枠(`theme.accent()`、内側は `theme.card()` を薄く)を描く。
- **必ず `dnd.slot(rect, target)` を登録する。** placeholder が入ると下の card がずれ、ポインタが placeholder の上に来る。その矩形が slot でないと `hovered` が `None` になって placeholder が消え、card が戻り、また `hovered` が立つ、を毎フレーム繰り返す。placeholder 自身が同じ target の slot なら安定する。
- `response.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "drop here"))` で名前を付け、テストから見えるようにする(8 章 B-11)。
- ゴースト(`look::ghost`)はそのまま。`Theme::accent` のコメント(「挿入線」)を直す。
- plain 版も同じ描き方(同じ定数、同じ slot 登録)。

### 5.3 `hooks.rs`

変更なし。`Dnd` の API はそのまま使える。

## 6. Toolbar

ラベルの chip は意味を失うので、**"open" / "done" の 2 つの chip による絞り込み**に置き換える(`filter: Option<bool>`)。押した chip をもう一度押すと解除(v1 の `Option<Label>` と同じ動き)。`<Chip>` と `use_memo` の deps(`(column.id, rev, search, filter)`)はそのまま生きる。色は `theme.accent()`。plain 版も同じ。

## 7. plain 版(`plain.rs`)

react 版の各項目をそのまま鏡写しにする。差は「状態をどこに置くか」だけ、という v1 の建前を守る。

- `CardUi { editing: bool, draft_title: String }`。`expanded` / `draft_body` は消す。
- 新規 card: `adding: Option<(ColumnId, String)>`(`renaming` と同じ形。同時に 1 列だけ。理由は v1 plan §5 と同じで、コメントに書く)。
- 自動 focus + 全選択: `fresh: Option<egui::Id>`(次のフレームで focus と選択を当てる field の id)を `PlainState` に持つ。react 版の `fresh` state に当たる。
- 背面 drag、cursor、checkbox、placeholder、Label の `.selectable(false)`、open/done 絞り込み。

## 8. テスト(`tests/board.rs`)

ヘルパを新仕様に合わせ、react / plain の両方に同じ関数を流す。

- **card を探す**: title で `get_by_label(title)`(Label は `value` に text を持つ。今も通っている)。編集中は Label が無いので **checkbox `"done: <title>"`** で探す。`fn card_rect(harness, title) -> Rect` は checkbox の矩形を基準にせず、`"done: <title>"` の checkbox の中心を **掴む点**にする(2.2: checkbox の上で押して動かすと card の drag になる)。`drag` の `from` はこれ。
- **drop 先**: `onto` の title Label の矩形の `top()+1` / `bottom()-1`(card の上半分 / 下半分に入る)。footer は `"+ card"` のまま。
- `fn open_editor(title)`: `"edit"` の中で y が近いものをクリック(今と同じ)。開いたら `draft_field()` が **focus を持っている**ことを assert(`accesskit_node().is_focused()` か `harness.ctx.memory(|m| m.focused())`)。
- `fn draft_field()`: 単一行 `TextInput` の最後(検索 → rename → 編集中 or 新規、の順)。名前 `"title"` / `"new card"` で引ける方が確実なので `get_by_label` を使う。
- `fn edit(title, text)`: `open_editor` → `key_press(Key::End)`(全選択を解いて末尾へ)→ `type_text(text)`。`fn editors()` は `query_all_by_label("title").count()`。

| # | 内容 |
|---|---|
| B-1 | "+ card" → 新規 field が focus 済み → "new card" と打って Enter → 13 cards、backlog 5/5、`column_of("new card") == 0`。続けて "+ card" → Esc → 13 のまま。"+ card" → 何も打たず Enter → 13 のまま |
| B-2 | `edit("buy milk", " and bread")` → 編集中のまま "wire the drag" の上半分へ drag → 列 1、順序、`draft() == "buy milk and bread"`、`editors() == 1`、保存 title は "buy milk" のまま |
| B-3 | `edit("read the plan", "!")` → "write the plan" の上へ → 順序が入れ替わり、`editors()==1` で draft が "read the plan!"、"write the plan" は編集中でない → 下半分へ戻す → 同じ |
| B-4 | 追加(B-1 の手順)→ "buy milk" を末尾列へ → undo ×2 → redo ×2(今と同じ) |
| B-5 | 検索(今と同じ。title だけ見るが結果は同じ: backlog 2/4、review 0/2) |
| B-6 | 永続化(追加の手順が変わるだけ) |
| B-8 | rename: 今と同じ + `rename` を開いて Esc → 名前が変わらず field が閉じる |
| B-9 (新) | 編集: ペン → 開いた直後に "z" を打つと draft が "z"(全選択の確認)→ Esc → title は元のまま、editors()==0。ペン → End → "!" → Enter → title "buy milk!"、editors()==0、undo で戻る |
| B-10 (新) | done: "buy milk" の checkbox クリック → `toggled` → undo で戻る。chip "done" → done 列 3/3、backlog 0/4 → もう一度押して解除 |
| B-11 (新) | preview: "buy milk" を掴んで "wire the drag" の下半分に hover(`drag_at` → `hover_at` ×2、離さない)→ `query_by_label("drop here")` がある、"name the hooks" の y が hover 前より下 → `drop_at` → "drop here" が無い |
| B-12 (新) | 選択: B-11 と同じ drag の後に `LabelSelectionState::load(&ctx).has_selection()` が false |
| B-7 | plain 版に B-1 〜 B-12 を流す |

`board.rs` のユニットテストは 1 章。

## 9. gallery / docs

- `examples/gallery/tests/a11y.rs`: 新しい focusable は checkbox(名前付き)、`TitleEdit`(名前付き)、IconButton(名前付き)なので `KNOWN_UNNAMED` は増えないはず。落ちたら **名前を付ける方を優先**し、どうしても無理なものだけ足す(木の順に厳密比較なので位置に注意)。
- `lib.rs` 冒頭の doc: body / expanded の記述を title / editing に直す。`META.summary` と `elements`(`Frame` は手書きになるので外す、`Checkbox` は使わないので足さない)。
- `plain.rs` 冒頭の doc も同じ。
- `docs/tasks/board/task.md` のスコープ(カードの項)と `docs/tasks/board/plan.md` の先頭に「v2 で改修。差分は [board-v2/plan.md](../board-v2/plan.md)」を 1 行。v1 plan の本文は書き換えない(当時の記録)。
- README の examples 表の board の 1 行を新しい説明に。
- `progress.md` の board の行に v2 を追記。

elements に足したくなったもの(足さない。記録だけ):

- `<TextEdit autofocus select_all>`、`<Checkbox>` の非 bind 版(`checked` + `on_change`)、`<Frame>` の `on_response`。全部 leaf の手書きで回避した。

## 10. 手順(subagent 用)

各ステップの後に `cargo fmt && cargo clippy -p board --all-targets -- -D warnings && cargo test -p board`。最後に workspace 全体。GPU snapshot は回さない。

1. `board.rs`: 1 章。ユニットテスト緑。
2. `lib.rs`: `TitleEdit`(3 章)→ `Card`(2 章)→ `Column`(4, 5.2 章)→ `Toolbar`(6 章)→ `look.rs`。この時点で `tests/board.rs` は壊れてよい。
3. `tests/board.rs`: 8 章。react 版の B-1 〜 B-6, B-8 〜 B-12 を緑にする。
4. `plain.rs`: 7 章。B-7 を緑にする。
5. 9 章(a11y、docs、README)。`cargo test --workspace`、`cargo check --workspace --target wasm32-unknown-unknown`。
6. 可能なら `cargo run -p board` / `--bin board-plain` で目視: cursor、全選択、placeholder、"✎" の描画。
