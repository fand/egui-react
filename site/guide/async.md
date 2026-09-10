---
title: Async
---

# Async

## `use_future`

```rust
let response = use_future(cx, (url, attempt), || {
    // Built here, where `url` is still borrowable. The future owns the
    // request and is `'static`.
    let request = ehttp::Request::get(url);
    async move { ehttp::fetch_async(request).await }
});
```

`use_future(cx, deps, f)` returns `&Poll<T>`. `f` runs in place, so it can
read locals and `State` guards. The future it returns must be `'static`. When
the hash of `deps` changes, the future is rebuilt and started again. `T` must
be `Send + 'static`.

## The `let`-`else` pattern

There is no `throw` in Rust. A component that has no value yet just returns:

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

`Poll` is in the prelude. Drawing nothing while pending is normal. The
waiting state belongs to the boundary above.

## `<Suspense>`

```rust
rsx! {
    <Suspense fallback={view(|cx| { cx.ui().spinner(); })}>
        <Response url={url.as_str()} attempt={*attempt}/>
    </Suspense>
}
```

`<Suspense>` draws `fallback` while any `use_future` below it is pending. The
children are still drawn, offscreen, so their futures start and finish. The
switch back happens in the same frame, so there is no gap.

Two differences from React: `use_effect` in suspended children does run, and
there is no `SuspenseList` or `useTransition`.

## `spawn` and `Dispatch`

`use_future` is for a value a component waits on. For fire-and-forget work
that reports back later, use `spawn` with a `Dispatch`:

```rust
let (state, dispatch) = use_reducer(cx, reduce, State::default);

rsx! {
    <Button on_click={|| {
        // Cloned, not moved: the handler borrows `dispatch` from the frame.
        let dispatch = dispatch.clone();
        spawn(async move { dispatch.send(Msg::Saved(save().await)); });
    }}>"save"</Button>
}
```

`Dispatch` is `Clone + Send + 'static` and asks for a repaint when it sends.

## Stale results

If deps change while a future is running, the old future is not stopped, but
its result is dropped when it arrives. Only the newest one is written. An old
response never overwrites a new one.

## Native and web

Native runs each future on its own thread. The web runs it on
`wasm_bindgen_futures::spawn_local`. That is the whole difference, and it
never shows in your code. This is sized for futures that mostly wait, like
HTTP. Do CPU-heavy work on a thread you start inside the future.

## See it running

[fetch](/examples/fetch) is this page in one screen. [patch](/examples/patch)
uses `<Suspense>` around shader compilation. The mechanism is in
[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#58-suspense)
sections 4 and 5.8.
