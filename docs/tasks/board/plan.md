# プラン: board

> v2 で改修した。差分は [board-v2/plan.md](../board-v2/plan.md)。以下は当時の記録なので書き換えない。

タスク定義は [task.md](task.md)。設計の根拠は [docs/ARCHITECTURE.md](../../ARCHITECTURE.md)。実装中にここから外れる判断をした場合は本書を更新し、設計上の意味があれば ARCHITECTURE.md も更新する。

## 0. 全体

1 PR。触るのは `examples/board`(新規)、`examples/gallery`(登録)、README、必要なら `crates/react-egui-elements` と ARCHITECTURE 6 章。core は無変更。

```
examples/board/src/
  lib.rs        App と画面のコンポーネント(gallery が読むのはこれ)
  board.rs      データモデル、Msg、reduce、絞り込み(純関数、ユニットテスト付き)
  hooks.rs      use_undoable / use_debounced / use_dnd / use_identity
  look.rs       両版が使う色とゴーストの描画(8.6)
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

**実装での追加(添字ではなく identity)**: ドロップ先は `DropTarget { column: ColumnId, before: Option<CardId> }`(「このカードの前」「`None` なら列の末尾」)で表し、`Board::drop_index(card, target) -> usize` が `MoveCard` の `to_index`(自分を取り除いた後の添字)に直す。UI 側が添字を作ると、(a) 掴んだカードが列から抜けた瞬間に添字がずれる、(b) 検索で隠れているカードがあると画面上の順番と `Vec` の添字が一致しない、の 2 つを呼び出し側で吸収することになる。`drop_index` は `board.rs` の純関数なのでユニットテストが書け、生 egui 版とも共有できる。

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

- `for` の中の `<Card key={card.id} .. />` / `<Column key={column.id} .. />` は key 必須(ARCHITECTURE 3.2)。
- **訂正(実装で判明)**: key だけでは B-2 は通らない。hook のスロット Id は「スコープの連鎖 + 呼び出し位置」で、`key` は *同じ親の下の兄弟* を区別するだけである。カードは列の内側に描かれるので、`doing` の `<Card key={7}/>` と `review` の `<Card key={7}/>` は別スロットになり、列をまたいで動かすと下書きは元の列に置き去りになる。そこでカード自身の state は `hooks::use_identity(cx, (card.id, "draft"), ..)` に置く。これは `Cx::new(store, cx.ui(), Id::new(("board/identity", key)))` でスコープをカードの identity に張り直してから `use_state` を呼ぶ 3 行の hook で、core には手を入れていない(8 章)。**B-2 / B-3 が通る理由はこの hook であり、`key` はあくまで「同じ列の中で 2 枚のカードを取り違えないこと」を保証する。**
- **props で下ろすもの / context で配るもの**: 「そのコンポーネントに起きたこと」は `#[event]` で 1 段上へ返す(`<Card>` は編集・削除・ラベル変更を `<Column>` へ、`<Column>` は「カードを 1 枚足したい」を `<BoardView>` へ)。「木全体で共有するもの」は context で配る(テーマ、ドラッグ session、`Dispatch`)。`<Column>` は自分が受けたカードのイベントを、context から取った `Dispatch` でメッセージに変えてしまう。列の id を知っている一番内側がそこであり、`<Column>` を 4 つ並べる `for` にコールバックを 4 本持たせずに済む。
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

**実装は (B) 寄りの形に落ち着いた。**

- 掴む場所はカードのタイトルで、`cx.leaf` の中の `egui::Label::new(..).sense(Sense::click_and_drag())` 1 つ。`dnd_drag_source` は使わない。あれはドラッグ中に中身を `Order::Tooltip` の layer へ描き直すので、taffy の leaf の中では「測られる中身」と「描かれる中身」がフレームごとに入れ替わることになる。掴む・落とす・線を引くのに要るのは矩形だけなので、`Response` を 1 つ持てば足りる。
- ゴーストは `Area` ではなく `ctx.layer_painter(LayerId::new(Order::Tooltip, ..))` に直接描く。ウィジェットを置くと同じタイトルが accessibility ツリーに 2 つ現れ、スクリーンリーダーからも kittest からも「同じラベルが 2 個」に見えてしまう。
- slot はカードのタイトル行の矩形を上下に割って 2 つ(「自分の前」「次のカードの前」)、加えて列の footer(`+ card`)が「末尾」の 1 つ。`<View>` は `Response` を返さないので、矩形が要るものは leaf にする必要がある(8 章)。
- `Dnd<P, T>` は payload と drop 先で型を分ける(`Dnd<CardId, DropTarget>`)。中身は `Rc<RefCell<..>>` で、`Handle::with`(repaint を要求しない)から書き換える。`Handle::set` / `update` は必ず `request_repaint` するので、毎フレーム slot を登録すると **アプリがアイドルにならず、`egui_kittest::Harness::run` が `ExceededMaxSteps` で落ちる**(ARCHITECTURE 5.6)。8 章。

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

`use_undoable` が `use_reducer` の上に素直に乗ることが「undo をライブラリ機能にしなくてよい」の根拠になる。

**実装での差**: 境界は `S: Clone + PartialEq`(`M: Send + 'static` は `use_reducer` の要求)。`PartialEq` は「何も変えなかったメッセージを履歴に積まない」ために使う。これが無いと、値の変わらない `EditCard` のあとに undo を 2 回押す羽目になる。`History<S>` は `Serialize` にしない。`showcase` と同じく `use_persisted` のスロットに `present` を毎フレーム鏡写しするだけにしてあり、履歴は保存されない。名前は `use_debounced`。

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
- **実装での差**: 列名の編集中の下書きは `renaming: Option<(ColumnId, String)>`(同時に 1 列だけ)にした。react 版は `<Column>` ごとの `use_state` なので 2 列同時に開ける。map をもう 1 つ持てば揃えられるが、掃除する対象が 1 つ増えるだけで誰も頼んでいない機能なので、`plain.rs` のコメントに理由を書いて 1 列に留めてある。「部品の state を全体が持つと、こういう選択を呼び出し側が迫られる」こと自体が差である。
- 掃除は `state.ui.retain(|id, _| board.card(*id).is_some())` の 1 行。なお react 版の sweep はこれより厳しく、検索で隠れたカードは unmount されるので下書きも消える。plain 版は「盤にまだ在る」ことを基準にしているのでその場合は残る。どちらも筋は通るが、同じにしたければ plain 版に規則をもう 1 つ書くことになる。

## 6. テスト(`tests/board.rs`)

`todo` / `form` と同じく react 版と plain 版の 2 つの harness を組み、同じヘルパで両方を叩く。

- **B-1** カードを追加すると、その列の件数が増える。
- **B-2(目玉)** カードの編集を開いて下書きを打ち、そのカードを別の列へ動かす。移動後もそのカードが編集中で、下書きが残っている。動かした先の隣のカードは編集中になっていない。
- **B-3(目玉)** 列内で 2 枚目を先頭へ動かす。展開していたカードだけが展開されたままで、位置ではなくカードに状態が付いていることを確認する。
- **B-4** undo / redo。追加 → 移動 → undo ×2 → redo で元に戻る。
- **B-5** 検索。debounce の待ち時間はハーネスで時間を進めて越える。絞り込みが列をまたいで効く。
- **B-6** 永続化。`Store::save_persisted` → 新しい `Store` に `load_persisted` → 同じボードが出る(`showcase` のテストに倣う)。
- **B-7** react / plain に同じ操作を流して同じ結果になる。**実装では plain 版も `CardUi` を直接読まず、B-1 〜 B-5 / B-8 をそのまま UI から流している**(位置は画面座標で見るので、ヘルパは両版で同じものが使える)。これで「plain でも正しく書けば同じことはできる」は UI の側から示せる。差は掃除を自分で書くかどうかであって、できる / できないではない。
- **B-8(追加)** 列名のインライン編集。`rename` → 打つ → Enter で名前が変わり、それも履歴 1 手なので undo で戻る。react / plain の両方に流す。
- `board.rs` のユニットテスト: `reduce` の `MoveCard`(同一列内の前方 / 後方移動、列間移動、末尾)と `visible`、`drop_index`、ラベルの巡回。

DnD をどう叩くか: **kittest のポインタ操作で足りた。** `harness.hover_at` → `drag_at`(押す)→ `hover_at` ×2 → `drop_at`(離す)で、egui はこれをクリックではなくドラッグと判定する(`drag_started` は「`dragged` が今フレーム立った」ことなので、押したフレームと動かしたフレームが分かれていてよい)。フォールバック(`input_mut().events` に直接積む / UI に別の移動手段を足す)はどれも要らなかった。

どの列に居るかは **画面上の x 座標**(4 等分のどこか)で、列内の順序は y で確かめる。両版とも 4 列を等幅に並べるので、同じヘルパがそのまま両方に効き、「reducer がどうなったか」ではなく「見えている位置」を見ることになる。

B-3 は上半分に落として前へ、下半分に落として後ろへ、の 2 回動かして、どちらでも展開状態が同じカードに付いていることまで見る。

## 7. gallery / README / CI

- `examples/gallery/Cargo.toml` に依存を足し、`EXAMPLES` に `board::META` を(`showcase` の次に)入れ、`Running` の `match` を 2 つとも足す(react 版と plain 版)。
- `Meta`: `name: "board"`、`hooks: [.. + "#[hook]"]`(自作 hook が 4 つあるので `custom-hook` と同じタグを足した)、`elements: ["View", "Text", "TextEdit", "Button", "ScrollArea", "Frame", "Separator"]`、`plain: Some(include_str!("plain.rs"))`。
- snapshot: `examples/gallery/tests/snapshots.rs` に react / plain の対を足す。**1 枚では一致させられないので `list_10k` と同じく 2 枚にした。** 列は react 版が taffy の `gap`、plain 版が `ui.columns` と `ui.horizontal` の item_spacing で並ぶので、描くものは同じでも位置が数ポイント単位で食い違う。揃えるには `plain.rs` を「読むため」ではなく「taffy の計算を再現するため」に書くことになり、それは 5 章が引いた線の向こう側である。なお snapshot は GPU が要るのでこの環境では 1 枚も生成していない(8 章)。
- README の表に 1 行。`Trunk.toml` と `index.html` は既存 example からコピーし、CI の trunk ループは `examples/*` を舐めているので追加設定は不要(要確認)。

## 8. 実装で判明した差分

core(`react-egui` / `react-egui-macros`)にも `react-egui-elements` にも手を入れていない。以下は「書けなかったこと / どう回避したか / 足すとしたら何か」。

### 8.1 state は木の中の位置に付いていて、identity には付かない(この example の主題そのもの)

**何が書けなかったか**: `<Card key={card.id}/>` の中の `use_state` は、カードを別の列へ動かすと別スロットになる。hook の Id は「スコープの連鎖 + 呼び出し位置」で、`key` は同じ親の下の兄弟を区別するだけだからである(ARCHITECTURE 3.2 / 3.4)。React も同じで、親が変われば unmount して mount し直す。しかし本 example が示したいのは「カードに付いて回る state」なので、これでは B-2 が成立しない。

**どう回避したか**: `hooks::use_identity(cx, key, init)`。`Cx::new(store, cx.ui(), Id::new(("board/identity", key)))` でスコープを identity に張り直し、その `Cx` で `use_state` を呼ぶ。返る guard はストア(`'s`)を借りているだけなので、この 1 行の `Cx` が落ちても生き残り、呼び出し側は普段どおり描き続けられる。sweep も衝突検出もそのまま効く。ただし内側の `use_state` の呼び出し位置は 1 か所なので、key に「誰の」だけでなく「何の」も混ぜる必要がある(`(card.id, "draft")`)。

**足すとしたら**: `use_keyed(cx, key, init) -> State<T>`(`use_persisted` の位置非依存キーから永続化を抜いたもの)。`Store::slot` が `pub(crate)` なので、今ユーザー land から任意 Id のスロットを作る道は「`Cx::new` で張り直す」か「`use_persisted` に文字列キーを渡す」しかない。後者は eframe の storage に書き込み、sweep 時にも直列化されて map に残るので、編集途中の下書きのような一時的な値には使えない。core に足すのは `Store::keyed_slot` と `use_keyed` の 2 つで済み、`use_persisted` はそれを使う形に書き直せる。

### 8.2 `Handle` に「dirty にしない書き込み」が無い

**何が書けなかったか**: DnD の slot は毎フレーム登録し直す。`Handle::set` / `update` / `update_later` はどれも `request_repaint` するので、毎フレーム呼ぶとアプリがアイドルにならない(ARCHITECTURE 5.6)。gallery でも CPU を焼き続け、`egui_kittest::Harness::run` は `ExceededMaxSteps` で落ちる。

**どう回避したか**: 値の側に `Rc<RefCell<..>>` を持たせ、`Handle::with`(`&T` を渡すだけで repaint しない)から書き換えた。`Dnd` が `Clone` で中身を共有するのはこのためで、装飾ではない。

**足すとしたら**: `State::bind()` の `Handle` 版(`Handle::with_mut` / `peek_mut`)。「ウィジェットや毎フレームの記録が直接書き込む、入力が無ければ値は変わらない」という `bind` と同じ意味論で、内部可変性をユーザー land に押し出さずに済む。

### 8.3 `leaf_fill` は「残り」ではなく「全部」を測る

**何が起きたか**: `<ScrollArea grow={1.0}>` は `leaf_fill` なので、taffy への max-content の申告が **ルート矩形の高さそのもの**になる(`egui_taffy` の measure が `infinite` を root_rect のサイズに読み替える)。ランナーのルート item style は `min_h: 100%` で高さは auto なので、`<View column>` の中に「ツールバー + ScrollArea」を並べると、列の高さは(ツールバー + 窓の高さ)になり、窓からはみ出す。`grow` も `basis={0}` も効かない。flex の伸縮は親の高さが確定している時の話で、ここは連鎖のどこにも確定した高さが無いためである。`h="100%"` も、親が auto なら auto に落ちる。

**どう回避したか**: 列の footer(`+ card` と「末尾に落とす」ゾーン)を `ScrollArea` の外ではなく **中の最後**に置いた。はみ出すのは列の下端の余白だけになり、押せるもの・落とせるものは全部見える位置に残る。UI としてもこの方が Trello に近い。

**足すとしたら**: ランナーのルートに「窓の高さそのもの」を与える道。`root_style()` の `min_h: 100%` は「中身が高ければ伸ばす」ためだが、そのせいで「窓を埋めて、余りを ScrollArea に配る」が書けない。`Options` に「ルートを窓の高さに固定する」切り替えを足すか、`ScrollArea` に「残りの高さを取る」意味の item style を用意するか。`showcase` の sidebar と gallery の左列も同じ形なので、この 2 つも今は少しだけ窓からはみ出しているはずである。

### 8.4 `<View>` は矩形を返さない

掴む場所・落とす場所・挿入線は、どれも矩形が要る。`<View>` は `Response` を返さないので、カードのタイトルと列の footer は `cx.leaf` / `cx.leaf_fill` で書いた(この example では escape hatch を見せる意味もあるので損はしていない)。カード全体を drop 先にするには「カードを描き終えた後の矩形」が要り、それは今は取れない。結果として、開いているカードの本文の上は drop 先ではない。足すとしたら `<View>` の `#[event] on_rect: egui::Rect`(`Canvas` の `paint` と同じ性格のもの)。

### 8.5 細かいもの

- `Option<T>` 型の引数は「省略可能な prop」なので、本当に `Option` を渡したい prop は `&Option<T>` にする必要がある(`showcase` の `selected` と同じ)。`next: &Option<CardId>`、`label: &Option<Label>` がそれ。
- `<Toolbar label={&*label} on_label={|..| *label = ..}/>` は E0502(ARCHITECTURE 3.7 の 2 つ目)。値を先にコピーして `label={&filter}` にした。props が借用のままなのは設計どおりなので、これは注意書きが 1 行要るという話。
- `egui::Id::new` は `Hash + Debug` を要求する(`AsId`)ので、`use_identity` の key も `Hash + Debug`。`rsx!` の `key=` と同じ条件なので揃っている。
- snapshot(`cargo test -p gallery --features snapshot`)はこのコンテナでは動かない(GPU も software Vulkan も無い)。`board_react` / `board_plain` の画像は未生成で、最初に GPU のある機械で `UPDATE_SNAPSHOTS=1` を回した人が入れることになる。

### 8.6 行数は思ったほど差が付かない

公平に書いた生 egui 版は、行数ではほとんど負けない。コメントと空行を除くと **react 版 435 行 / plain 版 460 行**、ファイル全体(gallery が表示するのはこちら)では **631 / 582 で react 版の方が長い**(散文が多いのは example の仕事なのでそれ自体は良い)。`todo` も 143 / 143(コードのみ 112 / 104)なので、この repo では前からそうだったことになる。

差が出ないのは、生 egui 版が「毎フレーム全部描く」だけで済む代わりに、react 版は 6 つのコンポーネントの `#[component]` 署名(props とイベント)を書くからである。**この example の主張を「行数が減る」に置くのは無理で、「どの行が何をしているか」に置くべきである。** すなわち生 egui 版にあって react 版に無い行は `ui: HashMap<CardId, CardUi>` とその `retain`、`card_ui(id)` 経由の読み書き、`renaming: Option<(ColumnId, String)>` の妥協であり、これらは全部「部品の state を全体が持つ」ことから出てくる。逆に react 版にあって生版に無いのは props とイベントの宣言で、こちらはコンポーネントを他所でも使えるようにする代金である。task.md の「行数で見せる」はそのままには成立しなかったので、gallery の行数表示は「同じ画面で、同じくらいの量のコードで、置き場所が違う」を見せるものとして読む。

なお `Theme` と ゴーストの描画は両版が使うので `look.rs`(plan 0 章のファイル一覧に対する追加)に出した。`lib.rs` に置いたままだと、共有しているものが react 版の行数にだけ乗って比較が歪む。
