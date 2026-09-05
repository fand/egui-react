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
- web でのスクリーンリーダー対応。egui / AccessKit の web 対応に依存する。egui はウィジェットツリーを AccessKit に出しており、native では OS のアクセシビリティ API に届くが、web ではそれを DOM に映す adapter が上流に無い(`docs/tasks/a11y/`)。要素のラベル(`Button` の `label`、`Image` の `alt`)は adapter が入った日にそのまま効くので、先に埋めてある。

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
| `leaf_fill(&ItemStyle, f)` | `leaf` と同じだが内容サイズを報告しない(`min_size = 0`、`infinite`)。`ScrollArea` のように「与えられた空間を埋めてその大きさを返す」ウィジェット用で、これを `leaf` で置くと最初のフレームの大きさに固定される。大きさは `w` / `h` / `grow` / 残り空間で taffy が決める |
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

- 要素: `<Button on_click={..}>"text"</Button>`。要素名はすべて Rust の関数コンポーネント(`<elements::Button/>` のようなパスも書ける)。HTML 風の小文字タグは持たない。
- 式埋め込み: `{expr}`。`expr: impl View`。引用符の無いテキストはエラーで、文字列は必ずリテラルで書く。
- 属性: `key={expr}`、`on_*={handler}`、`events={closure}`、レイアウト属性(`w` / `h` / `grow` / `p` / `m` など。まとめて `.style(ItemStyle::default()..)` になる)、それ以外は Props の setter。値の無い属性(`disabled`)は `true`。
- 子ノードは常に `.children(..)` で渡る。子が無ければ `()`、単一の文字列リテラルか単一の `{expr}` ならその式そのもの、それ以外は `view(|cx| ..)`。これにより `<Button>"OK"</Button>` の `children: impl Into<WidgetText>` と `<View>..</View>` の `children: impl View` が同じ構文で書ける。
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

- Props 構造体 `CounterProps`。typed-builder の `#[derive(TypedBuilder)]` が付き、`Counter(cx, props)` の第 2 引数になる。関数の引数はそのまま Props のフィールドになり、`&T` の省略ライフタイムは Props の `'e` に書き換わる。`impl Trait` の引数は型パラメータに脱糖する。
- 省略可能な prop は `Option<T>` 型の引数(自動)と `#[prop(default)]` / `#[prop(default = expr)]` を付けた引数。`#[prop(into)]` を付けると setter が `impl Into<T>` を取る。それ以外は必須で、省略すると typed-builder のコンパイルエラーになる。
- `children` フィールドは必ず存在する。宣言しなければ `children: ()` が `#[builder(default)]` で生成される(`rsx!` が常に `.children(..)` を呼ぶため)。子を受け取るコンポーネントは `children: impl View` を宣言する。
- `#[event]` 引数からイベント enum `CounterEvent` と `events` フィールド(3.6 参照)。
- `Props` trait の実装。`rsx!` が `props_builder(&Counter)` から builder を引くために使う。

本体の末尾式は `::react_egui::View::show(tail, cx)` に書き換わる。本体全体を `View::show({ body }, cx)` で包む形は、ブロック内のローカルを guard が借用したまま返すことになり通らないので、必ず末尾式だけを差し替える。`if` / `match` の腕ごとに別々の `rsx!` を返す本体は閉包の型が一致しないので、`rsx!{ if .. }` の形で書く。

`<Counter initial={0} />` は次のように展開される。

```rust
cx.scope((file!(), line!(), column!(), 3usize, key), |cx| {
    Counter(cx, ::react_egui::props_builder(&Counter).initial(0).children(()).build());
});
```

`scope` は `cx.scope` を一段深くし、同時に `ui.push_id`(Taffy モードでは `tui.with_auto_id_prefix`)を呼ぶ。これにより hooks の Id と egui 側のウィジェット Id の両方がコンポーネントインスタンスごとに安定する。Id の材料は `rsx!` 呼び出し位置と、その `rsx!` 内での要素の通し番号、そして `key` である。関数アイテムの型は名指しできないので、Props の型は `props_builder<P: Props, F: Fn(&mut Cx, P)>(_: &F) -> P::Builder` の `Fn` 境界から推論する。ユーザーが `use` するのは `Counter` だけでよい。

要素ごとに子 `Ui` を作ると、親の `Ui` から場所を切り取る egui のコンテナ(ドッキングされたパネル)や、親の `Ui` を書き換えるもの(`Grid` の `Ui::end_row`)が動かない。そこで `#[component(shares_ui)]` を用意する。これを付けたコンポーネントは hook のスコープは通常どおり深くなるが、`Ui::push_id` を通らず親の `Ui` にそのまま描く。実装は `Props` の `const SHARES_UI: bool`(既定 `false`、`#[component(shares_ui)]` が `true` にする)で、`rsx!` は要素の呼び出しを `::react_egui::__private::enter_scope(cx, source, props, Name)` に通す。`enter_scope` は `P::SHARES_UI` を見て `cx.scope` か `cx.scope_sharing_ui` を選ぶ。`Panel` / `CentralPanel` / `Row` がこれを使う。

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
- コンポーネント側では `#[event] on_ok: ()` が `Emitter` になり、`on_ok.emit(())` で発火する。`Emitter<'a, 'e, E, A>` は共有された `EventSink<'e, E> = RefCell<&'e mut dyn FnMut(E)>` への `&'a` 参照と、ペイロード `A` を variant に包む関数 `fn(A) -> E` を持つ。子の中で複数同時に生きられる(`RefCell` は不変なので lifetime は 2 つ必要)。再入的な emit は明確なメッセージで panic する。
- Props の `events` フィールドは `Option<&'e mut dyn FnMut(E)>` で、`#[builder(default, setter(strip_option))]` が付く。`rsx!` は融合閉包を `&mut` で渡し、`on_*` が 1 つも無ければ渡さない。渡されなかった場合、コンポーネント本体は `match` の腕でローカルの no-op 閉包に落とすので `emit` は何もしない。
- `on_*` を 1 つも渡さず `events={|e| match e {..}}` と書く escape hatch も通す。`rsx!` は式を `&mut (..)` で包んで `events` に渡す。
- ペイロードは借用でよい(`on_change: &str` が可能)。イベント enum は、ペイロードが実際に使うジェネリクスだけを引き継ぐ(`&'e str` なら `CounterEvent<'e>`)。
- 融合閉包は `CounterEvent::Ok(..)` を名指しするので、`<Counter on_ok=../>` と書く場所では `Counter` に加えて `CounterEvent` も import されている必要がある(モジュールを glob で `use` するか、`<components::Counter/>` のようにパスで書く)。

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
| `use_persisted(cx, "key", init) -> State<T>` | `T: Serialize + DeserializeOwned`。eframe の storage に保存し再起動を跨ぐ。キーは明示文字列(下記) |
| `use_memo(cx, deps, f) -> &T` | deps のハッシュが変化した時のみ `f` を再実行 |
| `use_effect(cx, deps, f)` | deps 変化時(と初回)に `f` を**その場で**実行。`f` は cleanup(`FnOnce + 'static`)を返してよい |
| `use_reducer(cx, reducer, init) -> (State<S>, Dispatch<Msg>)` | `Dispatch` は `Clone + Send + 'static`。`send` はキューに積んで `request_repaint` し、**次に hook を訪問した時**に reducer を順に適用する(下記) |
| `provide_context(cx, handle, children)` / `use_context::<T>(cx) -> Option<Handle<T>>` | 子孫レンダリング中だけ有効。guard ではなく `Handle` を返す(親の guard と二重借用しないため)。ストアは `(TypeId, スロット Id)` のスタックを持ち、`use_context` がスロット Id から `Handle` を組み直す。`Handle` 自体は `'s` を持つので `dyn Any` には入れられない |
| `use_future(cx, deps, \|\| async { .. }) -> &Poll<T>` | deps のハッシュが変わるたびに future を作り直して起動する。native はスレッド 1 本 + `pollster::block_on`、wasm は `wasm_bindgen_futures::spawn_local`。完了時に結果をスロットへ書いて `request_repaint`。deps 変更後に届いた古い結果は捨てる。`Pending` は最も近い `<Suspense>` に数えられる(下記) |
| `spawn(fut)` | hook ではないが同じ実行機構。起動して忘れる。結果は `Dispatch` で送る(`spawn(async move { dispatch.send(Msg::Saved(api.await)) })`) |
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

### `use_persisted` の詳細

- スロットの Id はスコープではなく `Id::new(("react_egui_persisted", key))` で、呼び出し位置に依存しない。行を足しても保存データが読めなくなることが無い代わりに、同じキーを 2 か所で使えば同じ 1 つの値を共有し、同じパスで 2 回訪問すれば通常どおり衝突として記録される。
- `Store` は「キー → JSON 文字列」の `HashMap` を持つ。`load_persisted(&mut self, json)` が丸ごと読み込み、`save_persisted(&self) -> String` が生きているスロットを直列化して map に上書きしてから全体を JSON にする。読めない JSON は `log::warn!` して無視し、値は `init` に落ちる。
- `Slot` は `persist: Option<(key, fn(&dyn Any) -> Option<String>)>` を持つ。sweep で persist 付きスロットを落とす時は、先に直列化して map に書く。unmount した後でも次回起動には残る。
- 保存形式は JSON、eframe の `Storage` には `"react_egui"` の 1 キーにまとめて書く。ランナーの `App::save` が呼ぶ(eframe が `auto_save_interval` と終了時に呼ぶ)。

### `use_reducer` の詳細

- メッセージは「パス末」ではなく「次に hook を訪問した時」に適用する。理由は 2 つ。(a) パス末に適用するには reducer を保存する必要があり `'static` になるが、訪問時なら reducer は通常の閉包でよい。(b) 別スレッドから届いたメッセージがパス末適用だと「次のパスの本体が古い状態を見て、そのパス末で適用され、さらに次のフレームで表示」となり 1 フレーム余計に遅れる。訪問時適用なら `send` の `request_repaint` で来る次のフレームの本体が新しい状態を見る。
- 訪問のたびにキューを空にするので、同一フレームの 2 パス目でメッセージが二重に適用されることはない。
- ハンドラから `send` した場合の見え方(次フレームで反映)は `State` への書き込みと同じで変わらない(5.7)。
- スロットは 2 つ使う。state 側は素の `S` を持ち(`State` と `update_later` がそのまま downcast できる)、メッセージキューは `Arc<Mutex<Vec<M>>>` を持つ別スロットに置く。

### `use_future` の詳細

- 実行機構の platform 差は `SpawnFuture<T>` trait と core の `task::spawn` に閉じる。bound は native が `Future<Output = T> + Send + 'static`、wasm が `Future<Output = T> + 'static` で、cfg で切り替えた 2 つの blanket impl で同じ名前にする。ユーザーが書く型にこの差は出ない。結果の型 `T` は両 platform で `Send + 'static` を要求する。bound を platform ごとに変えるのは future 側だけにして、ユーザーの型を 1 種類で済ませるためである(wasm で `JsValue` を返したい場合は future の中で `Send` な型に変換する)。
- native の executor はスレッド 1 本 + `pollster::block_on`。future 1 つにつきスレッド 1 本で、プールは作らない。`ehttp` やファイル IO のような「待つだけ」の future に十分で、依存も最小。CPU を食う処理は future の中で自分でスレッドを分ける。スレッド生成に失敗した場合は future を捨てて `log::error!` するだけで、panic しない(hook は `Pending` のまま止まる)。tokio が要るアプリは future の中で `Handle::current().spawn(..).await` する。
- `f` は呼び出し位置でその場で呼ぶ(`use_effect` と同じ)。ローカルや `State` の guard を読んで future を組み立ててよい。future 自身は `'static` なので、`move` で clone した値を持つ。
- スロットは 2 つ使う(`use_reducer` と同じ分け方)。**状態スロット**は deps のハッシュと `use_memo` と同じ `FrozenVec` を持ち、起動時に `Poll::Pending` を、結果が届いた時に `Poll::Ready(T)` を push する。**受信スロット**は `Arc<Mutex<Option<(u64, T)>>>`(世代付きの結果)と `Cell<u64>`(最後に起動した世代)を持つ。返り値は `use_memo` と同じ `&'s Poll<T>` で、`State` の guard と同時に生きる。古い `Poll` は同じパスで先に配った参照が指している可能性があるので、パス末の sweep(既存の `prune_memo`)で最新の 1 つだけを残す。
- 起動の直後は `Pending` を返すだけで受信を試みない。その場で完了していても次のフレームで拾う(完了時に `request_repaint` が飛ぶので取りこぼさない)。同一フレームの 2 パス目は deps が一致するので起動せず、受信を試みるだけである。
- deps を変えて future を作り直すのは訪問時。走っている古い future は止められないので完走するが、結果は世代不一致で捨てられる。訪問の前に「新しい方 → 古い方」の順で両方が届くと古い方が新しい結果を上書きしてしまうので、future 側の書き込みを「セルが空か、入っている世代より新しい時だけ」に限る。
- unmount では 2 つのスロットが sweep で落ちる。走っている future は `Arc` の自分の側を持っているので書き込みは成功し、`request_repaint` が 1 回余分に飛ぶ。次のフレームで誰も読まず、`Arc` の最後の参照が future の終了と共に消える。
- `Pending` を返す直前に `Store::note_pending()` を呼び、最も近い `<Suspense>` 境界のカウンタを +1 する(5.8)。境界が無ければ何もしない。
- 子コンポーネントの書き方は `let Poll::Ready(x) = use_future(..) else { return };`。React の throw の代わりで、境界が fallback を描く。

## 5. ランタイム

### 5.1 ストア

`Store` は slot の slab。slot は `RefCell<Box<dyn Any>>` と `last_visited: u64`(パス番号)を持つ。Id → slot の map を持つ。ストアはランナーの `App` 構造体が所有する(egui の `Context::data()` には入れない。テストしやすさのため)。

### 5.2 sweep

パス末に `last_visited` が現在パスより古い slot を列挙し、cleanup を走らせて破棄する。これが unmount であり、同時に状態のメモリリークを防ぐ。sweep 時点で guard は全て落ちている(コンポーネント本体と共に死ぬ)。`Handle` も同様で、`end_pass` は `&mut Store` を取るため、ストアを借用する `State` / `Handle` が生きたまま sweep が走ることは型で禁止されている。context スタックは `begin_pass` で防御的にクリアする。

### 5.3 多重パス

egui_taffy はレイアウト変化時に `request_discard` を呼び、同一フレーム内に 2 パス目を走らせる。egui の `Context::run` は各パスで `new_input.take()` を渡し、`RawInput::take` は `events: core::mem::take(&mut self.events)` でイベントを移動する。**2 パス目は空イベントで走るのでハンドラは 1 回しか発火しない**(egui ソースで確認済み)。状態の巻き戻しやジャーナルは不要。deps が変わっていない effect は 2 パス目で再実行されない。ただし 1 パス目のハンドラが変更した state から deps を導出している effect で、effect がハンドラより前に置かれている場合は、2 パス目で deps が本当に変わっているので実行される。これは「次フレームで走るはずだった 1 回」が同一フレームの 2 パス目に前倒しされただけで、deps の変化 1 回につき実行は 1 回である(spike のテストで確認)。ランナーは `Options::max_passes = 3` を設定する。2 ではなく 3 なのは、`<View>` の中の egui コンテナ(`ScrollArea` など)の中の `<View>` が別の egui_taffy ツリーになり、外側のツリーが 2 パス目で決めたサイズを内側が知るのは 3 パス目だからである。パス数を使い切って discard が却下された場合、ランナーは `request_repaint` して次フレームで収束させる(そうしないと次の入力まで古いレイアウトのまま止まる)。パス数の確認には `egui::Context::current_pass_index()` を使う。

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

- `State` の `DerefMut` が呼ばれたら dirty とし、guard の Drop で `request_repaint`。例外は `State::bind()` で、これは `&mut T` を渡すだけで dirty にしない。`TextEdit` のような bind 系ウィジェットに `&mut *state` を渡すと毎フレーム dirty になり、アプリがアイドルにならない(egui_kittest の `Harness::run` が `ExceededMaxSteps` で panic する)。値が変わるのは入力があった時だけで、その時は egui が自分で repaint するので取りこぼさない。
- 遅延キューの適用で状態が変われば `request_repaint`。
- `use_future` の完了、`Dispatch::send`(別スレッドから)は `request_repaint`。これを忘れると非同期結果が届いてもマウスを動かすまで画面が変わらない。
- 裏返しとして、毎パス state を書き換えるコンポーネントは毎パス repaint を要求し、アプリがアイドルにならない(egui_kittest の `Harness::run` は `ExceededMaxSteps` で panic する)。React の「render 中に setState」と同じ無限ループであり、アニメーション以外では避ける。

### 5.7 1 フレーム遅れ

ハンドラや effect が state を書き換えると、同じコンポーネント内でそれより前に描かれたウィジェットには次フレームで反映される。これは egui アプリの通常の性質で、React の「setState は次レンダで反映」に対応する差異として明示する。

### 5.8 Suspense

`<Suspense fallback={..}>children</Suspense>`(6 章、`react-egui-elements`)は、中の `use_future` が 1 つでも `Pending` なら children の代わりに `fallback` を描く。React の throw に当たるものが Rust には無いので、子は `let Poll::Ready(x) = use_future(..) else { return };` で抜け、`Pending` の数を `Store` のカウンタで数える。

- **カウンタのスタック** `Store` は `provide_context` と同じ形で `RefCell<Vec<usize>>` を持つ。`begin_suspense` が 0 を積み、`end_suspense` が積んだ数(= 中で `Pending` だった `use_future` の数)を返し、`note_pending` が最も近い(= 一番内側の)カウンタを +1 する。入れ子は内側が自分の分を消費するので、外側には数えられない。スタックは `begin_pass` で clear する。core に足すのはこの 3 メソッドだけで、境界そのものは elements にある(`Collapsing` と同じ「子を包む要素」)。
- **初期状態は suspended** 初回はオフスクリーンに描いてから、`Pending` が無ければ可視に切り替える。`max_passes` を使い切って `request_discard` が却下された時に、描きかけの children ではなく `fallback` が見える側に倒すためである。
- **suspended 中も children は描く** オフスクリーンの不可視 `Ui`(`egui::Ui::new(ctx, id, UiBuilder::new().max_rect(画面外の固定矩形).invisible().sizing_pass())`)に描く。hooks が走り、future が起動して完了する。矩形を固定値にしてあるのは、egui_taffy が毎パス同じ大きさを見て無駄な `request_discard` を出さないようにするため。`invisible()` は描画と操作の両方を無効にするので、children のハンドラはオフスクリーンでは発火しない。ただし egui はウィジェットの accessibility ノードを可視性に関係なく作るので、スクリーンリーダーと `egui_kittest` からは suspended 中の children も「画面外の座標にあるノード」として見える。
- **children のスコープ** suspended / 可視のどちらでも `Suspense` 自身の `scope_id()` を使う(`Cx::new(store, &mut ui, scope)` の第 3 引数)。hook のスロットが両経路で同じ Id になることが、切り替えで state と future が保たれる根拠である。`fallback` は同じ `cx` に描くが、`rsx!` の要素 Id は行・列で分かれるので children と衝突しない。
- **切り替えは同一フレーム** 切り替えの瞬間に `Handle::set` で状態を反転し、`request_discard` で同じフレームをやり直す。描きかけの children も、fallback から children への 1 フレームの隙間も見えない。`Handle::set` は `request_repaint` するが、呼ぶのは切り替えの時だけなので suspended のまま毎フレーム repaint することはない。`max_passes`(ランナー既定 3)を使い切って discard が却下された場合は、ランナーが `request_repaint` して次フレームで揃う。
- **`shares_ui`** `Suspense` は自分の `Ui` / leaf を作らず、children と fallback を親の surface(Ui でも Taffy でも)にそのまま流す。`<View>` の中に置けば children の `<View>` は親の taffy ツリーの子になる。
- **React との差** suspended 中の children の `use_effect` は走る(React は commit しないので走らない)。React の `SuspenseList` / `useTransition` に当たるものは持たない。

## 6. レイアウト

Flexbox / Grid を一級市民にするため egui_taffy を採用する(0.14、egui 0.36、taffy 0.9 対応)。React Native と同じく「`<View>` が taffy ノード、egui ウィジェットは leaf」とする。

```rust
<View direction="row" justify="space-between" align="center" gap={8} p={12}>
    <Text grow={1}>"Title"</Text>
    <Button onclick={|| *open = true}>"Open"</Button>
</View>
```

- `Cx` が「今 taffy コンテナの中か」を `Surface` として持つ(3.1)。`cx.leaf(&style, f)` は中なら `tui.style(style.to_taffy()).ui(f)`、外なら素の `ui` に流す。`cx.container(id, style, f)` は中なら子ノードの追加、外なら新しい `egui_taffy::tui(..)` ツリーの開始で、いずれも `f` には Taffy モードの `Cx` を渡す。
- Ui モード直下の `container` は `reserve_available_width()` を既定とし、ランナーのルートだけ `reserve_available_space()` を使う。`grow` や `justify="space-between"` は余白の分配なので、`<View>` 自身に幅(`w`)が無いと効かない。`reserve_available_space()` は egui_taffy に「これだけ場所がある」と伝えるだけで、ルートノード自身の `size` は `auto` のままである。これが無いとルートノードは中身の大きさになり、2 つ壊れる。(a) `<View grow={1.0} justify="center">` は広がる余地も中央寄せする先も持たず、アプリは左上に寄る。(b) 入りきらない子が縮まない。親が中身に合わせて伸びるので overflow が発生せず、`flex-shrink` が働く前提が消えて、行が窓の右へはみ出す。そこでランナーのルート item style は `w` を `100%`(窓の幅は固定なので確定値)、`min_h` を `100%`(縦は中身が高ければ伸ばす)にする(7 章)。
- egui 標準の `<Vertical>` / `<Horizontal>` / `<Grid>` などのコンテナも leaf として残し、パフォーマンスが要る箇所の逃げ道にする。Taffy モードから呼ばれた場合、これらの egui-native なコンテナは 1 つの leaf として振る舞い、その中の子は Ui モードで描かれる。 `ScrollArea` だけは `leaf_fill`(3.1)で置く。与えられた空間を埋めるウィジェットなので、内容で測る leaf では最初のフレームの大きさに固定されてしまう。`<View>` の中の `ScrollArea` には `grow` か `h` を与える。
- テキストを持つ leaf は全て wrap を `Extend` にする(`Text` `Label` `Button` `Checkbox` `Slider` `ComboBox` のラベル、`Collapsing` のヘッダ)。egui_taffy は leaf を「前回描いた時の大きさ」で測り、その 1 つの値を min-content としても max-content としても taffy に返す。最初の描画は幅 0 の `Ui` で起きるので、wrap する widget はそこで「1 文字幅」を報告し、ノードはその細さのまま固定され、ラベルが 1 文字ずつ縦に並ぶ。`grow` や `w` を持つ leaf は taffy が幅を決めるので影響を受けない。`wrap_mode` の builder がある widget(`Button` / `Label`)はそれを使い、無いもの(`Checkbox` / `Slider` / `ComboBox` / `CollapsingHeader`)は leaf の `Ui` の `style.wrap_mode` に置く。`Collapsing` は本体に入る前に元の値へ戻す(children が描くものは呼び出し側の領分)。
- `<Text>` と `<Label>` は共に `wrap` 属性で egui 既定の折り返しに切り替えられる。折り返すには幅が要るので、`w` か(幅の決まったコンテナの中の)`grow` と併せて使う。両者の違いは `Text` が `size` / `color` / `strong` を持つことだけである。
- ルートパネルは既定で `direction="column"` の `<View>` で包む。

### 要素一覧(`react-egui-elements`)

全要素が `#[component]` で書かれ、`#[prop(default)] style: ItemStyle` を受け取る。`rsx!` はレイアウト属性をまとめて `style` に詰める。イベント enum も含めて `react_egui_elements::prelude` から re-export する(3.6 のとおり、`on_*` を使う場所には enum 名が必要なため)。

| 種類 | 要素 |
|---|---|
| レイアウト | `View`(`display` / `direction` / `wrap` / `justify` / `align` / `align_content` / `gap` / `cols`)、`Text`(`size` / `color` / `strong` / `wrap`) |
| ウィジェット | `Button`(`enabled` / `label`, `on_click`)、`Label`(`wrap`)、`TextEdit`(`bind` / `multiline` / `hint` / `desired_width` / `rows`, `on_change` / `on_submit`)、`Checkbox`(`bind` / `label`, `on_change`)、`Slider<T: Numeric>`(`bind` / `range` / `label`, `on_change`)、`ComboBox`(`bind` / `options` / `label`, `on_change`)、`Image`(`source` / `fit` / `alt`)、`Separator`(`vertical`) |
| コンテナ | `ScrollArea`、`VirtualList`(`rows` / `row_h` / `render`)、`Collapsing`、`Frame`、`Window`(`title` / `open` / `resizable` / `default_pos` / `default_size`)、`Panel`(`side`)、`CentralPanel`、`Vertical`、`Horizontal`、`Grid` + `row()` |
| 描画 | `Canvas`(`sense` / `paint`, `on_drag` / `on_hover`。taffy がくれた矩形をそのまま渡す leaf) |
| 非同期 | `Suspense`(`fallback: impl View`、`shares_ui`。中の `use_future` が 1 つでも `Pending` なら children の代わりに `fallback` を描く。5.8) |

`Button` の `label` と `Image` の `alt` は支援技術が読む名前である。`Button` の `label` は描くもの(children)を変えず、accesskit ノードの名前だけを差し替える(`Context::accesskit_node_builder`)。アイコンや `"x"` だけのボタンはそのままでは字面しか読まれないので渡す。`Image` の `alt` は `egui::Image::alt_text` に落ち、読み込みに失敗したときの ⚠ の隣にも描かれる。web での読み上げ自体は 1 章の非ゴールのとおり上流待ちだが、これらは native では今日から効く。

`TextEdit` は taffy の中(`Cx::in_taffy()`)ではノードを埋める。単行は `desired_width` をノードの幅にし、`multiline` は `ui.add_sized(ui.available_size(), ..)` で縦横とも埋める(`desired_rows` だと行単位にしか合わず、端数がノードからはみ出す)。`grow` や `w` で広げたノードの中に egui 既定の 280pt / 4 行で描かれると残りが空くためである。`desired_width` / `rows` を明示した場合はそちらが勝つ。`Slider` / `ComboBox` / `Button` は今のところ伸びない(それぞれ `spacing.slider_width` / `spacing.combo_width` / 内容の幅のまま)。

`bind` を持つ要素はウィジェットが直接 state に書き込むので、`State::bind()` を通す。これは `&mut *state` と違って state を dirty にしない(5.6)。同じ state を触るハンドラを同じ要素に渡すと E0502 になるので、`bind` 要素の `on_change` はログや `Dispatch` のように別の場所へ通知する用途に限る。

egui 標準のコンテナのうち、親から場所を切り取るもの(`Panel` / `CentralPanel`)と、親の `Ui` に依存するもの(`Grid` の行区切り)は、`rsx!` が要素ごとに `Ui::push_id` で子 `Ui` を作ることの影響を受ける。行区切りは要素ではなく `{row()}`(`{expr}` ノードはスコープされない)として提供する。これらは `#[component(shares_ui)]` を付けて親の surface をそのまま引き継ぐ。

**パネルが場所を切り取る先は「最も近い egui の `Ui`」、つまり今の taffy ツリーを開始した `Ui` である。** 間に `<View>` が何段あっても飛ばす。taffy モードのとき `Panel` / `CentralPanel` は `cx.leaf` を使わず `cx.ui()` に対して `show_inside` する。leaf を作ってしまうとパネルは自分専用の小さなノードの中を切り取ることになり、兄弟に並べた 4 つのパネルが全部同じ角に重なる。ランナーの下ではこの `Ui` は窓そのものなので、「パネルはアプリのルートで使う」は自動的に成り立つ。裏返しの帰結として、`<View>` の奥に書いた `<Panel>` はその行の一部ではなく窓の端まで飛ぶ。これは docking の意味であって不具合ではない(`examples/shell`)。`ScrollArea` は children をそのまま全部描く。長いリストは `VirtualList` を使う。`rows` と `row_h` を受け取り、`render(cx, i)` を「見えている行」にだけ呼ぶ(中身は `egui::ScrollArea::show_rows`)。行は `cx.scope(i, ..)` の中で描かれるので、`for` + `key={i}` と同じく行ごとに hook を持てる。全行が同じ高さであることが条件である。`render` の bound は `impl for<'a, 's, 'u> FnMut(&'a mut Cx<'s, 'u>, usize)` と明示して書く。`#[component]` は prop の省略ライフタイムを props 構造体のものに書き換えるので、省略形(`impl FnMut(&mut Cx, usize)`)はコンパイルできない。

`Canvas` は自分では何も描かない leaf である。`leaf_fill` で taffy がくれた大きさを `ui.allocate_exact_size(ui.available_size(), sense)` で確保し、`paint(ui, rect)` を呼ぶだけで、中身は呼び出し側が `ui.painter()` に積む(線や図形でも、`egui_wgpu::Callback::new_paint_callback(rect, ..)` でも)。elements は egui-wgpu に依存しない。`sense` は既定が `Sense::hover()` で、`on_hover`(矩形内のポインタ位置)はそのまま、`on_drag`(`drag_delta`)には `Sense::drag()` が要る。閉包 prop の名前は `on_paint` ではなく `paint` である。`rsx!` は `on_` で始まる属性を全てイベント enum の variant として扱うので、`on_` 始まりの普通の prop は書けない(`VirtualList` の `render` と同じ命名)。bound は `impl for<'a> FnOnce(&'a mut egui::Ui, egui::Rect)` と明示する(`VirtualList` と同じ理由)。

`Suspense` も同じ理由で `shares_ui` である。自分では何も描かず children と `fallback` を親にそのまま流すので、`<View>` の中に置けば children が親の taffy ツリーの子になる。

context の provider には `<Provide value={handle}>` のような汎用要素を用意できない。`provide_context` が受け取る `Handle<'s, T>` はストアを借りているのに対し、`props_builder` はコンポーネントに `for<'a, 's, 'u> Fn(&'a mut Cx<'s, 'u>, P)` を要求するので、props の型 `P` は `'s` を名乗れないためである(試すと `implementation of Fn is not general enough` になる)。同じ理由で `View` の閉包の中でも provide できない(`View::show` も `'s` について higher-ranked)。書ける形は「値を自分で作って自分で配る provider コンポーネント」で、`#[component(shares_ui)] fn Themed(cx, children: impl View)` の中で `use_handle` してから `provide_context(cx, handle, |cx| children.show(cx))` する(`examples/theme`)。React で provider が state を持つのと同じ形なので、実用上は困らない。

### レイアウト属性

`react_egui::layout` に置く。taffy の型は `egui_taffy::taffy` を `react_egui::taffy` として re-export したものを使う。

- `Length`: `Px(f32)` / `Percent(f32)`(taffy と同じく 0.0〜1.0 の割合)/ `Auto`。`From<f32>` と `From<i32>` は `Px`、`From<&str>` は `"auto"` / `"50%"` / `"12px"` / `"12"` をパースし、それ以外は panic する。
- `ItemStyle`: 全要素が共通で受け付ける item 側の属性。`w h min_w min_h max_w max_h grow shrink basis align_self m mx my mt mr mb ml p px py pt pr pb pl col_span row_span`。setter は `impl Into<Length>` を取るので `rsx!` は数値リテラルも文字列リテラルもそのまま渡せる。`m` / `p` の短縮形は「全体 → `x` / `y` → 各辺」の順で、より具体的な指定が勝つ。`to_taffy()` で `taffy::Style` になる。
- 短縮属性と `style={expr}` は同じ `style` prop を埋めるので、`rsx!` は 1 つの `.style(..)` にまとめる。両方あれば `style=` の式を起点に短縮属性を繋ぐ(`<Chip style={style} p={6}/>` は `.style((style).p(6))`)。これにより、`style: ItemStyle` を受け取るラッパーコンポーネントが呼び出し元のレイアウトをそのまま受けて自分の分を足せる。
- `ContainerStyle`: `<View>` が受け付ける親側の属性。`display direction wrap justify align align_content gap cols`。`merge(&ItemStyle)` で item 側と合わせた 1 つの `taffy::Style` を作る(taffy のノードは自分の item 属性と子への container 属性を 1 つの `Style` に持つため)。`display="grid"` のときだけ `cols` が等幅カラムになる。
- `Direction` / `Justify` / `Align`(= `AlignSelf`)/ `Display` は enum で、`From<&str>` が CSS 綴り(`"row"`, `"space-between"`, `"center"`, `"grid"` など)をパースする。不正な文字列は候補を並べて panic する。`Justify` と `Align` は既定値 `Normal` を持ち、これは「未指定」を意味して taffy 側では `None` になる。`Option<Justify>` に `From<&str>` を実装することは orphan rule で不可能なので、Option ではなく `Normal` variant で「未指定」を表す。

却下した代替案: egui_flex。egui 0.35 止まりで、`justify-content` 未実装、`flex-shrink` は構造的に不可能と README 自身が述べている。一級市民には足りない。

## 7. クレート構成

```
react-egui/            core: View, Cx, Store, State, Handle, Dispatch, hooks, sweep, 遅延キュー, 永続化
react-egui-macros/     rsx! (rstml 0.13 ベース), #[component], #[hook]
react-egui-elements/   egui ウィジェット / コンテナのラッパー。View / Text は taffy 上に
react-egui-app/        run(Options, |_cx| rsx!{ <App/> })。eframe を包み native / wasm / Android を吸収。iOS ランナーもここ
examples/              counter, todo (use_reducer + use_persisted), layout, fetch (use_future + Suspense + ehttp), 後に mobile
```

`react_egui_app::run(Options, root)` が 1 フレームでやることは以下。

1. `CentralPanel` で包む(eframe が渡すルート `Ui` には余白も背景も無く、ライトモードで文字が読めないため)。
2. `store.begin_pass(ctx)`。
3. ルートの `Cx` を作り、`root(cx)` が返した `View` を `cx.root_container(..)`(`direction: column`、`w` は `100%`、`min_h` は `100%`、`reserve_available_space`)の中で `show` する。ネストしたコンテナは幅だけを確保するので、ルートだけが高さも取る。この id とスタイルは `react_egui_app::root_id()` / `root_style()` として公開し、テストや自作ランナーが同じ枠を再現できるようにする。
4. `store.end_pass()`。

`Options` は `title` / `max_passes`(既定 3、`ctx.options_mut` で明示設定。5.3 参照)/ `persist` / `canvas_id`(wasm)/ `native`(native のみ)/ `setup` を持つ。`setup: Option<Box<dyn FnOnce(&eframe::CreationContext)>>` は eframe が窓と描画バックエンドを用意した直後に 1 回だけ呼ぶ穴で、`ReactApp::new` の先頭で実行する。wgpu の pipeline を作って `cc.wgpu_render_state` の `renderer.write().callback_resources` に置く場所である(egui 公式 demo の `custom3d_wgpu` と同じ形)。hook や context 経由で `RenderState` を配る案は採らない。wgpu の型は hook API のどこにも出さず、wgpu を使わないアプリは一生見ない、という線を引くためである。native と wasm の両方にある。`App::save` が `store.save_persisted()` を `Storage` の `"react_egui"` キーに書き、`CreationContext::storage` から `load_persisted` する。wasm では `cfg(target_arch = "wasm32")` で `WebRunner` を `wasm_bindgen_futures::spawn_local` に載せ、canvas は `canvas_id` で引く。

`root` は毎パス呼ばれ、返す `View` は `root` の中で作ったものを借用できない(hook の guard を借りた `rsx!` はローカルを借用した値を返すことになる)。hooks はコンポーネントに置き、ルートは `|_cx| rsx!{ <App/> }` の形にする。

egui は 0.36 系に固定する。egui 0.35 以降 `eframe::App::ui` が `&mut Ui` を受け取るので、ランナーはそれをそのまま `Cx` に包む。`react-egui`(core)は `egui_taffy` を通常依存に持つ。`Cx` の `Surface` が `Tui` を知る必要があるためで、wasm ターゲットでもそのままビルドできる。加えて future の実行機構として、native では `pollster`(`cfg(not(target_arch = "wasm32"))`)、wasm では `wasm-bindgen-futures`(`cfg(target_arch = "wasm32")`)を持つ。

## 8. プラットフォーム

- native / wasm / Android: eframe。wasm は trunk でビルドする。描画バックエンドは wgpu。**eframe 0.36 の既定 feature には `wgpu` が入っていて `glow` は入っていない**(0.35 までとは逆)ので、何もしなくても wgpu で描かれる。glow の方が opt-in になったため、選択のための feature は置かない。web は WebGPU 非対応のブラウザのために WebGL へ落ちる。`eframe/wgpu` → `egui-wgpu/default` → `wgpu/webgl` と伝播するので、こちらで `wgpu` を直接依存に取る必要は無い。
- iOS: eframe は未対応(emilk/egui#3117 が open)。`egui-winit` + `egui-wgpu` の薄いランナーを `react-egui-app` 内に書く。ビルドは cargo-mobile2。
- アクセシビリティ: native は eframe(egui-winit)が `Context::enable_accesskit` を呼び、ウィジェットツリーが OS のアクセシビリティ API に届く。**web では届かない。** eframe の web ランナーは毎フレームの `TreeUpdate` を `accesskit_update: _, // not currently implemented`(`eframe/src/web/app_runner.rs`)と捨てており、canvas の中身を DOM に映す adapter も上流に無い。方針と試作は `docs/tasks/a11y/`。
- ライブラリ本体は `&mut egui::Ui` しか触らないので、プラットフォーム対応はランナー層とタッチ / IME の調整に閉じる。
- 非同期の実行機構は core の `task::spawn` に閉じる(4 章「`use_future` の詳細」)。iOS / Android は native と同じスレッド経路を使う。

## 9. テスト

- `egui_kittest` でコンポーネントの操作テストとスナップショットテスト。
- `trybuild` でマクロのコンパイルエラー(イベント名の誤り、`key` 忘れ等)の文面を固定する。
- examples も lib なので kittest を持つ。生 egui 版がある example は、同じ操作を両方に対して流して同じ結果になることを確かめる。
- 見た目の一致は `examples/gallery` の `snapshot` feature で撮る。react-egui 版と生 egui 版を同じ大きさで描き、**同じ画像 1 枚**と比べる。残る差は文字の縁だけなので `SnapshotOptions::max_failed_pixels` で吸収する(値は tasks/examples/plan.md 7 章)。
- feature の裏にある snapshot テストは CI で回らないので、その feature が触る変更をしたら手で撮り直す。
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
| future の executor | スレッド 1 本 + `pollster` | tokio を必須にする | 依存が小さく、待つだけの future に十分。tokio が要るアプリは future の中で `Handle::current()` を使えばよい |
| 非同期の結果の表現 | `std::task::Poll<T>` | 独自の `Loading` / `Ready` / `Error` enum | std にあり、エラーは `T = Result<..>` で表せる。状態の種類を増やさない |
| Suspense の実現 | オフスクリーン描画 + カウンタ + `request_discard` | panic / `catch_unwind` による巻き戻し | Rust に安価な巻き戻しが無い。子は let-else 1 行で抜けられる |
