---
title: 非同期
---

# 非同期

## `use_future`

```rust
let response = use_future(cx, (url, attempt), || {
    // ここで組み立てる。この時点なら `url` はまだ借りられる。future は
    // リクエストを所有し、`'static` になる。
    let request = ehttp::Request::get(url);
    async move { ehttp::fetch_async(request).await }
});
```

`use_future(cx, deps, f)` は `&Poll<T>` を返します。`f` はその場で走るので、ローカルや `State` ガードを読めます。返す future は `'static` でなければいけません。`deps` のハッシュが変わると、future は作り直されて再び走ります。`T` は `Send + 'static` である必要があります。

## `let`-`else` の型

Rust に `throw` はありません。まだ値の無いコンポーネントは、単に return します。

```rust
#[component]
fn Response(cx: &mut Cx, url: &str, attempt: u32) {
    let response = use_future(cx, (url, attempt), || {
        let request = ehttp::Request::get(url);
        async move { ehttp::fetch_async(request).await }
    });
    let Poll::Ready(response) = response else {
        return;
    };

    rsx! {
        match response {
            Ok(response) => { <Text strong>{format!("{}", response.status)}</Text> }
            Err(err) => { <Text>{format!("error: {err}")}</Text> }
        }
    }
}
```

`Poll` は prelude に入っています。保留中に何も描かないのは普通のことです。待っている状態を見せるのは、上にある境界の仕事です。

## `<Suspense>`

```rust
rsx! {
    <Suspense fallback={view(|cx| { cx.ui().spinner(); })}>
        <Response url={url.as_str()} attempt={*attempt}/>
    </Suspense>
}
```

`<Suspense>` は、その下のどれかの `use_future` が保留の間 `fallback` を描きます。子は画面の外で描かれ続けるので、その future はちゃんと始まり、終わります。切り替わるのは同じフレームの中なので、隙間はできません。

React との違いが 2 つあります。中断された子の `use_effect` は走ります。それから `SuspenseList` も `useTransition` もありません。

## `spawn` と `Dispatch`

`use_future` は、コンポーネントが待つ値のためのものです。投げっぱなしにしてあとから結果を受け取る仕事には、`spawn` と `Dispatch` を使います。

```rust
let (state, dispatch) = use_reducer(cx, reduce, State::default);

rsx! {
    <Button on_click={|| {
        // move ではなく clone: ハンドラは `dispatch` をフレームから借りている。
        let dispatch = dispatch.clone();
        spawn(async move { dispatch.send(Msg::Saved(save().await)); });
    }}>"save"</Button>
}
```

`Dispatch` は `Clone + Send + 'static` で、送信時に再描画を要求します。

## 古い結果

future の実行中に deps が変わっても、古い future は止まりません。ただし届いた結果は捨てられます。書き込まれるのは最新のものだけです。古いレスポンスが新しいものを上書きすることはありません。

## ネイティブと Web

ネイティブでは future ごとに専用のスレッドで走らせ、Web では`wasm_bindgen_futures::spawn_local` で走らせます。違いはそれだけで、自分のコードには出てきません。この作りは HTTP のような、ほとんど待っているだけのfuture に合わせてあります。CPU を使う仕事は、future の中で自分で立てたスレッドでやってください。

## 動かして見る

[fetch](/ja/examples/fetch) はこのページを 1 画面にしたものです。[patch](/ja/examples/patch) はシェーダのコンパイルを `<Suspense>` で包みます。仕組みは[docs/ARCHITECTURE.md](https://github.com/fand/egui-reactor/blob/main/docs/ARCHITECTURE.md#58-suspense)の 4 章と 5.8 節にあります（英語）。
