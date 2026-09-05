# プラン: core

タスク定義は [task.md](task.md)。設計の根拠は [docs/ARCHITECTURE.md](../../ARCHITECTURE.md)。本書は実装手順と確認方法を定める。実装中にここから外れる判断をした場合は本書を更新し、設計上の意味があれば ARCHITECTURE.md も更新する。

## 0. 全体

4 フェーズを順に進める。各フェーズは「実装 → テスト → ARCHITECTURE.md 反映 → コミット」で閉じ、次のフェーズは前のフェーズの API の上に乗る。

| フェーズ | 主な対象 | 触るクレート |
|---|---|---|
| 2 | `View`、hooks の残り、遅延キュー、`Cx` のレイアウトコンテキスト、衝突オーバーレイ | `react-egui` |
| 3 | `#[component]` / `#[hook]` / `rsx!`、trybuild、spike のマクロ版置換 | `react-egui-macros`、`react-egui`(re-export と tests) |
| 4 | `View` / `Text`、ウィジェット、コンテナ、スナップショット | `react-egui-elements` |
| 5 | `run`、`use_persisted`、wasm、examples、CI | `react-egui-app`、`examples/*`、`react-egui`(persist) |

追加する依存(`[workspace.dependencies]` に pin する)。

| crate | 用途 | 場所 |
|---|---|---|
| egui_taffy 0.14(dev → 通常依存に昇格) | `Cx` の `Tui` モード | react-egui |
| serde / serde_json | `use_persisted` | react-egui |
| typed-builder | Props の builder | react-egui(`__private` で再エクスポート) |
| syn 2(`full`, `extra-traits`)/ quote / proc-macro2 | マクロ | react-egui-macros |
| trybuild | コンパイルエラーの固定 | react-egui(dev) |
| egui_kittest `snapshot` + `wgpu` | スナップショット | react-egui-elements(feature `snapshot` の dev) |
| eframe `persistence` feature | `Storage` | react-egui-app |
| wasm-bindgen-futures / web-sys(`Document`, `HtmlCanvasElement`) | wasm ランナー | react-egui-app(`cfg(target_arch = "wasm32")`) |

`react-egui` は `react-egui-macros` を通常依存に持ち、`rsx!` / `component` / `hook` を re-export する。ユーザーは `react_egui::prelude::*` だけを `use` する。マクロの trybuild テストは `react-egui` 側の `tests/ui/` に置く(proc-macro クレートから facade への dev-dependency 循環を避ける)。

## 1. フェーズ 2: core hooks(`crates/react-egui/src/`)

### 1.1 `view.rs`

```rust
pub trait View {
    fn show(self, cx: &mut Cx<'_, '_>);
}
impl View for ()                                   // 何もしない
impl View for &str / String                        // cx.leaf(default, |ui| ui.label(..))
impl<V: View> View for Option<V>
impl<V: View> View for Vec<V>
impl<V: View, const N: usize> View for [V; N]
impl<F: FnOnce(&mut Cx<'_, '_>)> View for F        // escape hatch

/// `rsx!` の展開先。閉包の型推論を効かせるための補助関数
pub fn view<F: FnOnce(&mut Cx<'_, '_>)>(f: F) -> impl View { f }
```

ARCHITECTURE.md 3.2 の `IntoIterator<Item = V>` への blanket impl は、`FnOnce` の blanket impl および `Option<V>` と coherence で衝突する(rustc で確認済み)。`Option` / `Vec` / 配列の個別 impl に置き換える。`rsx!` 内の繰り返しは `for` で書けるので実用上の差はない。ARCHITECTURE.md 3.2 を更新する。

`{ |cx| .. }` を直接 `impl View` の引数に渡すと閉包の引数型が推論されないことがあるので、`rsx!` は必ず `::react_egui::view(|cx| { .. })` を emit する。ユーザーの escape hatch も `view(|cx| ..)` を案内する。

### 1.2 `use_memo`

```rust
#[track_caller]
pub fn use_memo<'s, D: Hash, T: 'static>(cx: &mut Cx<'s, '_>, deps: D, f: impl FnOnce() -> T) -> &'s T;
```

`&'s T` を返すために `RefCell` の外に値を置く。`Slot` に `memo: elsa::FrozenVec<Box<dyn Any>>` を足し、deps のハッシュが変わったら `push_get` で新しい値を積んで `&'s T` を返す。古い値はパス内に配られた `&'s T` が指しているかもしれないので消さず、`end_pass(&mut self)` で最後の 1 つ以外を落とす(`as_mut()` で `&mut Vec` を取る)。deps のハッシュは `deps_hash` を `use_effect` と共用する。初回は必ず計算する。

### 1.3 `use_reducer` と `Dispatch`(`dispatch.rs`)

```rust
#[track_caller]
pub fn use_reducer<'s, S: 'static, M: Send + 'static>(
    cx: &mut Cx<'s, '_>,
    reducer: impl FnMut(&mut S, M),
    init: impl FnOnce() -> S,
) -> (State<'s, S>, Dispatch<M>);

pub struct Dispatch<M> { queue: Arc<Mutex<Vec<M>>>, ctx: egui::Context }   // Clone。M: Send なら Send + Sync
impl<M> Dispatch<M> { pub fn send(&self, msg: M); }   // push して request_repaint
```

スロットの値は `(S, Arc<Mutex<Vec<M>>>)`。hook を訪問するたびにキューを `take` して reducer を順に適用し、その後で `State` guard と `Dispatch` を返す。

ARCHITECTURE.md 5.5 は「パス末に reducer を適用」としているが、訪問時に変更する。理由は 2 つ。(a) パス末に適用するには reducer を保存する必要があり `'static` になるが、訪問時なら reducer は通常の閉包でよい。(b) 別スレッドから届いたメッセージはパス末適用だと「次のパスの本体が古い状態を見て、そのパス末で適用され、さらに次のフレームで表示」となり 1 フレーム余計に遅れる。訪問時適用なら `send` の `request_repaint` で来る次のフレームの本体が新しい状態を見る。ハンドラから `send` した場合の見え方(次フレームで反映)は `State` への書き込みと同じで変わらない。ARCHITECTURE.md 4 と 5.5 を更新する。

### 1.4 遅延キュー: `defer` と `update_later`

```rust
// Store
deferred: RefCell<Vec<Box<dyn FnOnce(&Store)>>>,
pub(crate) fn defer_raw(&self, f: Box<dyn FnOnce(&Store)>);

// Cx
pub fn defer(&self, f: impl FnOnce() + 'static);

// State<'s, T> と Handle<'s, T> の両方
pub fn update_later(&self, f: impl FnOnce(&mut T) + 'static);
```

`update_later` はスロット Id を捕まえた閉包を積む。適用時に `slot_by_id` で引き、`borrow_mut` して `f` を呼び、`request_repaint` する。スロットが無ければ(そのパスで unmount された)黙って捨てる。`State::update_later` は guard が生きていても呼べる(適用はパス末で、guard はとっくに落ちている)。

`end_pass` の順序: 遅延キューを空になるまで適用 → sweep → 衝突オーバーレイ(1.6)。キューの適用中に新しい項目が積まれることは公開 API では起きない(`FnOnce()` は `Store` に触れない)。

閉包が `'static` なので、ループ変数を使う場合は `todos.update_later(move |t| t.remove(i))` と `move` が要る。ARCHITECTURE.md 3.7 と 4 の表に追記する。

### 1.5 repaint

`Dispatch::send` と `update_later` の適用は無条件に `request_repaint` する。`defer` はしない(状態に触れないため)。`repaint.rs` のテストにケースを足す。

### 1.6 衝突オーバーレイ

`Store` に `warn_on_collision: bool`(既定 `cfg!(debug_assertions)`)と `set_warn_on_collision` を足す。`end_pass` の最後で、有効かつ `collisions` が空でなければ `egui::Area::new(Id::new("react_egui_collision_warning")).order(Order::Debug).anchor(Align2::LEFT_TOP, (8.0, 8.0))` に `Frame::popup` で赤い文字を出す。文面は `react-egui: hook id collision at {file}:{line}:{column}. Wrap custom hooks in #[hook], or add key= inside loops.` とし、同じ位置は 1 パスに 1 行にまとめる。`end_pass` はランナーの `App::ui` の中で呼ばれるので、そのフレームの `Context` に描ける。

### 1.7 `Cx` のレイアウトコンテキスト

```rust
pub struct Cx<'s, 'u> {
    pub store: &'s Store,
    surface: Surface<'u>,
    scope: egui::Id,
}
enum Surface<'u> {
    Ui(&'u mut egui::Ui),
    Taffy(&'u mut egui_taffy::Tui),
}
impl<'s, 'u> Cx<'s, 'u> {
    pub fn new(store: &'s Store, ui: &'u mut egui::Ui, scope: Id) -> Self;
    pub fn new_taffy(store: &'s Store, tui: &'u mut egui_taffy::Tui, scope: Id) -> Self;
    pub fn ui(&mut self) -> &mut egui::Ui;          // Ui: そのまま。Taffy: tui.egui_ui_mut()(taffy の配置を受けない。elements は使わない)
    pub fn in_taffy(&self) -> bool;
    pub fn ctx(&self) -> &egui::Context;
    pub fn scope_id(&self) -> Id;
    pub fn scope<R>(&mut self, source: impl Hash + Debug, f: impl FnOnce(&mut Cx<'s, '_>) -> R) -> R;   // Ui: ui.push_id。Taffy: tui.with_auto_id_prefix(id, ..)
    pub fn hook_scope<R>(..) -> R;                                                                     // 変更なし
    pub fn defer(&self, f: impl FnOnce() + 'static);
    /// leaf: egui ウィジェットを 1 つ描く。Taffy の中なら tui.style(style.to_taffy()).ui(f)、外なら f(ui)
    pub fn leaf<R>(&mut self, style: &ItemStyle, f: impl FnOnce(&mut egui::Ui) -> R) -> R;
    /// container: taffy ノードを作り、その中を Taffy モードの Cx で描く。外なら egui_taffy::tui(ui, id).style(..).show、中なら tui.style(..).add
    pub fn container<R>(&mut self, id: Id, style: taffy::Style, f: impl FnOnce(&mut Cx<'s, '_>) -> R) -> R;
}
```

`cx.ui` フィールドは `cx.ui()` メソッドになる。spike のテストと examples はこの時点で書き換える(フェーズ 3 でマクロ版に置き換わるまでの一時対応)。egui のコンテナ閉包の内側で `Cx` を作り直す手順は変わらない(`let (store, scope) = (cx.store, cx.scope_id()); cx.ui().vertical(|ui| { let mut cx = Cx::new(store, ui, scope); .. })`)。`container` の Ui モード側は `reserve_available_width()` を既定とし、ランナーのルートだけ `reserve_available_space()` を使う(4.1)。ARCHITECTURE.md 3.1 と 6 を更新する。

### 1.8 `layout.rs`

```rust
pub enum Length { Px(f32), Percent(f32), Auto }      // From<f32> / From<i32> は Px、From<&str> は "50%" / "auto" / "12px" / "12" をパース(不正な文字列は panic)
#[derive(Default, Clone)]
pub struct ItemStyle { w, h, min_w, min_h, max_w, max_h: Option<Length>, grow, shrink: Option<f32>, basis: Option<Length>, align_self: Option<AlignSelf>, m, mx, my, mt, mr, mb, ml, p, px, py, pt, pr, pb, pl: Option<Length> }
impl ItemStyle { pub fn w(self, v: impl Into<Length>) -> Self; .. /* 各フィールドの setter */; pub fn to_taffy(&self) -> taffy::Style; }
#[derive(Default, Clone)]
pub struct ContainerStyle { display: Display(Flex | Grid | Block | None), direction: Direction, wrap: bool, justify: Justify, align: Align, align_content: Option<Align>, gap: (f32, f32), cols: Option<u16> /* grid の等幅カラム数 */ }
impl ContainerStyle { pub fn merge(&self, item: &ItemStyle) -> taffy::Style; }
```

`Direction` / `Justify` / `Align` / `AlignSelf` / `Display` は enum で、`From<&str>` を実装する(`"row"`, `"space-between"`, `"center"` など。不正な文字列は位置付きで panic)。`rsx!` が文字列リテラルをそのまま setter に渡せるようにするため。`m` / `p` の短縮形は `mx` → `ml` + `mr` の順で個別指定が優先する。taffy 側の型は `egui_taffy::taffy` を re-export して使う。

### 1.9 テスト(フェーズ 2)

| # | テストファイル | 確認内容 |
|---|---|---|
| 2-1 | `view.rs` | `()` / `&str` / `String` / `Option` / `Vec` / 配列 / 閉包のそれぞれが描画される。`view(|cx| ..)` で `cx` の型注釈なしにコンパイルが通る |
| 2-2 | `memo.rs` | deps が同じフレームでは `f` が再実行されず、変わると再実行される。同一フレームで同じ `&T` を 2 か所で読める。unmount で破棄される |
| 2-3 | `reducer.rs` | ハンドラから `send` した次フレームで state が変わる。`Dispatch` を `std::thread::spawn` に渡して `send` し、`ctx.has_requested_repaint()` が true、次フレームで反映される。2 パス目で二重適用されない(`multi_pass` の手順を流用) |
| 2-4 | `deferred.rs` | `for` で `todos` を読みながらループ内のハンドラで `update_later(move |t| t.remove(i))` を呼び、次フレームで要素が減る。同じフレーム内で後続のウィジェットは古い値を見る(5.7)。`defer` がパス末に 1 回だけ走る。unmount 済みスロットへの `update_later` が panic しない |
| 2-5 | `repaint.rs`(追記) | `send` / `update_later` のフレームだけ repaint が要求される |
| 2-6 | `collision.rs`(追記) | 衝突があるフレームで `get_by_label` にオーバーレイの文言が見える。`set_warn_on_collision(false)` で消える |
| 2-7 | `taffy_cx.rs` | `cx.container` の中で `cx.leaf` を 3 つ描き、`direction="row"` で x 座標が単調増加、`"column"` で y 座標が単調増加(kittest の `Node::rect()` で確認)。`container` の入れ子で内側の hooks が動く。`cx.scope` が Taffy モードでも hooks の Id と egui の Id を分ける(同じ関数を 2 回 `scope` で包んで描き、state が独立し、`Collapsing` のような egui 側の状態も独立する) |

kittest の `Harness::new_ui_state` と `run_app` の形は spike と同じ。`tests/common/mod.rs` の `run_app` は `Cx::new` を使い続ける。

## 2. フェーズ 3: マクロ(`crates/react-egui-macros/src/`)

### 2.1 `#[component]`(`component.rs`)

入力は `fn Name<generics>(cx: &mut Cx, <props>..)`。戻り値は `()` のみ(それ以外はエラー)。第 1 引数が `&mut Cx` でなければエラー。

引数の扱い。

| 形 | 生成 |
|---|---|
| `x: T` | 必須 prop。`&T` の elided lifetime は `'e` に書き換える(`Type::Reference` で `lifetime: None` のもの)。`Cow<str>` のような path 内の省略はユーザーが明示する |
| `x: Option<T>` | 省略可能 prop(`#[builder(default)]`) |
| `#[prop(default)] x: T` / `#[prop(default = expr)] x: T` | 省略可能 prop |
| `#[prop(into)] x: T` | `#[builder(setter(into))]` |
| `children: C` where `C: View`、または `children: impl View` | `impl` は generic `C: View` に脱糖する。`rsx!` は子が無くても `.children(())` を渡すので(2.3)、`()` を受けられる型なら省略可能になる |
| `children` を宣言しない | `children: ()` を `#[builder(default)]` で生成する。`rsx!` が常に `.children(..)` を呼ぶため |
| `#[event] on_x: A` | イベント enum に `X(A)` を足す。本体では `on_x: Emitter<'_, '_, NameEvent, A>` |

生成物。

```rust
pub enum NameEvent { X(A), .. }                                  // #[event] が 1 つ以上ある場合のみ
#[derive(::react_egui::__private::TypedBuilder)]
#[builder(crate_module_path = ::react_egui::__private::typed_builder)]
pub struct NameProps<'e, C: View, ..generics> {
    pub x: T,
    #[builder(default)] pub y: Option<U>,
    #[builder(default)] pub events: Option<&'e mut dyn FnMut(NameEvent)>,   // #[event] がある場合のみ
    pub children: C,
}
#[allow(non_snake_case)]
pub fn Name<'e, C: View, ..>(cx: &mut Cx<'_, '_>, props: NameProps<'e, C, ..>) {
    let NameProps { x, y, events, children } = props;
    let mut __noop = |_: NameEvent| {};
    let __events: &mut dyn FnMut(NameEvent) = match events { Some(e) => e, None => &mut __noop };   // match の腕で lifetime を縮める
    let __sink = ::react_egui::EventSink::new(__events);
    let on_x = ::react_egui::Emitter::new(&__sink, NameEvent::X);
    { /* 本体。末尾式 tail は ::react_egui::View::show(tail, cx) に書き換える */ }
}
impl<'e, C: View, ..> ::react_egui::__private::Props for NameProps<'e, C, ..> {   // 2.3 の props_builder 用
    type Builder = NamePropsBuilder<'e, C, ..>;
    fn builder() -> Self::Builder { Self::builder() }
}
```

本体の末尾式の書き換えは、本体を `syn::Block` として見て最後の `Stmt::Expr(expr, None)` を差し替える。末尾が `;` で終わる本体はそのまま。`if` / `match` の各腕で別々の `rsx!` を返す本体は閉包の型が一致せずコンパイルできないので、`rsx!{ if .. }` の形を案内する(trybuild で文面を固定)。`{ body }` を `View::show({ body }, cx)` で包む形はブロック内のローカルを guard が借用したまま返すことになり通らない。必ず末尾式だけを差し替える。

`Emitter` は spike の `Emitter<'a, 'e, E>` にペイロード型 `A` と variant コンストラクタ `fn(A) -> E` を足す。`on_x.emit(a)` は `sink(NameEvent::X(a))` になる。ARCHITECTURE.md 3.6 を更新する。

### 2.2 `#[hook]`(`hook.rs`)

`#[track_caller]` を付け、本体を `cx.hook_scope(::std::panic::Location::caller(), |cx| { body })` で包む。`cx` は「`&mut Cx` 型の最初の引数」で、無ければエラー。戻り値と generics はそのまま。本体内の `return` は閉包から返ることになるが、閉包の戻り値がそのまま hook の戻り値になるので意味は変わらない。

### 2.3 `rsx!`(`rsx/`)

#### パース

`rstml::ParserConfig::new().custom_node::<ControlFlow>()` で `Vec<Node<ControlFlow>>` にパースする。`ControlFlow` は `if` / `for` / `match` の 3 種で、`peek_element` で先頭の ident を見る。本体(`{ .. }`)は `RecoverableContext::parse_recoverable::<Node<ControlFlow>>` を `}` まで繰り返して子ノードにする。

- `if cond { nodes } else if cond { nodes } else { nodes }`: `cond` は `syn::Expr::parse_without_eager_brace`。
- `for pat in expr { nodes }`: `expr` も同様。
- `match expr { pat (if guard)? => { nodes } | <Elem/> , .. }`: 腕の右辺は 1 つの要素か `{ nodes }`。

ノードの種類と扱い。

| ノード | 扱い |
|---|---|
| `<Name attrs>children</Name>` / `<Name attrs/>` | コンポーネント呼び出し。`Name` は Rust のパス(`elements::Button` も可) |
| `"literal"` | 文字列リテラル。`View` として `show` する |
| 引用符なしのテキスト | エラー: `text must be a string literal: "..."` |
| `{expr}` | `::react_egui::View::show(expr, cx);` |
| `<> .. </>` | 子を順に展開 |
| `<!-- -->` | 無視 |
| `if` / `for` / `match` | Rust の制御構文をそのまま emit し、本体を展開する |

#### 属性

| 形 | 扱い |
|---|---|
| `key={expr}` | scope の Id に混ぜる。要素ごとに最大 1 つ |
| `on_x={expr}` | 融合閉包の腕 `NameEvent::X(a) => ::react_egui::Handler::call(expr, a)` |
| `events={expr}` | 融合閉包の代わりに `expr` を `events` に渡す。`on_*` と併用はエラー |
| レイアウト属性(`w h min_w min_h max_w max_h grow shrink basis align_self m mx my mt mr mb ml p px py pt pr pb pl`) | まとめて `.style(::react_egui::layout::ItemStyle::default().w(..).grow(..))` を 1 回呼ぶ。1 つも無ければ呼ばない |
| その他 `name={expr}` / `name="lit"` / `name`(bool の true) | builder の setter `.name(expr)` |

`on_x` → `X` の変換は `on_` を外して PascalCase(`on_ok` → `Ok`、`on_value_change` → `ValueChange`)。属性名の重複はエラー。

#### 展開

`rsx!{ nodes }` 全体は `::react_egui::view(|cx| { stmts })` になる(非 `move`)。要素 1 つは次の文になる。

```rust
cx.scope((line!(), column!(), 3usize, key), |cx| {          // line!/column! は要素の span で emit。3 は rsx! 内の要素の通し番号
    Name(cx, ::react_egui::props_builder(&Name)
        .x(expr)
        .style(::react_egui::layout::ItemStyle::default().grow(1.0))
        .events(&mut |__ev| {
            #[allow(unreachable_patterns)]
            match __ev {
                NameEvent::Ok(a) => ::react_egui::Handler::call(|| *open = false, a),
                NameEvent::Cancel(a) => ::react_egui::Handler::call(|| *open = false, a),
                _ => {}
            }
        })
        .children(::react_egui::view(|cx| { .. }))
        .build());
});
```

- `key` が無ければ `(line!(), column!(), n)`。`line!()` / `column!()` が要素の位置を返さない(マクロ呼び出し位置を返す)場合は、`Span::line()` / `Span::column()`(1.88 で stable)でマクロ側に埋め込む。どちらでも要素ごとに一意になればよい。
- `children`: 常に `.children(..)` を呼ぶ。子が「1 つの文字列リテラル」または「1 つの `{expr}`」ならその式をそのまま渡す(`Button` の `children: impl Into<WidgetText>` と `View` の `children: impl View` の両方に効く)。複数の子か要素の子なら `::react_egui::view(|cx| { .. })`。子が無ければ `()`。`<View/>` は `()` が `View` なので通り、`<Button/>` は `()` が `Into<WidgetText>` でないので落ちる。generic `C` が未指定のまま残らないので、`children: impl View` を `#[builder(default)]` にする必要がない。
- `props_builder`: `pub fn props_builder<P: Props, F: for<'a, 's, 'u> Fn(&'a mut Cx<'s, 'u>, P)>(_: &F) -> P::Builder { P::builder() }`。関数アイテムの型は名指しできないので、`Fn` bound から `P` を推論させる。`Props` trait は `#[component]` が `NameProps` に実装し、関連型で `NamePropsBuilder` に結ぶ。これで `use components::Name;` だけで `<Name/>` が書ける。
- `.style(..)` は `style` prop を持たないコンポーネントに対しては「no method named `style`」で落ちる。レイアウト属性を受けたいユーザーコンポーネントは `style: ItemStyle` を宣言して子に渡す。
- ハンドラは `Handler::call(closure, a)` の形で呼ばれるので、spike の `(|| ..)()` にあった `redundant_closure_call` は出ない。展開結果には `#[allow(clippy::redundant_closure_call)]` を付けない(付ける理由が無くなった。ARCHITECTURE.md 3.2 を更新)。
- 融合閉包は `#[event]` の無いコンポーネントに `on_*` を渡した場合、`.events(..)` メソッドが無いことでコンパイルエラーになる。variant 名の誤りは `NameEvent::Foo` が無いことで落ちる。どちらも trybuild で文面を固定する。

### 2.4 trybuild(`crates/react-egui/tests/ui/`)

`tests/compile_fail.rs` から `trybuild::TestCases::new().compile_fail("tests/ui/*.rs")` と `pass("tests/ui/pass/*.rs")` を回す。

| ファイル | 内容 |
|---|---|
| `unknown_event.rs` | `<Dialog on_foo={..}/>`: `no variant named Foo` |
| `event_on_plain_component.rs` | `#[event]` の無いコンポーネントに `on_click` |
| `missing_prop.rs` | 必須 prop の省略(typed-builder のエラー) |
| `unquoted_text.rs` | `<Text>hello</Text>` |
| `component_returns_value.rs` | `#[component] fn A(cx: &mut Cx) -> i32` |
| `hook_without_cx.rs` | `#[hook] fn use_x() {}` |
| `handler_borrows_prop.rs` | `<Dialog title={&*title} on_rename={|s| *title = s}/>` の E0502(ARCHITECTURE.md 3.7 の 2 つ目) |
| `loop_handler_mutates.rs` | `for` 内のハンドラで `todos.remove(i)`(3.7 の 1 つ目) |
| `pass/borrow_shapes.rs` | 兄弟ハンドラの `&mut` 共有、`for` + `update_later`、`children` 閉包、`view(|cx| ..)` の推論が通ること |

`.stderr` はコミットする。toolchain は pin されているので文面は安定する。

### 2.5 spike の置き換え

`tests/common/mod.rs` の手書き `counter` / `dialog` / `use_counter` を `#[component]` / `#[hook]` / `rsx!` 版にし、10 本の spike テストを変更せずに通す(`cx.ui()` への追従を除く)。`use_counter_unscoped` は `#[hook]` を付けない関数として残す(衝突テストの対照)。テストが期待するボタンのラベルとログは変えない。

### 2.6 テスト(フェーズ 3)

| # | テストファイル | 確認内容 |
|---|---|---|
| 3-1 | spike の 10 本 | マクロ版で全て緑 |
| 3-2 | `rsx_control_flow.rs` | `if` / `else if` / `else`、`for` + `key`、`match` の各腕、フラグメント、`Option` の `{expr}` |
| 3-3 | `rsx_children.rs` | 単一リテラル / 単一 `{expr}` / 複数子 / 子なし。`children: impl View` を受ける親の中で子の hooks が親のスコープ下で動く |
| 3-4 | `component_props.rs` | `Option` の省略、`#[prop(default = expr)]`、`#[prop(into)]`、`&str` prop、generics 付き props |
| 3-5 | `component_events.rs` | `#[event]` 3 つ(ペイロードなし / 値 / 借用 `&str`)、`events=` escape hatch、`on_*` を 1 つも渡さない場合に `emit` が no-op |
| 3-6 | `rsx_scope.rs` | 同じ `rsx!` 内の 2 つの `<Counter/>` が独立、`for` 内で `key` あり / なし(なしは `collisions()` が非空)、要素を `if` で消すと unmount される |
| 3-7 | `compile_fail.rs` | trybuild |

## 3. フェーズ 4: elements(`crates/react-egui-elements/src/`)

全要素は `#[component]` で書き、`style: ItemStyle` を `#[prop(default)]` で受ける。ウィジェットは `cx.leaf(&style, |ui| ..)` の中で egui を呼ぶ。コンテナ(egui-native のもの)は Taffy モードでは leaf として振る舞い、その中の子は Ui モードで描く。

### 3.1 `View` と `Text`

```rust
#[component]
pub fn View(cx: &mut Cx, #[prop(default)] style: ItemStyle, #[prop(default)] display: Display, #[prop(default)] direction: Direction, #[prop(default)] wrap: bool, #[prop(default)] justify: Justify, #[prop(default)] align: Align, align_content: Option<Align>, #[prop(default)] gap: Gap, cols: Option<u16>, children: impl View)
```

`ContainerStyle::merge(&style)` で `taffy::Style` を作り、`cx.container(id, style, |cx| children.show(cx))`。`id` は `cx.scope_id()`。`gap: Gap` は `From<f32>` と `From<(f32, f32)>`。`display="grid"` のときは `cols` を `grid_template_columns: vec![fr(1.0); cols]` にする。`col_span` / `row_span` は `ItemStyle` に足す。

```rust
#[component]
pub fn Text(cx: &mut Cx, #[prop(default)] style: ItemStyle, size: Option<f32>, color: Option<egui::Color32>, #[prop(default)] strong: bool, #[prop(default)] wrap: bool, children: impl Into<egui::WidgetText>)
```

`egui::Label::new(rich).wrap_mode(if wrap { Wrap } else { Extend })` を leaf に置く。既定 `Extend` は ARCHITECTURE.md 6 のとおり。

### 3.2 ウィジェット

| 要素 | props | events | 実装 |
|---|---|---|---|
| `Button` | `children: impl Into<WidgetText>`, `enabled: bool = true` | `on_click: ()` | `ui.add_enabled(enabled, egui::Button::new(..))` |
| `Label` | `children: impl Into<WidgetText>` | | `ui.label`(egui 既定の wrap。`Text` との違いは wrap 既定のみ) |
| `TextEdit` | `bind: &mut String`, `multiline: bool = false`, `hint: Option<&str>`, `desired_width: Option<f32>` | `on_change: ()`, `on_submit: ()` | `changed()` で `on_change`、`lost_focus && Enter` で `on_submit` |
| `Checkbox` | `bind: &mut bool`, `label: Option<&str>` | `on_change: bool` | |
| `Slider<T: Numeric>` | `bind: &mut T`, `range: RangeInclusive<T>`, `label: Option<&str>` | `on_change: ()` | |
| `ComboBox` | `bind: &mut usize`, `options: &[impl AsRef<str>]`, `label: Option<&str>` | `on_change: usize` | `egui::ComboBox::from_id_salt(cx.scope_id())` |
| `Image` | `source: egui::ImageSource`, `fit: Option<egui::Vec2>` | | `egui::Image::new(source)`。ローダーはアプリ側 |
| `Separator` | `vertical: bool = false` | | |

`bind` を持つ要素と、同じ state を触るハンドラを同じ要素に渡すと E0502 になる(2.4 の `handler_borrows_prop`)。`bind` 要素の `on_change` は state を触らない用途(ログ、`Dispatch`)に限られることをドキュメントに書く。

### 3.3 コンテナ

| 要素 | props | 実装 |
|---|---|---|
| `ScrollArea` | `horizontal`, `vertical = true`, `max_h: Option<f32>`, `children: impl View` | `egui::ScrollArea::..show(ui, ..)` の中で `Cx::new(store, ui, scope)` |
| `Collapsing` | `header: &str`, `default_open: bool`, `children` | `CollapsingHeader::new(header).id_salt(cx.scope_id())` |
| `Frame` | `fill: Option<Color32>`, `stroke: Option<Stroke>`, `inner_margin: Option<f32>`, `corner_radius: Option<f32>`, `children` | `egui::Frame::new()..show(ui, ..)` |
| `Window` | `title: &str`, `open: Option<&mut bool>`, `resizable`, `children` | `egui::Window::new(title).id(cx.scope_id()).show(cx.ctx(), ..)` |
| `SidePanel` / `TopBottomPanel` / `CentralPanel` | `side: Side` / `TopBottom`, `default_size: Option<f32>`, `resizable`, `children` | `show_inside(ui, ..)` |
| `Vertical` / `Horizontal` | `children` | `ui.vertical` / `ui.horizontal` |
| `Grid` | `cols: usize`, `striped: bool`, `children` | `egui::Grid::new(cx.scope_id()).show`。`children` は `Row` 要素で区切る(`Row` は `ui.end_row()` を呼ぶだけの要素) |

すべて `children` の描画前に `Cx::new(store, ui, scope)` を作り直す(spike の nested_ui と同じ形)。`Window` と各 `Panel` は `cx.container` の中(Taffy モード)からも呼べる。その場合は leaf として扱わず、`cx.ctx()` / `cx.ui()` に直接描く(`Window` はコンテキストに浮くので配置を受けない)。

### 3.4 テスト(フェーズ 4)

| # | テストファイル | 確認内容 |
|---|---|---|
| 4-1 | `widgets.rs` | 各ウィジェットを kittest で操作(`click` / `type_text` / `key_press(Enter)`)し、`bind` と events の両方が期待どおり動く。`ComboBox` は `click` → 選択肢 `click` |
| 4-2 | `containers.rs` | 各コンテナの子に `use_state` を置き、フレームを跨いで保持される。`Window` の `open` を false にすると子が unmount される。`Grid` の `Row` で行が変わる(`rect()` の y) |
| 4-3 | `layout.rs` | `View` の `direction` / `justify` / `align` / `gap` / `grow` / `w` / `p` / `m` を `rect()` で検証。`display="grid" cols={2}` で 2 列に並ぶ。入れ子の `View` の中の `Text` が親の幅で折り返さない(`Extend`) |
| 4-4 | `snapshots.rs`(feature `snapshot`) | `layout` example の各セクションと `Text` の wrap を `harness.snapshot("..")`。`SnapshotOptions::threshold` は OS ごとの既定を使う |
| 4-5 | `multi_pass.rs`(core 側、置き換え) | spike の taffy 版テストを `<View>` + `<Button>` + `<Text>` で書き直し、2 パス目でハンドラが 1 回だけ発火することを引き続き確認する |

スナップショットの CI は 4.5 参照。画像は `crates/react-egui-elements/tests/snapshots/` にコミットする。

## 4. フェーズ 5: ランナーと examples

### 4.1 `react-egui-app::run`

```rust
pub struct Options {
    pub title: String,
    pub max_passes: usize,               // 既定 2
    pub persist: bool,                   // 既定 true。eframe の Storage を使う
    pub canvas_id: String,               // wasm。既定 "react_egui_canvas"
    pub native: eframe::NativeOptions,
}   // impl Default
pub fn run<V: View>(options: Options, root: impl FnMut(&mut Cx<'_, '_>) -> V + 'static) -> eframe::Result;
```

- native: `eframe::run_native`。`App::ui` で `CentralPanel::default().show(ui, ..)` の中で `store.begin_pass` → `cx.container(root_id, column + reserve_available_space, |cx| root(cx).show(cx))` → `store.end_pass`。`App::save` で `store.save_persisted()` を `storage.set_string("react_egui", ..)` に書く。`CreationContext` の `storage` から `load_persisted` する。
- wasm: `cfg(target_arch = "wasm32")` で `wasm_bindgen_futures::spawn_local(eframe::WebRunner::new().start(canvas, WebOptions::default(), Box::new(..)))`。canvas は `web_sys::window().document().get_element_by_id(canvas_id)`。`run` の戻り値は `Ok(())`。
- `root` は毎フレーム呼ばれる。`root` の中で hooks を使い、その state を借りる `rsx!` を返すと「ローカルを借用した値を返せない」エラーになる。ルートは `|cx| rsx!{ <App/> }` の形にして hooks はコンポーネントに置くことを doc comment と README に書く。`#[component]` の本体末尾は 2.1 の書き換えで同じ問題を回避している。
- `Options::max_passes` を `ctx.options_mut` で設定する。

### 4.2 `use_persisted`(core、`hooks.rs`)

```rust
#[track_caller]
pub fn use_persisted<'s, T: Serialize + DeserializeOwned + 'static>(cx: &mut Cx<'s, '_>, key: &str, init: impl FnOnce() -> T) -> State<'s, T>;
```

- Id は `Id::new(("react_egui_persisted", key))`。スコープには依存しない。
- `Store` に `persisted: RefCell<HashMap<String, String>>`(キー → JSON 文字列)を持つ。`load_persisted(&mut self, json: &str)` で一括読み込み、`save_persisted(&self) -> String` で生きているスロットを直列化して map に上書きしてから全体を JSON にする。
- `Slot` に `persist: Option<(String, fn(&dyn Any) -> Option<String>)>` を足す。初回訪問時は map にキーがあれば deserialize、失敗か無しなら `init`。
- sweep で persist 付きスロットを落とすときは、先に直列化して map に書く(unmount した状態も次回起動で残る)。
- ランナーの `App::save` は eframe が定期的(`auto_save_interval`)と終了時に呼ぶ。

### 4.3 examples

各 example は `src/main.rs` 1 つと `index.html` / `Trunk.toml`。`main` は native / wasm 共通で `react_egui_app::run(Options { title, ..Default::default() }, |cx| rsx!{ <App/> })`。

| example | 内容 |
|---|---|
| `counter` | `use_state` + `View` / `Text` / `Button`。README の例と同じコード |
| `todo` | `use_reducer` で `Vec<Todo>`(Add / Toggle / Remove / Clear)。`TextEdit bind` + `on_submit` で追加、`for` + `key` で一覧、`Checkbox` で toggle、削除は `Dispatch`。`use_persisted("todos", ..)` で保存。`Collapsing` に完了済みをまとめる |
| `layout` | `View` の flex 属性を並べたデモ(row / column / justify 各種 / align / grow / gap / 入れ子 / grid)。`ScrollArea` の中に置く。4-4 のスナップショットと同じ構成 |

`examples/spike` は削除する。`Cargo.toml` の `members` は `examples/*` のままでよい。

### 4.4 README

「使い方」節に counter の全コードと `cargo run -p counter`、`trunk serve examples/counter/index.html` を書く。hooks / elements の一覧表は ARCHITECTURE.md へのリンクで済ませる(英訳と整備はフェーズ 8)。

### 4.5 CI

`ci.yml` に足すステップ。

1. `cargo check --workspace --target wasm32-unknown-unknown`(`-p react-egui` から `--workspace` に広げる。examples も wasm でコンパイルできること)
2. trunk: `jetli/trunk-action@v0.5` で `trunk` を入れ、`trunk build --release examples/counter/index.html`
3. スナップショット: `sudo apt-get install -y mesa-vulkan-drivers` の上で `WGPU_BACKEND=vulkan cargo test -p react-egui-elements --features snapshot`。2 回試して安定しなければステップを外し、PR 本文に理由を書く

`Swatinem/rust-cache` のキーは既存のまま。

## 5. 手順

1. フェーズ 2: `view.rs` → `layout.rs` → `Cx` の `Surface`(`cx.ui()` への追従で spike のテストと examples を直す)→ `use_memo` → `dispatch.rs` / `use_reducer` → 遅延キュー → オーバーレイ。テスト 2-1 〜 2-7。ARCHITECTURE.md 3.1 / 3.2 / 3.7 / 4 / 5.5 / 6 を更新。コミット。
2. フェーズ 3: `react-egui-macros` の依存を足し、`#[hook]` → `#[component]` → `rsx!`(パース → 属性 → 展開 → 制御構文)の順。`react-egui` に `__private`(typed_builder、`Component` / `Props` trait、`props_builder`)と re-export を足す。`tests/common` をマクロ版に置換して spike のテストを通す。テスト 3-2 〜 3-7。ARCHITECTURE.md 3.2 / 3.3 / 3.6 を更新。コミット。
3. フェーズ 4: `View` / `Text` → ウィジェット → コンテナ。テスト 4-1 〜 4-3、`multi_pass` の置き換え(4-5)。スナップショット(4-4)は feature の裏で書き、ローカルで画像を生成してコミット。ARCHITECTURE.md 6 を更新。コミット。
4. フェーズ 5: `use_persisted`(core)→ `run`(native)→ examples 3 つ → wasm ランナー → `index.html` / `Trunk.toml` → README → CI。`examples/spike` を削除。`cargo run` 3 つと `trunk serve` を目視。コミット。
5. CI が全ステップ緑であることを確認する。スナップショットのステップが不安定なら 4.5 のとおり外す。
6. PR 本文に、フェーズごとの結果、ARCHITECTURE.md の変更点、外したもの(あれば)を書く。

## 6. 判断が必要になりそうな点

- **`props_builder(&Name)` の推論**(2.3)。lifetime `'e` と generic `C` を持つ props で `Fn` bound 経由の推論が通らない場合は、`#[component]` が関数と同名の braced struct(型名前空間は関数と衝突しない)を Props として生成し、`rsx!` は `Name::builder()` を emit する。ユーザーの `use` は同じ 1 つで済む。`NameProps` 名は捨てる。
- **`typed-builder` の `crate_module_path`**。効かない場合は必須フィールドの typestate builder を `#[component]` が自前で生成する(必須フィールド数ぶんの marker generics)。
- **`line!()` / `column!()` の span**(2.3)。要素ごとに一意でなければ `Span::line()` / `Span::column()` をマクロ側で読んで数値リテラルとして埋め込む。
- **`use_memo` の `&'s T`**(1.2)。`FrozenVec` で持ち回るのが煩雑なら `Memo<'s, T>: Deref<Target = T>` の guard を返す形に落とす。その場合 ARCHITECTURE.md 4 の表を更新する。
- **Ui モード直下の `View` のサイズ**(1.7)。`reserve_available_width()` で `Window` の中に置いたときに崩れる場合は `View` に `fill: bool` prop を足して切り替える。
- **`Surface::Taffy` での `cx.ui()`**。`egui_ui_mut()` に描いた結果がテストで明らかに壊れて見えるなら、Taffy モードの `ui()` は `leaf(default)` を自動で挟む代わりに panic ではなく `log::warn!` で 1 回だけ警告する。
- **スナップショットの CI**(4.5)。
- **wasm の `cargo check --workspace`**。`react-egui-app` の wasm 依存で check が通らない場合、`eframe` の `wasm-bindgen` 系 feature を見直す。examples 単体で通るまでは `-p react-egui -p react-egui-elements -p react-egui-app` に絞ってもよい。

## 7. ARCHITECTURE.md に反映する変更(着手時点で判明しているもの)

- 3.1: `Cx` は `ui` フィールドではなく `ui()` メソッド。`Surface`(Ui / Taffy)、`leaf` / `container`、`defer` を持つ。
- 3.2: `View` の impl 一覧を `Option` / `Vec` / 配列 / 閉包に改める。`rsx!` は `view(|cx| ..)` を emit する。`#[allow(clippy::redundant_closure_call)]` は不要になった。
- 3.3: Props は typed-builder の builder で組み立てる。`Option` と `#[prop(default)]` が省略可能。`children` は必須。本体の末尾式は `View::show(tail, cx)` に書き換わる。
- 3.6: `Emitter<'a, 'e, E, A>` はペイロード型と variant コンストラクタを持つ。`events` は `Option<&mut dyn FnMut(E)>` で、省略時は no-op。
- 3.7: `update_later` の閉包は `'static`(`move`)。
- 4: `use_reducer` のメッセージは次の訪問時に適用。`use_persisted` はキーのみで識別し、eframe `Storage` の 1 キーに JSON でまとめる。
- 5.5: 遅延キューの適用は sweep の前、`Dispatch` は含まない。
- 6: レイアウト属性の一覧、`Length` の単位、`View` の props、egui-native コンテナが Taffy モードでは leaf になること。
- 7: `react-egui` が `egui_taffy` に依存する。`run(Options, |cx| rsx!{ <App/> })` の形。

## 8. 実装で判明した差分

### フェーズ 2(手順 1)

- **1.1** `impl View for &str` / `String` / `Option` / `Vec` / 配列と `FnOnce` の blanket impl は coherence で衝突せず、そのまま共存した。`view(|cx| ..)` の型推論も注釈なしで通る(テスト `view::every_view_impl_draws`)。
- **1.2** `elsa::FrozenVec<Box<dyn Any>>` の `push_get` は素直に `&'s dyn Any` を返すので、6 章の代替案(`Memo` guard)には落とさずに `&'s T` を実現できた。`Slot` に `memo_last` / `memo_push` / `prune_memo` を足し、`prune_memo` は sweep の中で生存スロットに対して呼ぶ(`end_pass` の別ループにはしていない)。deps のハッシュ計算は `hooks::deps_hash` として `use_effect` と共用した。
- **1.3** スロットの値は `(S, Arc<Mutex<Vec<M>>>)` のタプルにせず、state 用スロット(素の `S`)とキュー用スロット(`id.with("__react_egui_reducer_queue")`、`Arc<Mutex<Vec<M>>>`)の 2 つに分けた。タプルにすると `State` / `update_later` が `Box<dyn Any>` から `S` へ downcast できず、スロット値への射影関数を `State` に持たせる必要が出るため。2 スロットとも同じパスで訪問されるので sweep の挙動は変わらない。
- **1.4** `update_later` は `State` / `Handle` の両方に生えるが、実装は `state.rs` の `queue_update` 1 つに寄せた。`State` と `Handle` は `ctx: &'s egui::Context` の代わりに `store: &'s Store` を持つように変え(`ctx` は `store.ctx()` から取る)、`State::new` / `Handle::new` のシグネチャが `(store, slot, location)` / `(store, slot)` になった。
- **1.4** 遅延キューの適用は sweep より前なので、公開 API の範囲では「スロットが既に無い」経路には到達しない(そのパスで unmount されるスロットもまだ生きている)。`slot_by_id` が `None` の場合に黙って捨てる分岐は防御的なもので、テスト `deferred::update_later_on_a_slot_that_unmounts_in_the_same_pass_is_dropped` は「同じパスで unmount される state への `update_later` が panic しない」ことまでを確認する。
- **1.6** オーバーレイの文言は `Collision::location` ごとに `BTreeSet` で重複を落とす。kittest からは `query_by_label_contains` で読める。
- **1.7** `Surface` は `pub` にせず `cx.rs` の非公開 enum にした。外から必要なのは `Cx::new` / `Cx::new_taffy` / `in_taffy()` だけである。`Cx::hook_scope` は `Surface` を短い lifetime に再借用する `reborrow()` を経由する。`container` の `reserve_available_space()` 版(ランナーのルート用)はフェーズ 5 で足す。
- **1.8** `ContainerStyle` の `justify` / `align` を `Option` にはできない。`rsx!` が `justify="center"` を渡せるためには `From<&str>` が要り、`impl From<&str> for Option<Justify>` は orphan rule に反するため。代わりに `Justify` / `Align` に既定値 `Normal`(= 未指定、taffy では `None`)の variant を足した。`align_content` は taffy の `AlignContent` が `JustifyContent` と同じ型なので、プランの `Option<Align>` ではなく `Option<Justify>` にした。`AlignSelf` は taffy と同じく `Align` の型エイリアスである。`ItemStyle.align_self` は `Option<Align>` のままで、setter が `impl Into<AlignSelf>` を取る。
- **1.8** 等幅カラムは `taffy::style_helpers::evenly_sized_tracks(cols)` を使う。プランの `vec![fr(1.0); cols]` と等価だが、taffy 0.9 ではこれは `repeat(cols, 1fr)` 1 要素の `Vec` になる。
- **1.8** `Length::Percent` は taffy に合わせて 0.0〜1.0 の割合を持つ。`"50%"` は `Percent(0.5)` になる。
- **1.9** プランの表に無い `tests/layout.rs`(`Length` とレイアウト enum のパース、`m` / `p` 短縮形の優先順位、`to_taffy` / `merge` の写り方)を足した。テスト 2-7 の「`cx.scope` が Taffy モードでも Id を分ける」は、hooks 側(`use_state`)と egui 側(`ui.collapsing`)の 2 本に分けて確認している。
- **その他** `Store::end_pass` を `run_deferred` → `sweep` → `show_collision_overlay` の 3 つに分割した。`react-egui` は `egui_taffy::taffy` を `react_egui::taffy` として re-export する。

### フェーズ 3(手順 2)

- **0 依存表** `syn` は 2 ではなく **3.0**。rstml 0.13 が syn 3 に依存しており、`Node` / `KeyedAttribute` が syn 3 の型を埋め込んでいるので選択の余地がない。feature は `full` / `extra-traits`(`Node<C>` の `Debug` 導出に必要)/ `visit` / `visit-mut` / `parsing` / `printing` / `proc-macro`。`typed-builder` は 0.23、`trybuild` は 1.0。
- **2.1** `#[builder(crate_module_path = ::react_egui::__private::typed_builder)]` は再エクスポート経由でそのまま動いた。6 章の「自前 builder を生成する」代替案は不要。
- **2.3** `props_builder(&Name)` の推論も `'e` + generic `C` を持つ props で通った。6 章の「関数と同名の braced struct を生成する」代替案は不要。`Props` trait と `props_builder` は `react_egui::__private` に置き、`props_builder` だけクレート直下にも再エクスポートしている。
- **2.1** `#[event]` の引数は Props のフィールドにはならない(`Emitter` になるだけ)。イベント enum は、ペイロード型が実際に使うジェネリクスだけを引き継ぐ(`#[event] on_rename: &str` なら `NameEvent<'e>`)。使わないパラメータを enum に宣言できないため。
- **2.1** `events` フィールドは `#[builder(default, setter(strip_option))]`。`Option<&mut dyn FnMut(E)>` をそのまま setter に渡させるのは煩雑なので、`rsx!` は `.events(&mut |ev| ..)` と書ける。`events=` escape hatch も `&mut (expr)` で包んで渡す。
- **2.1** 本体末尾式の書き換えは `Stmt::Expr(_, None)` だけでなく `Stmt::Macro`(セミコロン無し)も対象にする。`rsx! { .. }` を本体の末尾に書くと syn は文マクロとしてパースするため。
- **2.1** `impl Trait` 引数は `TProp0`, `TProp1`, .. という型パラメータに脱糖する。props 構造体・関数・`Props` impl の 3 か所に同じジェネリクス(`'e` + 関数自身のパラメータ + `TProp*`)を付ける。`'e` は実際に使われる場合だけ宣言する(未使用パラメータはエラーになるため)。
- **2.2** `#[hook]` は「最初の `&mut Cx` 引数」を探す(第 1 引数に限定していない)。`&mut Cx` の判定は型の最終セグメントが `Cx` かどうかで行う。
- **2.3** `Option<T>` prop は `#[builder(default)]` のみで `strip_option` は付けない(プラン 2.1 の表どおり)。したがって `hint={Some("x")}` と書く。将来 elements で煩雑になれば見直す。
- **2.3** 要素の Id の材料は `(file!(), line!(), column!(), 通し番号, key)`。`line!()` / `column!()` は `rsx!` の呼び出し位置を返すので、同じ `rsx!` 内の要素は通し番号で、別の `rsx!` は位置で区別される。`Span::line()` / `Span::column()` は不要だった。`file!()` を足したのは、同じ位置に展開される別ファイルの `rsx!` を確実に分けるため。
- **2.3** 属性値と `{expr}` ノードは、単一式のブロックなら中身を取り出して emit する。`{ expr }` をそのまま渡すとユーザーコードに `unused_braces` 警告が出るため。
- **2.3** 不正なノード(引用符無しテキスト、`<!DOCTYPE>`)があった場合、`rsx!` は `compile_error!` と空の `view` だけを emit する。要素展開を続けると型エラーが連鎖して本来のメッセージが埋もれるため。
- **2.3 / 3.6** 融合閉包は `NameEvent::Variant` を名指しするので、`on_*` を使う場所では `NameEvent` も import されている必要がある。プランの「ユーザーは `Name` だけを `use` すればよい」は props についてのみ成立する。ARCHITECTURE.md 3.6 に明記した。
- **2.4** trybuild は 9 本(`compile_fail` 8 + `pass` 1)。`.stderr` はコミット済み。`missing_prop` は typed-builder の `Error_Missing_required_field_label` 型のエラーになる。
- **2.5** `tests/common/mod.rs` はマクロ版の `Counter` / `NamedCounter` / `Dialog` / `use_counter` を持ち、加えて `counter(cx, initial)` / `named_counter(..)` という薄いラッパ関数(`rsx!` を 1 行呼ぶだけ)を残した。これで `sibling_handlers` / `custom_hook` / `collision` は無修正のまま通る。`fused_events` だけは手書きの `DialogProps { .. }` を組み立てていたので、`rsx!` + `on_ok` / `on_cancel` / `on_rename` に書き換えた(assert は無修正)。
- **2.6** テストファイル名はプランどおり `rsx_control_flow.rs` / `rsx_children.rs` / `component_props.rs` / `component_events.rs` / `rsx_scope.rs` / `compile_fail.rs`。
- **その他** `examples/spike` はマクロ版に置き換えた(`App` / `Counter` / `Dialog`)。`main.rs` は `rsx! { <components::App/> }.show(&mut cx)` を呼ぶ。

### フェーズ 4(手順 3)

#### フェーズ 3 への追随

- **2.1** `Option<T>` prop は `#[builder(default, setter(strip_option))]` になった。`hint="x"` / `size={14.0}` と書ける。`#[prop(into)]` と併用すると `setter(into, strip_option)`。フェーズ 3 の「`strip_option` は付けない」判断はここで撤回した。trybuild の `.stderr` は影響を受けなかった。

#### 実装

- **3.1** `Gap`(`From<f32>` / `From<i32>` / `From<(f32, f32)>`)は `react_egui::layout` に置いた。`ContainerStyle::gap` の隣にあるべき型で、`ContainerStyle::gap()` setter も `impl Into<Gap>` を取るようにした。`ItemStyle` には `col_span` / `row_span` を足し、`rsx!` のレイアウト属性一覧にも加えた。
- **3.1** `View` の `align_content` は `Option<Justify>`(フェーズ 2 の型どおり)。`display` / `direction` / `justify` / `align` / `gap` / `side` は `#[prop(default, into)]` で、文字列リテラルをそのまま受ける。
- **3.2** `ComboBox` の `options` は `&[impl AsRef<str>]` ではなく generic `S: AsRef<str>` の `&[S]`。`#[component]` は引数型のトップレベルの `impl Trait` しか脱糖しないため。
- **3.2 / 5.6** `bind` を `&mut *state` で渡すと `DerefMut` が毎フレーム dirty を立て、アプリがアイドルにならない(kittest が `ExceededMaxSteps` で落ちる)。`State::bind(&mut self) -> &mut T` を core に足した。dirty を立てずに `&mut T` を貸すだけで、値が変わるのは入力があった時だけなので repaint は egui 側が出す。ARCHITECTURE.md 5.6 に追記した。
- **3.3** egui 0.36 には `SidePanel` / `TopBottomPanel` が無く、`Panel::left/right/top/bottom` に統合されている。要素も `Panel`(`side="left"|"right"|"top"|"bottom"`)1 つ + `CentralPanel` にした。`Side` enum は `react-egui-elements` に置く。
- **3.3** `Grid` の行区切りは `<Row/>` 要素ではなく `row()` という `impl View` を返す関数にし、`{row()}` と書く。`rsx!` は要素ごとに `cx.scope` → `Ui::push_id` で子 `Ui` を作るので、`<Row/>` の中の `ui.end_row()` は grid の `Ui` に届かない。`{expr}` ノードはスコープされないので届く。
- **3.3** 同じ理由で、`<Panel>` と `<CentralPanel>` を兄弟要素として並べてもドッキングしない(それぞれが自分の子 `Ui` から場所を切り取り、親のカーソルはその下に進む)。スコープを挟まずに同じ `Ui` へ描けば期待どおり並ぶことをテスト `containers::panels_dock_when_they_share_one_ui` で固定した。パネルはランナーのルートで使う想定。ARCHITECTURE.md 6 に明記した。
- **3.4 テスト 4-3** `grow` と `justify="space-between"` は余白の分配なので、`<View>` に `w` が無いと差が出ない(Ui モードの `container` は `reserve_available_width()` で親の幅を確保するが、taffy ノード自身の `size.width` は `auto` のまま)。テストでは `w={300.0}` を付けた。
- **3.4 テスト 4-5** core の `multi_pass.rs` はそのまま残し、`react-egui-elements/tests/multi_pass.rs` に `<View>` + `<Button>` + `<Text>` 版を足した(core が elements に依存しないため)。taffy の再計算は「同じパスの中でノードの内容が変わった」時に起きるので、幅の変わる `<Text>` はハンドラより**後**に書く必要がある。
- **3.4 テスト 4-4** スナップショットは feature `snapshot`(`egui_kittest/snapshot` + `egui_kittest/wgpu`)の裏。このマシンでは wgpu が動いたので 5 枚の PNG を生成してコミットした(`row` / `column_justify` / `grid` / `text_wrap` / `widgets`)。
- **その他** `View` 要素(関数、値の名前空間)と `View` trait(型の名前空間)は共存できるので、`react_egui::prelude` と `react_egui_elements::prelude` を両方 glob import しても衝突しない。
