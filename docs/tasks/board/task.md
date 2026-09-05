# タスク: board(PR8 = フェーズ 6.6 の前半。[patch](../patch/task.md) と 1 つの PR)

## 目的

既存 example が示していない React の利点を 1 つの example で示す。すなわち、

- **動的に増減・並べ替えされる要素が、それぞれローカル state を持てる**こと。
- **その state が key(identity)に付いて回る**こと。カードを別の列へ動かしても、そのカードの「編集中の下書き」「展開状態」は一緒に移り、隣のカードのものと入れ替わらない。
- **props / children / `#[event]` を持つ自作コンポーネントを組み合わせて画面を作れる**こと。今の examples はライブラリの要素を使うばかりで、ユーザーが部品を作って使い回す例が `showcase` の `Themed` しかない。
- **振る舞いを custom hook に切り出して再利用できる**こと。`custom-hook` example の 3 つより実務的なもの(DnD、debounce、undo)を、ライブラリに手を入れずユーザー land で書けることを示す。

題材は Trello 風のカンバンボード。同じ UI の生 egui 版を隣に置き、per-item のローカル state を呼び出し側が `HashMap<CardId, _>` で持って自分で掃除する必要が消えることを、コードと行数で見せる。

同じ PR の後半([patch](../patch/task.md))は、この example が作る DnD まわりの custom hook をそのまま使う。**board を仕上げてから patch に入る**。ここは「利点を 1 つずつ証明する場」、patch は「それが規模の大きい画面でも崩れないことを見せる場」という分担にする。

core(`react-egui`、`react-egui-macros`)には手を入れない。`react-egui-elements` への追加は必要最小限に留める(判断は [plan.md](plan.md) 4 章)。

## スコープ

### 含む

- `examples/board`(lib + bin + `plain.rs`)。
  - 列 4 つ(Backlog / Doing / Review / Done)を横に並べ、各列はカードの縦リスト。
  - カード: タイトル、ラベル色、本文。**インライン編集(`editing` / `draft`)と本文の展開(`expanded`)はカードのローカル state**。
  - ドラッグ & ドロップ: 列間の移動と列内の並べ替え。
  - 列: 名前のインライン編集、件数表示、末尾にカードを追加。
  - ツールバー: 検索(debounce)、ラベルでの絞り込み、undo / redo、ダーク / ライト。
  - ボードの中身は `use_reducer` が持ち、`use_persisted` が再起動を跨ぐ(`showcase` と同じ組み合わせ)。
  - undo / redo は `use_undoable`(reducer を履歴で包む custom hook)。
  - `Dispatch` とテーマは `provide_context` で配り、間の `<View>` は何も知らない。
  - 列ごとの表示カード(検索 + 絞り込みの結果)は `use_memo`。
  - 自作コンポーネント: `<Card>` `<Column>` `<Chip>` `<IconButton>` `<Toolbar>`。いずれも `style: ItemStyle` を受けて呼び出し側のレイアウトに従い、`#[event]` でイベントを親へ返す。
  - 自作 hook: `use_dnd` / `use_debounced` / `use_undoable`。
- `plain.rs`: 同じ UI・同じ見た目を生 egui で。per-card state の保持と掃除、DnD、undo を含む。
- gallery への登録(`Meta`)、kittest、README の表、行数の比較。
- 実装で判明した「ライブラリ側に足りないもの」を plan.md 8 章に記録する。

### 含まない

- core(`react-egui` / `react-egui-macros`)の変更。必要が出たら plan.md 8 章に書き、別 PR で扱う。
- DnD のアニメーション(ゴーストの補間、並べ替えのイージング)。位置は即時入れ替えでよい。
- 複数ボード、ラベルの編集 UI、期日、添付、担当者、検索のハイライト。
- サーバ同期、実データ、ファイル入出力。`use_persisted` の 1 キーだけ。
- 仮想化。カードは数十枚を想定する(大量要素は `list-10k` の担当)。

## 成果物

- `examples/board/`(`src/lib.rs` / `src/board.rs` / `src/hooks.rs` / `src/plain.rs` / `src/main.rs` / `src/plain_main.rs` / `Trunk.toml` / `index.html` / `tests/board.rs`)。
- `examples/gallery` への登録(`EXAMPLES`、`match name` の 2 箇所)、gallery の snapshot。
- README の examples 表に 1 行。
- 必要なら `crates/react-egui-elements` への最小の追加と `docs/ARCHITECTURE.md` 6 章の更新。
- plan.md 8 章(実装で判明した差分と、ライブラリ側の宿題)。

## 終了条件

- kittest が緑。目玉は次の 2 つ(plan.md 6 章 B-2 / B-3)。
  - 編集中のカードを別の列へ移しても、下書きと展開状態がそのカードに付いて回る。
  - 列内で並べ替えても、隣のカードの状態と入れ替わらない。
- react-egui 版と生 egui 版に同じ操作を流して同じ結果になる(`todo` / `form` と同じ方式)。gallery の snapshot が 1 枚で一致する。
- `cargo run -p board` と `cargo run -p board --bin board-plain` が動く。`trunk serve` でブラウザでも動く(目視)。
- gallery に `board` が載り、`#board` で直リンクでき、生 egui 版へ切り替えられる。
- `cargo fmt --check` / `cargo clippy --workspace --all-targets -- -D warnings` / `cargo test --workspace` / `cargo check --workspace --target wasm32-unknown-unknown` / trunk ループが CI で緑。
- README の表と ARCHITECTURE が実装と一致している。

## 決めごと(着手時点での前提)

- gallery に載せるので `Panel` / `CentralPanel` を使わず、渡された領域を埋めるコンポーネントにする。`std::time::Instant` は使わず時刻は `ctx.input(|i| i.time)` から取る。`use_persisted` のキーは `"board/..."`。
- DnD はまず egui 0.36 の `Ui::dnd_drag_source` / `Ui::dnd_drop_zone` をそのまま使う方向で試し、taffy と噛み合わなければ `Ui::interact` + `DragAndDrop::set_payload` の自前判定に落とす(判断基準は plan.md 3 章)。どちらでも「ユーザー land の custom hook / コンポーネントとして書ける」ことは崩さない。
- undo / redo はライブラリの機能にしない。`use_reducer` を包む custom hook で足りることを示すのがこの example の主張の一部である。
- 生 egui 版は公平に書く。per-card state を `Vec` の添字で持って壊れる書き方をわざと選ばない。`HashMap<CardId, CardUi>` で正しく書き、その map の生成・破棄・掃除が呼び出し側の仕事になることを差として示す。
- 見た目は snapshot が 1 枚で一致する範囲に留める(角丸の `Frame`、ラベル色の細いバー、件数のバッジ程度)。凝った装飾は入れない。
