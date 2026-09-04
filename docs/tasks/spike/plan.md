# プラン: spike

タスク定義は [task.md](task.md)。設計の根拠は [docs/ARCHITECTURE.md](../../ARCHITECTURE.md)。本書は実装手順と確認方法を定める。実装中にここから外れる判断をした場合は本書を更新する。

## 1. ワークスペース

```
Cargo.toml                      workspace。members = crates/*, examples/*
rust-toolchain.toml             channel = stable(egui_taffy の MSRV 以上)、targets = ["wasm32-unknown-unknown"]
LICENSE-MIT / LICENSE-APACHE
README.md                       1 段落の説明と ARCHITECTURE.md へのリンク
.github/workflows/ci.yml
crates/react-egui/              core(本 PR の実体)
crates/react-egui-macros/       proc-macro クレート。lib.rs は空
crates/react-egui-elements/     空
crates/react-egui-app/          空
examples/spike/                 eframe バイナリ。Counter と Dialog を表示
```

`[workspace.dependencies]` に以下を pin する。

| crate | version |
|---|---|
| egui / eframe / egui_kittest | 0.36 |
| egui_taffy | 0.14 |
| rstml | 0.13(macros に依存だけ張る。未使用) |
| elsa | 最新 |
| log | 最新 |

CI(`ci.yml`、ubuntu-latest):

1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. `cargo check -p react-egui --target wasm32-unknown-unknown`

egui_kittest は `wgpu` / `snapshot` feature を有効にしない(ヘッドレスで動かす)。

## 2. core の実装(`crates/react-egui/src/`)

### 2.1 `store.rs`

```rust
pub struct Store {
    slots: elsa::FrozenMap<egui::Id, Box<Slot>>,   // &self で insert でき、&Slot が安定
    pass: Cell<u64>,
    collisions: RefCell<Vec<Collision>>,
}
struct Slot {
    value: RefCell<Box<dyn Any>>,
    last_visited: Cell<u64>,
    cleanup: RefCell<Option<Box<dyn FnOnce()>>>,   // use_effect 用
    deps_hash: Cell<Option<u64>>,                  // use_effect 用
    location: &'static Location<'static>,          // 衝突報告用
}
pub struct Collision { pub id: egui::Id, pub location: &'static Location<'static> }
```

- `begin_pass(&mut self)`: `pass += 1`。
- `slot(&self, id, location, init) -> &Slot`: 存在すれば返す。`last_visited == pass` なら衝突として `collisions` に記録し `log::warn!`。無ければ `init` で作って insert。いずれも `last_visited = pass` に更新。
- `end_pass(&mut self)`: `last_visited < pass` のスロットを列挙し、cleanup を実行して削除する(`FrozenMap::as_mut()` で `&mut HashMap` を取る)。`collisions` を返すか取り出せるようにする。
- guard が生きている間に `end_pass` は呼ばれない(呼び出し側の責務。ランナーはコンポーネント本体を抜けた後に呼ぶ)。

### 2.2 `cx.rs`

```rust
pub struct Cx<'s, 'u> {
    pub store: &'s Store,
    pub ui: &'u mut egui::Ui,
    scope: egui::Id,
    contexts: &'s ContextStack,   // 2.5 参照。's でよい(Store が所有)
}
impl<'s, 'u> Cx<'s, 'u> {
    pub fn new(store: &'s Store, ui: &'u mut Ui, scope: Id) -> Self;
    pub fn scope_id(&self) -> Id;
    /// コンポーネント境界。scope を深くし、ui.push_id も行う
    pub fn scope<R>(&mut self, source: impl Hash, f: impl FnOnce(&mut Cx<'s, '_>) -> R) -> R;
    /// カスタム hook 境界。scope だけ深くする(ui.push_id はしない)
    pub fn hook_scope<R>(&mut self, loc: &'static Location<'static>, f: impl FnOnce(&mut Cx<'s, '_>) -> R) -> R;
    pub fn ctx(&self) -> &egui::Context;
}
```

egui のコンテナ閉包(`ui.vertical(|ui| ..)`)の内側では、`store` と `scope` をコピーして `Cx::new(store, ui, scope)` で作り直す。Rust 2021 の閉包は構造体のフィールドを個別に捕獲するので、`cx.ui.vertical(|ui| { let mut cx = Cx::new(store, ui, scope); .. })` は `cx.ui` の可変借用と衝突しない。この形をマクロが生成する前提で、spike では手書きする。

### 2.3 `state.rs`

```rust
pub struct State<'s, T> {
    inner: RefMut<'s, T>,      // RefMut::map(slot.value.borrow_mut(), downcast)
    dirty: bool,
    ctx: egui::Context,
}
impl Deref / DerefMut          // DerefMut で dirty = true
impl Drop                      // dirty なら ctx.request_repaint()
impl State { pub fn into_handle(self) -> Handle<'s, T>; }   // guard を消費して Handle に変える

#[derive(Clone, Copy)]
pub struct Handle<'s, T> { slot: &'s Slot, ctx: &'s egui::Context?, _t: PhantomData<T> }
impl Handle { get()(T: Clone), set(v), update(|&mut T|), with(|&T| -> R) }   // 各呼び出しで一瞬だけ borrow。set/update は request_repaint
```

`Handle` が `Copy` であるために `egui::Context` を値で持てない。`Store` に `Context` のクローンを 1 つ持たせ、`&'s Context` を参照させる(`begin_pass(&mut self, ctx: &Context)` で更新)。

`State::handle(&self)` は提供しない。guard が生きたまま `Handle` を使うと同じ `RefCell` の二重借用で panic するため、必ず `into_handle` で guard を消費させる。この点は ARCHITECTURE.md 3.5 に追記する。

### 2.4 `hooks.rs`

```rust
#[track_caller]
pub fn use_state<'s, T: 'static>(cx: &mut Cx<'s, '_>, init: impl FnOnce() -> T) -> State<'s, T>;
#[track_caller]
pub fn use_handle<'s, T: 'static>(cx: &mut Cx<'s, '_>, init: impl FnOnce() -> T) -> Handle<'s, T>;
#[track_caller]
pub fn use_effect<D: Hash, C: IntoCleanup>(cx: &mut Cx, deps: D, f: impl FnOnce() -> C);
```

- Id は `cx.scope_id().with(Location::caller())`。`Location` は `(file, line, column)` を hash する。
- `use_effect`: deps を `std::hash::DefaultHasher` で hash し、スロットの `deps_hash` と比較。異なる(または初回)なら、既存 cleanup を実行してから `f` を**その場で**呼び、返された cleanup を保存。`IntoCleanup` は `()` と `F: FnOnce() + 'static` に実装する。

### 2.5 `context.rs`(最小)

```rust
pub fn provide_context<'s, T: 'static>(cx: &mut Cx<'s, '_>, value: Handle<'s, T>, children: impl FnOnce(&mut Cx<'s, '_>));
pub fn use_context<'s, T: 'static>(cx: &Cx<'s, '_>) -> Option<Handle<'s, T>>;
```

`Store` が `ContextStack: RefCell<Vec<(TypeId, *const ())>>` 相当のスタックを持ち、`provide_context` は push → children → pop する。`Handle<'s, T>` は `Copy` かつ `'s` なので、`Vec<(TypeId, Box<dyn Any>)>` に `Handle` を値で入れれば unsafe は不要。`children` の後に必ず pop する(panic 安全性は spike では考えない)。

### 2.6 `events.rs`(マクロ生成物の手書き版)

```rust
pub trait Handler<A> { fn call(self, a: A); }
impl<F: FnOnce()> Handler<()> for F;         // 引数を捨てる版と受ける版の両立方法を確認する
impl<F: FnOnce(A), A> Handler<A> for F;      // ↑ と impl が衝突するなら、Handler0 / Handler1 に分けてマクロ側で引数の有無を見て選ぶ

pub struct Emitter<'e, E> { sink: &'e RefCell<&'e mut dyn FnMut(E)> }
impl<E> Emitter<'_, E> { pub fn emit(&self, e: E); }
```

`Handler` の 2 つの blanket impl は coherence で衝突する可能性が高い。衝突した場合はマクロが閉包の引数個数を構文的に見て `call0` / `call1` を選ぶ方針にし、その旨を ARCHITECTURE.md 3.6 に追記する。spike ではどちらで成立するかを確定させる。

### 2.7 `lib.rs`

上記を re-export。`prelude` モジュールを置く。

## 3. 手書き展開コード(`crates/react-egui/tests/common/`)

マクロが生成するはずのコードを、以下の形で手書きする。テストと examples の両方から使うので `tests/common/mod.rs` と `examples/spike/src/components.rs` に置く(重複は許容。マクロ導入時に消える)。

### Counter

```rust
pub fn counter(cx: &mut Cx, initial: i32) {
    let mut count = use_state(cx, || initial);
    let (store, scope) = (cx.store, cx.scope_id());
    cx.ui.horizontal(|ui| {
        let mut cx = Cx::new(store, ui, scope);
        if cx.ui.button("-").clicked() { (|| *count -= 1)(); }
        cx.ui.label(format!("{}", *count));
        if cx.ui.button("+").clicked() { (|| *count += 1)(); }
    });
}
```

### Dialog(2 つの callback props)

```rust
pub enum DialogEvent { Ok(()), Cancel(()) }
pub struct DialogProps<'e> { pub title: &'e str, pub events: &'e mut dyn FnMut(DialogEvent) }
pub fn dialog(cx: &mut Cx, props: DialogProps<'_>) {
    let sink = RefCell::new(props.events);
    let on_ok = Emitter { sink: &sink };
    let on_cancel = Emitter { sink: &sink };
    cx.ui.label(props.title);
    if cx.ui.button("OK").clicked() { on_ok.emit(DialogEvent::Ok(())); }
    if cx.ui.button("Cancel").clicked() { on_cancel.emit(DialogEvent::Cancel(())); }
}

// 親側の展開形
let mut open = use_state(cx, || true);
if *open {
    dialog(cx, DialogProps {
        title: "Quit?",
        events: &mut |ev| match ev {
            DialogEvent::Ok(a)     => Handler::call(|| *open = false, a),
            DialogEvent::Cancel(a) => Handler::call(|| *open = false, a),
        },
    });
}
```

### カスタム hook(`#[hook]` の展開形)

```rust
#[track_caller]
pub fn use_counter<'s>(cx: &mut Cx<'s, '_>) -> State<'s, i32> {
    let loc = Location::caller();
    cx.hook_scope(loc, |cx| use_state(cx, || 0))
}
```

## 4. テスト(`crates/react-egui/tests/`)

各テストは `egui_kittest::Harness::new_ui_state(|ui, store: &mut Store| { .. }, Store::new())` の形で書き、閉包の中で `store.begin_pass(ui.ctx())` → `Cx::new(&*store, ui, Id::new("root"))` でコンポーネントを描く → guard が落ちたあと `store.end_pass()` を呼ぶ。この一連を `tests/common/run.rs` の `run_app(ui, store, |cx| ..)` にまとめる。

| # | ARCHITECTURE.md 10 章の項目 | テストファイル | 確認内容 |
|---|---|---|---|
| 1 | guard が本体の間だけ生き、兄弟ハンドラが同じ state を順に `&mut` 借用できる | `sibling_handlers.rs` | Counter の `+` `-` をクリックし、ラベルが 1 → 2 → 1 になる |
| 2 | `#[hook]` 越しに guard を返せる | `custom_hook.rs` | `use_counter` を同一コンポーネントで 2 回呼び、それぞれ独立にインクリメントできる |
| 3 | 融合閉包と `Handler` | `fused_events.rs` | Dialog の OK / Cancel それぞれで `open` が false になる。両方の閉包が同一 state を捕獲してコンパイルが通ること自体が主目的 |
| 4 | `use_context` の `Handle` と親の guard の共存 | `context_handle.rs` | 親が `use_handle` で theme を持ち `provide_context`、子が `use_context().set()`、親が children の後に `.get()` で新値を読む。同時に親は別スロットの `State` guard を握っている |
| 5 | 多重パスでハンドラが 1 回だけ発火し effect が再実行されない | `multi_pass.rs` | (a) 1 パス目で `ctx.request_discard()` を呼ぶ手動版、(b) egui_taffy の flex 内でクリックによりラベル幅が変わる版。両方で count が +1 のみ、effect 実行回数が 1。テストが実際に 2 パス走ったことを、ルート閉包の呼び出し回数で確認する。`ctx.options_mut(|o| o.max_passes = 2)` を設定 |
| 6 | Id 衝突検出 | `collision.rs` | (a) `hook_scope` の無いヘルパーを 2 回呼ぶ、(b) `for` 内で key 無しに `use_state` を呼ぶ。両方で `store.collisions()` が空でない。対照として `cx.scope(i, ..)` で包んだ版は空 |
| 7 | sweep が状態を破棄し cleanup を走らせる | `unmount.rs` | `if show { child }` の child が `use_effect((), || { log.push("mount"); move || log.push("cleanup") })` と `use_state`。show を true → false → true にして、ログが `[mount, cleanup, mount]`、state が初期値に戻る。ログは `Arc<Mutex<Vec<&str>>>` |
| 8 | egui コンテナ閉包の内側で新しい `Cx` を作り hooks が動く | `nested_ui.rs` | `ui.vertical` の中で `use_state` を持つ子を描き、外側と内側の state が独立し、フレームを跨いで保持される |
| 9 | repaint ポリシー | `repaint.rs` | `DerefMut` を呼んだフレームだけ `ctx.has_requested_repaint()` が true。読むだけのフレームは false |
| 10 | `use_effect` の deps | `effect_deps.rs` | deps が同じフレームでは再実行されず、変わると cleanup → 本体の順で走る |

kittest の操作は `harness.get_by_label("+").click(); harness.run();` の形。ラベルの取得は AccessKit 経由なので、ボタンのテキストを一意にする。

## 5. examples/spike

eframe の `App::ui` で `Store` を `begin_pass` / `end_pass` し、Counter と Dialog(開くボタン付き)を描く。`Options::max_passes = 2` を設定する。動作を目視で確認するためのもので、テストではない。

## 6. 手順

1. ワークスペースと CI を作り、空クレートで CI が緑になることを確認する。
2. `store.rs` → `cx.rs` → `state.rs` → `hooks.rs` の順に実装し、テスト 1, 8, 9, 10 を通す。
3. `events.rs` を実装し、`Handler` の coherence 問題を確定させ、テスト 3 を通す。
4. `hook_scope` と衝突検出を実装し、テスト 2, 6 を通す。
5. sweep と cleanup を実装し、テスト 7 を通す。
6. `context.rs` を実装し、テスト 4 を通す。
7. egui_taffy を dev-dependency に追加し、テスト 5 を通す。
8. examples/spike を書き、`cargo run -p spike` で目視確認する。
9. 検証で崩れた前提を ARCHITECTURE.md に反映する。少なくとも `into_handle`(2.3)と `Handler` の結論(2.6)は追記が必要になる。
10. PR 本文に、検証項目ごとの結果と ARCHITECTURE.md の変更点を書く。

## 7. 判断が必要になりそうな点

- `Handler` の blanket impl が衝突した場合の方針(2.6 に記載)。
- `elsa::FrozenMap` の `as_mut()` が使いにくい場合、`Store` を `slots: UnsafeCell<HashMap<Id, Box<Slot>>>` で自前実装してもよい。その場合は安全性の根拠(insert は `Box` の中身を動かさない、削除は `&mut self` でのみ行う)をコメントに書く。
- kittest で `num_completed_passes` を直接読めない場合は、ルート閉包の呼び出し回数を `Store` 外のカウンタで数える。
- テスト 5(b) で egui_taffy が discard を要求しない場合は、(a) の手動版が通っていれば項目 5 は満たしたとみなし、(b) は削除して理由を PR 本文に書く。
