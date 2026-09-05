# react-egui アーキテクチャ

React 風の書き方(JSX、関数コンポーネント、hooks)で egui アプリを書くための Rust ライブラリ。本書は設計上の決定事項を記録する。実装がここから逸れる場合は本書を先に更新する。

## 1. ゴールと非ゴール

### ゴール

- ユーザーコードは Rust のみ。`rsx!` マクロ(JSX 風構文)と `#[component]` 関数、hooks で UI を書く。
- 動作ターゲットは native(macOS / Windows / Linux)と wasm を必須とし、iOS / Android を後続で対応する。
- Yew で苦痛だった hooks の儀式(`'static` 閉包、`Rc<RefCell>`、`Callback`、全部 `.clone()`)を、immediate mode の性質を利用して取り除く。
- Flexbox / Grid レイアウトを一級市民として提供する。

### 非ゴール

- JS / TS の React を動かすこと。JS ランタイムは同梱しない。
- React の意味論への忠実な再現。VDOM、reconciler、`memo()`、`useCallback` は作らない。
- Signal 風の細粒度反応性。購読機構は持たない。

## 2. 基本原理

### 2.1 reconciler を持たない

React の diff は「DOM 操作が高価だから最小化する」ためにある。egui は毎フレーム全ウィジェットを再発行する前提なので、差分を取る対象(retained 側)が存在しない。`rsx!` は保持ツリーを作らず、その場で egui 呼び出しに直接展開する。

この結果、イベントハンドラは展開された場所で即座に呼ばれて捨てられる。ハンドラがフレームを跨いで生きないので、`'static` も `Rc` も不要で、ローカル変数を `&mut` で普通に借用できる。これが本設計の最大の利点であり、以降の全ての決定はこの性質を壊さないことを優先する。

### 2.2 状態は Id をキーにしたストアに置く

egui はウィジェットツリーを保持しないが、`CollapsingHeader` の開閉やスクロール位置は `Context::Memory` に `Id` をキーにして保存している。`Id` は親から順に導出される階層的なハッシュなので、同じ場所に同じ順で描かれたウィジェットは毎フレーム同じ Id を持つ。本ライブラリのユーザー状態(hooks)も同じ原理で保持する。新しいパラダイムを egui に足すのではなく、egui のパラダイムにユーザー状態を乗せる。

状態の識別は「ツリー上の位置 + 呼び出し位置 + key」で行う。これは Jetpack Compose の positional memoization、SwiftUI の structural identity と同じ考え方で、React の「呼び出し順インデックス」とは異なる。描画は Dear ImGui / egui 式、状態識別は Compose / SwiftUI 式の組み合わせである。

### 2.3 pull 型で毎フレーム読み直す

Signal(SolidJS、Preact Signals)は push 型で、state が購読者を知り、変更を該当ノードだけに伝える。本設計は pull 型で、誰も購読せず、毎フレーム全部読み直す。egui にはスキップすべき retained 側が無いので、細粒度更新は得るものがない。

egui の「毎フレーム」は 60fps 固定ではなく、入力があった時と `request_repaint` された時だけ描き直す。何もなければ 0 フレーム。この性質を壊さないため、ランタイムは状態変化時に確実に `request_repaint` を呼ぶ(5.6 参照)。

描画コストは削れないので、重い導出値だけ `use_memo` で手動キャッシュする。長いリストは egui アプリと同様に `ScrollArea::show_rows` などで仮想化する。

## 3. コア概念

### 3.1 `Cx`

コンポーネントと hooks に渡されるコンテキスト。以下を持つ。

- `store: &'s Store`: hooks 状態のストアへの共有参照。lifetime `'s` は `&mut Cx` の借用とは独立で、hooks が返すハンドルはこの `'s` を持つ。
- `surface: Surface<'u>`: 現在の描画先。`Surface::Ui(&mut egui::Ui)` か `Surface::Taffy(&mut egui_taffy::Tui)` のどちらかで、taffy コンテナの内側かどうかがこの enum そのものである(6 章参照)。フィールドは非公開で、`cx.ui()` メソッドが現在の `&mut egui::Ui` を返す(Taffy なら `tui.egui_ui_mut()`)。
- `scope: Id`: 現在のコンポーネントスコープの Id。hooks の Id 導出の基点。

主なメソッドは以下。

| メソッド | 意味 |
|---|---|
| `ui() -> &mut egui::Ui` | 現在の `Ui`。Taffy モードでは taffy の配置を受けない escape hatch |
| `ctx() -> &egui::Context` / `scope_id() -> Id` / `in_taffy() -> bool` | 参照系 |
| `scope(source, f)` | コンポーネントスコープを一段深くする。Ui モードは `ui.push_id`、Taffy モードは `tui.with_auto_id_prefix`。どちらでも hooks の Id と egui 側のウィジェット Id が同時に分かれる |
| `hook_scope(location, f)` | カスタム hook のスコープ。egui の Id には触らない |
| `leaf(&ItemStyle, f)` | egui ウィジェットを 1 つ描く。Taffy モードなら `tui.style(style.to_taffy()).ui(f)` で taffy の leaf にし、Ui モードなら `f(ui)`(`style` は無視) |
| `container(id, taffy::Style, f)` | taffy ノードを作り、その中を Taffy モードの `Cx` で描く。Ui モードなら `egui_taffy::tui(ui, id).reserve_available_width()`、Taffy モードなら子ノードの追加 |
| `defer(f)` | パス末に走る遅延キューに積む(5.5 参照)。`f` は `'static` |

egui のコンテナ閉包(`ui.vertical(|ui| ..)` など)に入るときは、これまでどおり内側の `Ui` で `Cx::new(store, ui, scope)` を作り直す。暗黙のグローバルや thread-local は使わない。`cx` は常に明示的に引き回す。

### 3.2 `View` と `rsx!`

`rsx!{ ... }` は `impl View` を返す。`View` は `fn show(self, cx: &mut Cx)` を持つ trait で、実体は `FnOnce(&mut Cx)` の閉包である。`View` は以下にも実装する。

- `()`: 何もしない。子を持たない要素の `children` がこれになる。
- `&str`、`String`: `Label` として描く。
- `Option<V: View>`: `Some` なら描く。
- `Vec<V: View>`、`[V: View; N]`: 順に描く。
- `FnOnce(&mut Cx)`: そのまま呼ぶ(egui を直接触る escape hatch)。

`IntoIterator<Item = V>` への blanket impl は `FnOnce` の blanket impl とも `Option<V>` とも coherence で衝突するので採らず、`Vec` と配列の個別 impl にした。`rsx!` の中の繰り返しは `for` で書けるので実用上の差はない。

`rsx!` は常に `::react_egui::view(|cx| { .. })` を emit する。`pub fn view<F: FnOnce(&mut Cx<'_, '_>)>(f: F) -> impl View` は閉包の引数型を固定するためだけの補助関数で、`impl View` の位置に裸の閉包を書くと `cx` の型が推論されないことがある。ユーザーが escape hatch を書くときも `view(|cx| ..)` を使う。

`rsx!` の閉包は非 `move` で、ローカルを借用する。閉包は生成された文の中で即座に消費されるので借用は短命である。

`rsx!` の中では以下が書ける。

- 要素: `<Button onclick={..}>"text"</Button>`。要素名はすべて Rust の関数コンポーネント。HTML 風の小文字タグは持たない。
- 式埋め込み: `{expr}`。`expr: impl View`。
- 制御構文: `if` / `else` / `for` / `match` を直接書く(Dioxus 方式)。直接展開なので実際の Rust の制御構文を emit するだけで済み、`items.iter().map(|i| rsx!{..})` で起きる「`FnMut` から借用を返せない」問題を回避できる。
- `key={expr}`: 要素のスコープ Id に混ぜる。`for` の中で hooks を持つコンポーネントを描く場合は必須。式は `Hash + Debug` を満たす必要がある(egui 0.36 の `Ui::push_id` が `AsIdSalt = Hash + Debug` を要求するため)。
- ハンドラは `Handler::call(closure, payload)` の形で生成・即時呼び出しされる。spike の手書き展開にあった `(|| ..)()` は使わないので、`rsx!` の展開結果に `#[allow(clippy::redundant_closure_call)]` は要らない。

### 3.3 コンポーネント

```rust
#[component]
fn Counter(cx: &mut Cx, initial: i32, label: Option<&str>, #[event] on_change: i32) {
    ...
}
```

`#[component]` は以下を生成する。

- Props 構造体(`CounterProps`)。`Option<T>` のフィールドは省略可能。
- 子を受け取る場合の `children: impl FnOnce(&mut Cx)`(rsx! の子ノード群がこの閉包になる)。
- `#[event]` 引数からイベント enum `CounterEvent`(3.6 参照)。

`<Counter initial={0} />` は次のように展開される。

```rust
cx.scope(Id::new(call_site).with(key), |cx| Counter(cx, CounterProps { initial: 0, ..Default::default() }));
```

`scope` は `cx.scope` を一段深くし、同時に `ui.push_id` を呼ぶ。これにより hooks の Id と egui 側のウィジェット Id の両方がコンポーネントインスタンスごとに安定する。

インスタンスの同一性は以下の挙動になる(React と一致する)。

- 別の場所に書いた 2 つの `<Counter/>` は独立した状態を持つ。
- `for` 内の `<Counter key={i}/>` は key で区別される。key を忘れると Id が衝突し、衝突検出(3.4)が警告する。
- `if show { <Counter/> }` で `show` が false になったフレームで Counter の Id は訪問されず、フレーム末の sweep が状態を破棄して `use_effect` の cleanup を走らせる(unmount)。再び true になれば初期化からやり直す(mount)。
- ツリー上の位置を移すと Id が変わり状態がリセットされる。

### 3.4 Id の導出と衝突検出

hook の Id は `scope.with(Location::caller())` で導出する。`use_state` などの hook 関数は `#[track_caller]` を持ち、呼び出し元の `file:line:column` を得る。React の「hooks は無条件・同順序で呼べ」というルールは不要で、`if` の中で `use_state` を呼んでよい。

カスタム hook は `#[hook]` を付ける。`#[hook]` は関数に `#[track_caller]` を付け、本体を `cx.hook_scope(Location::caller(), |cx| { .. })` で包む。これによりネストした hook の Id は「スコープ → カスタム hook の呼び出し位置 → 内側の hook の呼び出し位置」という呼び出し位置のスタックになり、何段でも一意になる。合成可能性はこれで担保する。

同一パス内で同じ Id が 2 回要求されたら衝突である(訪問済みマークで検出できる)。1 回目の guard がまだ生きている場合は同じ `RefCell` の二重借用になるので、原因と対処(`#[hook]` の付け忘れ、`for` 内の `key` 忘れ)を示すメッセージで panic する。素の `RefCell` の panic にはしない。1 回目の guard が既に落ちている場合は 2 回目が同じスロットを黙って再利用する(`for i in 0..3 { use_state(cx, || i) }` は 3 回とも 0 を返す)。これがまさに避けたい「静かなバグ」なので、記録と `log::warn!` に加えて、debug ビルドでは egui の Id 衝突警告と同じ UX で画面上に警告を出す。オーバーレイの主目的はこの後者のケースである。実装は `Store::end_pass` の最後で、`warn_on_collision`(既定 `cfg!(debug_assertions)`、`set_warn_on_collision` で切り替え)が有効かつ衝突があれば、`Order::Debug` の `egui::Area` を左上に置き、`react-egui: hook id collision at {file}:{line}:{column}. Wrap custom hooks in #[hook], or add key= inside loops.` を赤字で出す。同じ呼び出し位置は 1 パスに 1 行にまとめる。

却下した代替案: Id に「同一位置の出現回数」を混ぜて衝突を無くす案。衝突は消えるが、構造が変わった時に状態が別インスタンスへ静かに移る(React の rules-of-hooks 違反と同じ現象)。ループで出現回数が変わることを正当な利用として許すため検出もできない。「うるさいエラー」を「静かなバグ」と交換する設計なので採らない。

`Location::caller()` はコード編集で変わる。プロセス内で完結する状態には問題ないが、永続化する状態を位置で識別すると行の挿入で保存データが読めなくなる。`use_persisted` は明示的な文字列キーを必須とする。

### 3.5 `State<'s, T>`

`use_state(cx, init)` が返すハンドル。ストア内スロットへの `RefMut` 相当の guard で、`Deref<Target = T>` と `DerefMut` を実装する。`*count += 1`、`&mut *name` がそのまま書ける。値はストアに直接置かれ、ハンドルにキャッシュしないので Drop 書き戻しは無く、`T: Clone` も要求しない。

guard はコンポーネント本体の間だけ生きる(`'s` はストアの lifetime だが、ローカル変数として本体を抜ける時に落ちる)。カスタム hook は guard を値で返せる。guard を move で抱えた閉包を返すこともできる。

補助として `count.into_handle()` で guard を消費して `Handle<'s, T>` に変えられる。`State::handle(&self)` は提供しない。guard が生きたまま `Handle` を使うと同じ `RefCell` の二重借用で panic するためである。context に渡すなど最初から `Handle` が欲しい state は `use_handle(cx, init)` で取る。`Handle` を得る経路は `use_handle`(guard を作らない)と `into_handle`(guard を解放する)の 2 つしかなく、`provide_context` は `Handle` しか受け取らないので、「guard と `Handle` が同じスロットに同時に存在する」状態は公開 API では作れない(spike で確認)。`Handle` は Copy で、`.get()`(`T: Clone`)、`.set(v)`、`.update(|&mut T|)`、`.with(|&T|)` を `&self` で提供する。フレーム内で state を構造体に入れて持ち回る場合や、`use_context`(4 章)で使う。フレームを跨いで運ぶ場合は `Dispatch`(4 章)を使う。

### 3.6 イベント(callback props)

直接展開では、兄弟要素のハンドラは順番に生成・消費されるので同じ state を `&mut` で捕まえても衝突しない。衝突するのは「同一要素に複数の callback props を渡し、両方が同じ state を `&mut` で捕まえる」場合だけである。これを `rsx!` が 1 つの閉包に融合して解決する。

```rust
// ユーザーコード
<Dialog title="Quit?" on_ok={|| *open = false} on_cancel={|| *open = false} />

// 展開
Dialog(cx, DialogProps {
    title: "Quit?",
    events: &mut |ev| match ev {
        DialogEvent::Ok(a)     => Handler::call(|| *open = false, a),
        DialogEvent::Cancel(a) => Handler::call(|| *open = false, a),
        _ => {}
    },
});
```

- `rsx!` は要素名 `Dialog` と属性名 `on_ok` から `DialogEvent::Ok` を文字列的に組み立てる(`on_` を外して PascalCase)。型情報は不要。存在しないイベント名は variant が無いのでコンパイルエラーになる。
- `Handler<A, Marker>` は `FnOnce() -> R` と `FnOnce(A) -> R` の両方を受ける trait。2 つの blanket impl は coherence で衝突するので、マーカー型引数 `(Arity0, R)` / `(Arity1, R)` で区別する。マーカーは常に推論され、`on_ok={|| ..}`、`on_change={|v| ..}`(引数型の注釈なしでも可)、ペイロードを捨てる `|| ..`、非 `()` を返す本体のいずれも `::react_egui::Handler::call(closure, a)` の一形式で呼べる(spike で確認済み)。マクロは常にこの完全修飾パスを emit する。
- 閉包リテラルは `match` の腕の中で生成・即時呼び出しされる。`open` を `&mut` で捕まえるのは外側の融合閉包 1 つだけ。
- コンポーネント側では `#[event] on_ok: ()` が `Emitter` になり、`on_ok.emit(())` で発火する。`Emitter<'a, 'e, E>` は `&'a RefCell<&'e mut dyn FnMut(E)>` を持つだけなので、子の中で複数同時に生きられる(`RefCell` は不変なので lifetime は 2 つ必要)。再入的な emit は明確なメッセージで panic する。
- `on_*` を 1 つも渡さず `events={|e| match e {..}}` と書く escape hatch も通す。
- ペイロードは借用でよい(`on_change: &str` が可能)。

却下した代替案: 子がイベントを戻り値で返し、マクロが子の呼び出し後に `match` する案。単純だが、1 フレームに複数イベントが起きる場合(TextEdit の change と submit)に `Vec` が要り、借用ペイロードを返せない。Cell 風 `Handle` のみで書かせる案は `*count += 1` の糖衣を失う。ユーザーが `callback={|e| match e {..}}` を毎回書く案は冗長で、融合閉包はそれをマクロが代行したものである。

これは実質 Elm / Yew の `Msg` enum だが、生成されて隠れているのでユーザーは React の `onOk` / `onCancel` と同じ感覚で書ける。

### 3.7 残る借用の制約

融合閉包で消えない借用衝突は 2 つあり、どちらも Rust そのものの制約である。

1 つ目は「ループで回している state をループ内のハンドラが変更する」場合である。`for` の展開は読み取りを共有借用で行うので、ハンドラ内の `todos.remove(i)` はコンパイルエラーになる(実行時 panic ではない)。対処は `todos.update_later(move |t| { t.remove(i); })` で、パス末に適用される書き込みキューに積む。egui の「削除は後でやる」慣習に対応する。キューはパス末まで生きるので閉包は `'static` であり、ループ変数のようなローカルを使うには `move` が要る。借用したい場合は `Dispatch` か値の clone を使う。`update_later` は guard が生きたままでも呼べる(適用時には guard はとっくに落ちている)。

2 つ目は「同一要素に、state を借用する値 prop と、同じ state を変更するハンドラを両方渡す」場合である。`<Dialog title={&*title} on_rename={|s| *title = s} />` は、props 構造体が `&str` の共有借用を持ち、融合閉包が同じ state の可変借用を持つので E0502 になる(spike で確認)。対処は値を先にコピーする(`title={title.clone()}`)か、書き込みを `update_later` に回すかのどちらかで、いずれもユーザーが書く。props は既定で借用(ゼロコピー)のままとし、`rsx!` に暗黙のクローンは入れない。フェーズ 3 の examples で頻出して苦痛なら、`TextEdit` の `bind` のように「読み書きを 1 つの `&mut` で渡す」形のコンポーネント設計で回避する。

## 4. hooks 一覧

| hook | 意味論 |
|---|---|
| `use_state(cx, init) -> State<T>` | ストアに `T` を保持。初回のみ `init` を呼ぶ |
| `use_persisted(cx, "key", init) -> State<T>` | `T: Serialize + Deserialize`。eframe の storage に保存し再起動を跨ぐ。キーは明示文字列 |
| `use_memo(cx, deps, f) -> &T` | deps のハッシュが変化した時のみ `f` を再実行 |
| `use_effect(cx, deps, f)` | deps 変化時(と初回)に `f` を**その場で**実行。`f` は cleanup(`FnOnce + 'static`)を返してよい |
| `use_reducer(cx, reducer, init) -> (State<S>, Dispatch<Msg>)` | `Dispatch` は `Clone + Send + 'static`。`send` はキューに積んで `request_repaint` し、**次に hook を訪問した時**に reducer を順に適用する(下記) |
| `provide_context(cx, handle, children)` / `use_context::<T>(cx) -> Option<Handle<T>>` | 子孫レンダリング中だけ有効。guard ではなく `Handle` を返す(親の guard と二重借用しないため)。ストアは `(TypeId, スロット Id)` のスタックを持ち、`use_context` がスロット Id から `Handle` を組み直す。`Handle` 自体は `'s` を持つので `dyn Any` には入れられない |
| `use_future(cx, deps, async_fn) -> &Poll<T>` | native は thread / tokio、wasm は wasm-bindgen-futures で実行。完了時に `request_repaint` |
| `cx.defer(f)` / `state.update_later(f)` / `handle.update_later(f)` | パス末(sweep の前)に実行される遅延キュー。閉包は `'static`。`update_later` は適用時に `request_repaint` するが、`defer` は状態に触れないのでしない |

`use_callback`、`memo` は提供しない。差分が無いので参照同一性を保つ意味がない。フレームを跨ぐ callback の代替は `Dispatch` である。

### `use_effect` の詳細

- deps は `Hash` で比較する。`PartialEq + Clone + 'static` を要求すると `(&str, &[T])` のような借用 deps が書けない。ハッシュ衝突は egui の Id と同程度に無視できる。
- 本体は呼び出し位置でその場で実行する。React が commit 後に回すのは DOM 実測のためだが、egui では `Response` の rect がウィジェット呼び出し直後に手元にある。後回しにすると本体が `'static` になり Yew の儀式が復活する。その場実行なら `State` の guard もローカルも借用できる。
- 本体の戻り値は `()` か cleanup 閉包のどちらでもよい。これを受ける `IntoCleanup<Marker>` の 2 つの blanket impl(`()` と `FnOnce() + 'static`)は coherence で衝突するので、マーカー型引数で区別する。マーカーは常に推論され、呼び出し側には現れない。
- cleanup は保存されるので `'static`。抱えるのは本体が作ったもの(task handle、購読解除子)なので自然に満たせる。unmount 時に他の state を変えたい場合は `Dispatch` を抱える。
- 前回の cleanup があれば本体の前に走らせる。unmount はパス末の sweep で検出して cleanup を走らせる。
- 多重パスの 2 パス目では deps が一致するので再実行されない。

### `use_memo` の詳細

- 返り値は `&'s T`(`'s` はストアの lifetime)で、`&mut Cx` の借用とは独立なので `State` の guard や後続の hooks と同時に生きられる。
- 値は `RefCell` の外、スロット上の `elsa::FrozenVec<Box<dyn Any>>` に積む。deps のハッシュが変われば新しい値を push して新しい参照を返す。古い値は、同じパス内で先に配った `&'s T` が指している可能性があるので消さず、パス末の sweep で最新の 1 つを残して落とす。
- deps の比較は `use_effect` と同じ Hash である。

### `use_reducer` の詳細

- メッセージは「パス末」ではなく「次に hook を訪問した時」に適用する。理由は 2 つ。(a) パス末に適用するには reducer を保存する必要があり `'static` になるが、訪問時なら reducer は通常の閉包でよい。(b) 別スレッドから届いたメッセージがパス末適用だと「次のパスの本体が古い状態を見て、そのパス末で適用され、さらに次のフレームで表示」となり 1 フレーム余計に遅れる。訪問時適用なら `send` の `request_repaint` で来る次のフレームの本体が新しい状態を見る。
- 訪問のたびにキューを空にするので、同一フレームの 2 パス目でメッセージが二重に適用されることはない。
- ハンドラから `send` した場合の見え方(次フレームで反映)は `State` への書き込みと同じで変わらない(5.7)。
- スロットは 2 つ使う。state 側は素の `S` を持ち(`State` と `update_later` がそのまま downcast できる)、メッセージキューは `Arc<Mutex<Vec<M>>>` を持つ別スロットに置く。

## 5. ランタイム

### 5.1 ストア

`Store` は slot の slab。slot は `RefCell<Box<dyn Any>>` と `last_visited: u64`(パス番号)を持つ。Id → slot の map を持つ。ストアはランナーの `App` 構造体が所有する(egui の `Context::data()` には入れない。テストしやすさのため)。

### 5.2 sweep

パス末に `last_visited` が現在パスより古い slot を列挙し、cleanup を走らせて破棄する。これが unmount であり、同時に状態のメモリリークを防ぐ。sweep 時点で guard は全て落ちている(コンポーネント本体と共に死ぬ)。`Handle` も同様で、`end_pass` は `&mut Store` を取るため、ストアを借用する `State` / `Handle` が生きたまま sweep が走ることは型で禁止されている。context スタックは `begin_pass` で防御的にクリアする。

### 5.3 多重パス

egui_taffy はレイアウト変化時に `request_discard` を呼び、同一フレーム内に 2 パス目を走らせる。egui の `Context::run` は各パスで `new_input.take()` を渡し、`RawInput::take` は `events: core::mem::take(&mut self.events)` でイベントを移動する。**2 パス目は空イベントで走るのでハンドラは 1 回しか発火しない**(egui ソースで確認済み)。状態の巻き戻しやジャーナルは不要。deps が変わっていない effect は 2 パス目で再実行されない。ただし 1 パス目のハンドラが変更した state から deps を導出している effect で、effect がハンドラより前に置かれている場合は、2 パス目で deps が本当に変わっているので実行される。これは「次フレームで走るはずだった 1 回」が同一フレームの 2 パス目に前倒しされただけで、deps の変化 1 回につき実行は 1 回である(spike のテストで確認)。ランナーは `Options::max_passes = 2` を設定する。パス数の確認には `egui::Context::current_pass_index()` を使う。

### 5.4 1 フレームの流れ

1. 入力を受け取り、ルートの `rsx!` が走る。
2. 各コンポーネントが `use_state` でストアから guard を取り、ウィジェットを描く。
3. クリック等が起きた場合、その場でハンドラが走り `State` を書き換える。
4. コンポーネント本体を抜けると guard が落ちる(値はストアに直接書かれているので書き戻しは無い)。
5. パス末に遅延キュー(`defer`、`update_later`)を適用し、sweep が未訪問 Id を破棄して cleanup を走らせ、最後に Id 衝突のオーバーレイを描く。
6. `request_discard` されていれば空イベントで 2 パス目。
7. 次フレームは更新済みの値から描き始める。

### 5.5 遅延キュー

`cx.defer(f)` と `state.update_later(f)` / `handle.update_later(f)` は `Store` の 1 本のキュー(`Vec<Box<dyn FnOnce(&Store)>>`)に積まれ、`end_pass` の先頭、sweep より前に空になるまで適用される。sweep より前なので、そのパスで unmount されるスロットへの書き込みも届く(スロットが既に無ければ黙って捨てる)。`update_later` は適用時に `request_repaint` し、`defer` はストアに触れないのでしない。

`Dispatch::send` はこのキューには入らない。メッセージは `use_reducer` を次に訪問した時に適用される(4 章)。

### 5.6 repaint ポリシー

- `State` の `DerefMut` が呼ばれたら dirty とし、guard の Drop で `request_repaint`。
- 遅延キューの適用で状態が変われば `request_repaint`。
- `use_future` の完了、`Dispatch::send`(別スレッドから)は `request_repaint`。これを忘れると非同期結果が届いてもマウスを動かすまで画面が変わらない。
- 裏返しとして、毎パス state を書き換えるコンポーネントは毎パス repaint を要求し、アプリがアイドルにならない(egui_kittest の `Harness::run` は `ExceededMaxSteps` で panic する)。React の「render 中に setState」と同じ無限ループであり、アニメーション以外では避ける。

### 5.7 1 フレーム遅れ

ハンドラや effect が state を書き換えると、同じコンポーネント内でそれより前に描かれたウィジェットには次フレームで反映される。これは egui アプリの通常の性質で、React の「setState は次レンダで反映」に対応する差異として明示する。

## 6. レイアウト

Flexbox / Grid を一級市民にするため egui_taffy を採用する(0.14、egui 0.36、taffy 0.9 対応)。React Native と同じく「`<View>` が taffy ノード、egui ウィジェットは leaf」とする。

```rust
<View direction="row" justify="space-between" align="center" gap={8} p={12}>
    <Text grow={1}>"Title"</Text>
    <Button onclick={|| *open = true}>"Open"</Button>
</View>
```

- `Cx` が「今 taffy コンテナの中か」を `Surface` として持つ(3.1)。`cx.leaf(&style, f)` は中なら `tui.style(style.to_taffy()).ui(f)`、外なら素の `ui` に流す。`cx.container(id, style, f)` は中なら子ノードの追加、外なら新しい `egui_taffy::tui(..)` ツリーの開始で、いずれも `f` には Taffy モードの `Cx` を渡す。
- Ui モード直下の `container` は `reserve_available_width()` を既定とし、ランナーのルートだけ `reserve_available_space()` を使う。
- egui 標準の `<Vertical>` / `<Horizontal>` / `<Grid>` も leaf として残し、パフォーマンスが要る箇所の逃げ道にする。Taffy モードから呼ばれた場合、これらの egui-native なコンテナは 1 つの leaf として振る舞い、その中の子は Ui モードで描かれる。
- `<Text>` はデフォルトの wrap を `Extend` にし、egui_taffy が警告する「テキストが縦一列になる」問題を避ける。
- ルートパネルは既定で `direction="column"` の `<View>` で包む。

### レイアウト属性

`react_egui::layout` に置く。taffy の型は `egui_taffy::taffy` を `react_egui::taffy` として re-export したものを使う。

- `Length`: `Px(f32)` / `Percent(f32)`(taffy と同じく 0.0〜1.0 の割合)/ `Auto`。`From<f32>` と `From<i32>` は `Px`、`From<&str>` は `"auto"` / `"50%"` / `"12px"` / `"12"` をパースし、それ以外は panic する。
- `ItemStyle`: 全要素が共通で受け付ける item 側の属性。`w h min_w min_h max_w max_h grow shrink basis align_self m mx my mt mr mb ml p px py pt pr pb pl`。setter は `impl Into<Length>` を取るので `rsx!` は数値リテラルも文字列リテラルもそのまま渡せる。`m` / `p` の短縮形は「全体 → `x` / `y` → 各辺」の順で、より具体的な指定が勝つ。`to_taffy()` で `taffy::Style` になる。
- `ContainerStyle`: `<View>` が受け付ける親側の属性。`display direction wrap justify align align_content gap cols`。`merge(&ItemStyle)` で item 側と合わせた 1 つの `taffy::Style` を作る(taffy のノードは自分の item 属性と子への container 属性を 1 つの `Style` に持つため)。`display="grid"` のときだけ `cols` が等幅カラムになる。
- `Direction` / `Justify` / `Align`(= `AlignSelf`)/ `Display` は enum で、`From<&str>` が CSS 綴り(`"row"`, `"space-between"`, `"center"`, `"grid"` など)をパースする。不正な文字列は候補を並べて panic する。`Justify` と `Align` は既定値 `Normal` を持ち、これは「未指定」を意味して taffy 側では `None` になる。`Option<Justify>` に `From<&str>` を実装することは orphan rule で不可能なので、Option ではなく `Normal` variant で「未指定」を表す。

却下した代替案: egui_flex。egui 0.35 止まりで、`justify-content` 未実装、`flex-shrink` は構造的に不可能と README 自身が述べている。一級市民には足りない。

## 7. クレート構成

```
react-egui/            core: View, Cx, Store, State, Handle, Dispatch, hooks, sweep, 遅延キュー
react-egui-macros/     rsx! (rstml 0.13 ベース), #[component], #[hook]
react-egui-elements/   egui ウィジェット / コンテナのラッパー。View / Text は taffy 上に
react-egui-app/        run(|cx| rsx!{..})。eframe を包み native / wasm / Android を吸収。iOS ランナーもここ
examples/              counter, todo (use_reducer), fetch (use_future), layout, mobile
```

egui は 0.36 系に固定する。egui 0.35 以降 `eframe::App::ui` が `&mut Ui` を受け取るので、ランナーはそれをそのまま `Cx` に包む。`react-egui`(core)は `egui_taffy` を通常依存に持つ。`Cx` の `Surface` が `Tui` を知る必要があるためで、wasm ターゲットでもそのままビルドできる。

## 8. プラットフォーム

- native / wasm / Android: eframe。wasm は trunk でビルドする。
- iOS: eframe は未対応(emilk/egui#3117 が open)。`egui-winit` + `egui-wgpu` の薄いランナーを `react-egui-app` 内に書く。ビルドは cargo-mobile2。
- ライブラリ本体は `&mut egui::Ui` しか触らないので、プラットフォーム対応はランナー層とタッチ / IME の調整に閉じる。

## 9. テスト

- `egui_kittest` でコンポーネントの操作テストとスナップショットテスト。
- `trybuild` でマクロのコンパイルエラー(イベント名の誤り、`key` 忘れ等)の文面を固定する。
- スパイク段階から kittest を使い、多重パスでハンドラが 1 回だけ発火することをテストで固定する。

## 10. スパイクで検証した項目

マクロ抜きの手書き展開で以下を確認した(PR1、`crates/react-egui/tests/`)。全項目にテストがあり緑。前提が崩れた箇所は本書の該当節に反映済み。

- `State` の guard がコンポーネント本体の間だけ生き、兄弟要素のハンドラが同じ state を順に `&mut` 借用できる。
- `#[hook]` 越しに guard を返せる(lifetime `'s` が `&mut Cx` と独立である)。
- 融合閉包の展開形がコンパイルし、`Handler` trait で `FnMut()` / `FnMut(A)` を統一的に呼べる。
- `use_context` の `Handle` と親の guard が共存できる。
- egui_taffy の 2 パス目でクリックハンドラが 1 回だけ発火し、effect が再実行されない。
- Id 衝突検出が `#[hook]` 忘れと `key` 忘れを捕まえる。
- sweep が `if` で消えたコンポーネントの状態を破棄し cleanup を走らせる。
- egui のコンテナ閉包(`ui.vertical`)の内側で新しい `Cx` を作り、そこで hooks が動く。

## 11. 決定ログ

| 決定 | 採用 | 却下 | 理由 |
|---|---|---|---|
| ホスト言語 | Rust のみ | JS React + JS ランタイム | 規模が数倍、iOS の JIT 制限、wasm-bindgen 層 |
| ツリー | 直接展開 | VNode 保持 + 走査 | ハンドラが `'static` + `Rc` になり Yew の苦痛が戻る |
| hook Id | 呼び出し位置スタック + 衝突検出 | 出現回数を混ぜる | 後者は静かに状態がずれ、検出不能 |
| State | guard(`Deref/DerefMut`)+ 補助 `Handle` | Cell 風 `Handle` のみ | `*count += 1` を残す。衝突は融合閉包で消える |
| callback props | `on_*` を enum に融合 | 戻り値でイベント返却 / 単一 `callback` 手書き | 借用ペイロード、複数イベント、冗長さ |
| effect のタイミング | 呼び出し位置で即時 | commit 後 | egui に commit が無く、後回しは `'static` を招く |
| deps 比較 | Hash | PartialEq + Clone | 借用 deps を許すため |
| レイアウト | egui_taffy | egui_flex | justify / shrink 未対応、egui 追従が遅い |
| 多重パス対策 | 不要 | スナップショット巻き戻し | egui が 2 パス目のイベントを空にすることを確認 |
| ストアの置き場 | ランナーの `App` | `Context::data()` | テストしやすさ |
| `Handler` の実装 | マーカー型引数で 2 つの blanket impl を共存 | 引数個数で `call0` / `call1` を選ぶ | マーカーは常に推論され、マクロは 1 形式だけ emit すればよい |
| 値 prop とハンドラの借用衝突 | ユーザーが clone か `update_later` | `rsx!` が暗黙に clone | 型情報なしに clone を挿入できず、ゼロコピーを既定にしたい |
