---
title: フック
---

# フック

このページのものはすべて `egui_react::prelude` から export されています。

**deps は `Hash` で比べます。** `use_memo`、`use_effect`、`use_future` はどれも`deps` を取り、そのハッシュが変わると走り直します。`PartialEq` ではなくハッシュにしているおかげで、deps を借用できます。`(&str, &[T])` は正しい depsのタプルです。ハッシュの衝突は、egui 自身の id と同じ程度に無視できます。`()` を deps にすると「マウント時に 1 回」の意味になります。

**フックの id は呼び出し位置から作られます。** だから `if` の中でフックを呼べますし、2 行に分かれた 2 つの呼び出しが同じスロットを共有することはありません。ループの中では、囲んでいる要素に `key` を付けてください。自分の関数の中で使うなら `#[hook]` を付けます。

## `use_state`

```rust
pub fn use_state<'s, T: 'static>(cx: &mut Cx<'s, '_>, init: impl FnOnce() -> T) -> State<'s, T>
```

`T` をストアに置き、そこを指すガードを返します。`init` が走るのは、このコンポーネントのインスタンスが最初に描かれるときだけです。ガードは両方向にderef するので、`*count += 1` がすべてです。deref-mut はスロットに dirty の印を付け、ガードが落ちるときに再描画を要求します。

`state.bind()` は dirty の印を **付けずに** `&mut T` を渡します。毎フレーム書くウィジェット用です。`state.into_handle()` はガードを手放して `Copy` な`Handle` を返します。`state.update_later(f)` は書き込みをパスの終わりまでキューに積みます。

```rust
let mut count = use_state(cx, || 0i32);
rsx! { <Button on_click={|| *count += 1}>{format!("{}", *count)}</Button> }
```

## `use_handle`

```rust
pub fn use_handle<'s, T: 'static>(cx: &mut Cx<'s, '_>, init: impl FnOnce() -> T) -> Handle<'s, T>
```

同じスロットを、最初からガードを作らずに `Handle` として取ります。`Handle` は`Copy` で、`&self` 越しに動きます。`.get()`（`T: Clone` が要る）、`.set(v)`、`.update(|v| ..)`、`.with(|v| ..)`、`.update_later(f)`。`provide_context` で公開したい状態や、フレームの中で持ち回す状態に使ってください。

```rust
let theme = use_handle(cx, || Theme { dark: true });
provide_context(cx, theme, |cx| children.show(cx));
```

## `use_persisted`

```rust
pub fn use_persisted<'s, T: Serialize + DeserializeOwned + 'static>(
    cx: &mut Cx<'s, '_>,
    key: &str,
    init: impl FnOnce() -> T,
) -> State<'s, T>
```

再起動をまたぐ `use_state` です。値は JSON として、1 つの鍵で eframe のストレージに保存されます。スロットを決めるのは呼び出し位置ではなく、渡した文字列です。行を 1 つ足しただけで保存データが読めなくなっては困るからです。同じ鍵を 2 回使えば同じ値です。鍵には、それが属する機能名を前置してください。読めない JSON が入っていた場合は、警告を出して無視し、`init` に戻ります。

```rust
let mut todos = use_persisted(cx, "todo/todos", Vec::<Todo>::new);
```

## `use_memo`

```rust
pub fn use_memo<'s, D: Hash, T: 'static>(
    cx: &mut Cx<'s, '_>,
    deps: D,
    f: impl FnOnce() -> T,
) -> &'s T
```

`deps` のハッシュが変わったときだけ `f` を走らせ、保持している値への参照を返します。この参照は `&mut Cx` の借用から独立しているので、`State` ガードやあとのフックと問題なく共存します。

本当に時間のかかる値に使ってください。絞り込んで整形したリスト、パース済みの文書など。安い値には要りません。これが節約するのは描画のコストではありません。

```rust
let filtered = use_memo(cx, (query.as_str(), removed.len()), || {
    rows.iter().filter(|r| r.contains(query.as_str())).cloned().collect::<Vec<_>>()
});
```

## `use_effect`

```rust
pub fn use_effect<D, C, M>(cx: &mut Cx<'_, '_>, deps: D, f: impl FnOnce() -> C)
```

`deps` のハッシュが変わったときと、最初の 1 回に `f` を走らせます。本体は呼び出し位置で **その場で** 走るので、ガードやローカルを借りられます。コミット後にエフェクトが走る React とは違うところです。

`f` は何も返さなくても、クリーンアップのクロージャ（`FnOnce() + 'static`）を返しても構いません。次に本体が走る前に、前回のクリーンアップが走ります。アンマウント時には、パス終わりの掃除が最後のクリーンアップを走らせます。クリーンアップは保存されるので `'static` です。ほかの状態を変えたいなら`Dispatch` を持たせてください。

```rust
let ctx = cx.ctx().clone();
use_effect(cx, dark, move || {
    ctx.set_visuals(if dark { egui::Visuals::dark() } else { egui::Visuals::light() });
});
```

## `use_animate` / `use_animate_with`

```rust
pub fn use_animate(cx: &mut Cx<'_, '_>, on: bool, time: f32) -> f32
pub fn use_animate_with(cx: &mut Cx<'_, '_>, on: bool, time: f32, easing: fn(f32) -> f32) -> f32
```

`on` が false の間は 0、true の間は 1 で、`on` が切り替わってからの `time` 秒間はイージングで補間します（既定は `cubic_out`）。値を持っているのは egui のアニメーションマネージャで、動いている間の再描画もそちらが要求します。だからストアにスロットは無く、掃除が落とすものもありません。

```rust
let down = use_animate(cx, open, 0.25);
if down <= 0.0 {
    return;
}
```

## `use_reducer`

```rust
pub fn use_reducer<'s, S: 'static, M: Send + 'static>(
    cx: &mut Cx<'s, '_>,
    reducer: impl FnMut(&mut S, M),
    init: impl FnOnce() -> S,
) -> (State<'s, S>, Dispatch<M>)
```

状態とメッセージキューの組です。`Dispatch::send` はメッセージを積んで`request_repaint` を呼びます。reducer が積まれたメッセージをたたむのは**次にこのフックが訪問されたとき** です。だから別スレッドから来たメッセージに余分なフレームがかかりません。キューは訪問のたびに空にされるので、マルチパスのフレームの 2 パス目で二重に適用されることもありません。

`Dispatch<M>` は `Clone + Send + 'static` で、パスより長生きできる唯一のフックハンドルです。だからこれが、スレッドや spawn した future から結果を返す手段になります。

```rust
let (todos, dispatch) = use_reducer(cx, |state: &mut Vec<Todo>, msg| reduce(state, msg), Vec::new);
```

## `provide_context` / `use_context`

```rust
pub fn provide_context<'s, T: 'static>(
    cx: &mut Cx<'s, '_>,
    value: Handle<'s, T>,
    children: impl FnOnce(&mut Cx<'s, '_>),
)
pub fn use_context<'s, T: 'static>(cx: &Cx<'s, '_>) -> Option<Handle<'s, T>>
```

値を型で部分木に配ります。束縛が効くのは `children` が走っている間だけです。返ってくるのはガードではなく `Handle` なので、プロバイダは同時に自分の状態を持てます。プロバイダの外では `use_context` は `None` を返します。

`<Provide value={handle}>` のような要素はありません。props の型はストアのライフタイムを名指しできないからです。値を自分で作るプロバイダのコンポーネントを書いてください。

```rust
#[component(shares_ui)]
fn Themed(cx: &mut Cx, children: impl View) {
    let theme = use_handle(cx, || Theme { dark: true });
    provide_context(cx, theme, |cx| children.show(cx));
}
```

## `use_future`

```rust
pub fn use_future<'s, D, T, F>(cx: &mut Cx<'s, '_>, deps: D, f: impl FnOnce() -> F) -> &'s Poll<T>
```

`deps` のハッシュが変わると future を作って走らせ、`&Poll<T>` を返します（`Poll` は prelude から再 export しています）。`f` はその場で走るので、ガードやローカルを読んで future を組み立てられます。future 自体は `'static` で、捕まえたものを所有します。`T` は `Send + 'static` である必要があります。

deps が変わったあとに届いた結果は捨てられるので、古いレスポンスが新しいものを上書きすることはありません。保留の間、このフックは自分をいちばん近い`<Suspense>` の数に入れます。

```rust
let response = use_future(cx, (url, attempt), || {
    let request = ehttp::Request::get(url);
    async move { ehttp::fetch_async(request).await }
});
let Poll::Ready(response) = response else {
    return;
};
```

## `spawn`

```rust
pub fn spawn(fut: impl SpawnFuture<()>)
```

フックではありません。同じエグゼキュータ（ネイティブでは`pollster::block_on` を回すスレッド、Web では `spawn_local`）に投げっぱなしの仕事を流します。何も返さないので、結果は `Dispatch` で知らせてください。

```rust
spawn(async move { dispatch.send(Msg::Saved(save().await)); });
```

## `update_later` と `cx.defer`

```rust
state.update_later(|v: &mut T| ..)    // Handle にもある
cx.defer(|| ..)
```

これらもフックではありませんが、同じキューです。どちらもパスの終わり、掃除の前に走るので、そのパスでアンマウントされるスロットへの書き込みもちゃんと届きます。クロージャは `'static` なので、ローカルを捕まえるには `move` が要ります。`update_later` は適用時に再描画を要求します。`defer` は状態を触らないので要求しません。

「ループの中のハンドラが、そのループが回している集合を変更したい」の答えがこれです。

```rust
<Button key={i} on_click={|| todos.update_later(move |t| { t.remove(i); })}>"x"</Button>
```

## 自作のフック

`&mut Cx` を取ってほかのフックを呼ぶ、普通の関数に `#[hook]` を付けたものです。再利用できるようにしているのはこの属性です。**呼び出し位置** を鍵にしたスコープに入るので、2 か所からの呼び出しは衝突せず、それぞれの状態を持ちます。

```rust
#[hook]
pub fn use_previous<T: Clone + PartialEq + 'static>(cx: &mut Cx, value: T) -> Option<T> {
    let mut seen = use_state(cx, || (value.clone(), None::<T>));
    if seen.0 != value {
        let last = std::mem::replace(&mut seen.0, value);
        seen.1 = Some(last);
    }
    seen.1.clone()
}
```

## 用意していないもの

`use_callback` と `memo` はありません。差分を取らない以上、参照の同一性を保っても得るものが無いからです。フレームをまたぐ必要のあるコールバックは `Dispatch` です。

## もっと知る

意味論の全体 ― id の作り方、衝突の検出、掃除がすること、順序の保証 ― は[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#4-hooks-list)の 4 章にあります（英語）。
