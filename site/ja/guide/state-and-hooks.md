---
title: 状態とフック
---

# 状態とフック

## `use_state` はガードを返す

```rust
let mut count = use_state(cx, || 0i32);
let mut name = use_state(cx, String::new);
```

`use_state(cx, init)` が返すのは `State<T>` です。ストアのスロットを指す`RefMut` のようなガードで、セッタもタプルもありません。

```rust
*count += 1;
name.push_str("!");
let shown: String = name.clone();
```

初期化関数はコンポーネントが最初に描かれるとき 1 回だけ走ります。値はストアにあるので、`T` が `Clone` である必要はありません。ガードはコンポーネントの本体と一緒に死にます。だから並んだ 2 つのハンドラが、それぞれ可変で借りられます。

あとに続くハンドラが状態を `&mut` で取るなら、値は先に読んでおきます。

```rust
let mut selected = use_state(cx, || 0usize);
let current = *selected;
```

## `bind()` と `&mut *state`

可変 deref はその状態に dirty の印を付け、dirty な状態は再描画を要求します。クリックハンドラにはちょうどいい挙動です。毎フレーム書くウィジェットには困ります。アプリが永久に再描画し続けてしまいます。

束縛できるウィジェットは `state.bind()` を取ります。これは dirty の印を付けずに `&mut T` を渡します。

```rust
<TextEdit bind={name.bind()} hint="name"/>
<Checkbox bind={done.bind()} label="done"/>
<Slider bind={amount.bind()} range={0.0..=1.0} label="amount"/>
```

これで失うものはありません。こうしたウィジェットが値を変えるのは入力があったときだけで、入力があれば egui はどのみち再描画します。

ウィジェットが状態を可変で握るので、同じ要素の `on_change` からそれを触ることはできません（`E0502`）。`on_change` は `Dispatch` など、ほかのものに知らせるために使ってください。

## `Handle`

`Handle<T>` は同じスロットを指す `Copy` なハンドルで、cell のような API を持ちます。`.get()`、`.set(v)`、`.update(|v| ..)`、`.with(|v| ..)`。フレームの中で状態を持ち回すときや、`provide_context` に渡すときに使います。

```rust
let theme = use_handle(cx, || Theme { dark: true });   // ガードは作らない
let handle = count.into_handle();                      // ガードを手放す
```

手に入れる方法はこの 2 つだけです。同じスロットのガードとハンドルを同時に持つことはできません。二重借用になって panic します。

## コンテキスト

`provide_context` は `Handle` を子に公開します。`use_context::<T>` は型でいちばん近いものを探します。

```rust
#[component(shares_ui)]
fn Themed(cx: &mut Cx, children: impl View) {
    let theme = use_handle(cx, || Theme { dark: true });
    provide_context(cx, theme, |cx| children.show(cx));
}

#[component]
fn Swatch(cx: &mut Cx) {
    let theme = use_context::<Theme>(cx).map_or(Theme { dark: true }, |t| t.get());
    rsx! { <Text>{theme.name()}</Text> }
}
```

コンテキストごとに専用の型を作ってください。`<Provide value={..}>` のような要素はありません。プロバイダは、その値を自分で作るコンポーネントである必要があります。

## `use_reducer` と `Dispatch`

変更が決まった種類のメッセージで表せるなら、reducer が 1 か所にまとめます。

```rust
enum Msg {
    Add(String),
    Toggle(usize),
    Remove(usize),
}

let (todos, dispatch) = use_reducer(
    cx,
    |state: &mut Vec<Todo>, msg| reduce(state, msg),
    Vec::new,
);

rsx! { <Button on_click={|| dispatch.send(Msg::Add(draft.clone()))}>"add"</Button> }
```

`dispatch.send` はメッセージをキューに積み、再描画を要求します。reducer は次のフレームで走ります。

`Dispatch<M>` は `Clone + Send + 'static` で、フレームより長生きできる唯一のフックハンドルです。スレッドや future に渡せば、そこから結果を送り返せます。

## `use_persisted`

ガードは同じですが、eframe のストレージに保存され、次の起動時に読み戻されます。

```rust
let mut todos = use_persisted(cx, "todo/todos", Vec::<Todo>::new);
```

`T` は `Serialize + DeserializeOwned` である必要があります。鍵は呼び出し位置ではなく文字列なので、コードを編集しても保存済みのデータは読めます。同じ鍵を2 か所で使えば、同じ値です。保存データはすべて 1 つの名前空間を共有するので、鍵には機能名を前置してください（`"todo/todos"`）。

## よく出会う借用エラー 2 つ

どちらも普通の Rust の話で、どちらも 1 行で直ります。

### ループが回している集合をハンドラが編集する

```rust
for (i, todo) in todos.iter().enumerate() {
    // `todos` はループに借りられているので、これは通らない:
    <Button on_click={|| todos.remove(i)}>"x"</Button>
}
```

書き込みをキューに積みます。`update_later` はフレームの終わりに走り、再描画を要求します。

```rust
<Button key={i} on_click={|| todos.update_later(move |t| { t.remove(i); })}>"x"</Button>
```

このクロージャは `'static` なので、ローカルを捕まえるには `move` が要ります。状態を触らない仕事には、同じキューの `cx.defer(f)` を使います。

### 同じ状態に対する値 prop とハンドラ

```rust
// E0502: props が `title` を共有借用し、
// ハンドラが可変借用している。
<Dialog title={&*title} on_rename={|s| *title = s}/>
```

値をクローンして渡す（`title={title.clone()}`）か、書き込みを `update_later`か `Dispatch` に回してください。`rsx!` が暗黙にクローンすることはありません。`bind` を軸に組んだコンポーネントなら、この形にはなりません。

## ほかのフック

`use_effect`、`use_memo`、`use_animate`、`use_future`、それに自作の `#[hook]`は [フックのリファレンス](/ja/reference/hooks) にあります。

## 動かして見る

ガードは [counter](/ja/examples/counter)、`bind` は [form](/ja/examples/form)、`use_reducer` と `use_persisted` は [todo](/ja/examples/todo)、コンテキストは[theme](/ja/examples/theme)、`#[hook]` は[custom-hook](/ja/examples/custom-hook) を見てください。ストアと再描画の方針は[docs/ARCHITECTURE.md](https://github.com/fand/egui-reactor/blob/main/docs/ARCHITECTURE.md#5-runtime)の 5 章にあります（英語）。
