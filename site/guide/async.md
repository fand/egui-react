---
title: Async
---

# Async

## `use_future`

```rust
let response = use_future(cx, (url, attempt), || {
    // Built here, where `url` is still borrowable; the future itself owns
    // the request and is `'static`.
    let request = ehttp::Request::get(url);
    async move { ehttp::fetch_async(request).await }
});
```

`use_future(cx, deps, f)` returns `&Poll<T>`. `f` is called in place — it can
read locals and `State` guards to build the future — and the future it returns
is `'static`, so it owns what it needs. When the hash of `deps` changes, the
future is rebuilt and started again.

The result type must be `Send + 'static`. Only the future itself differs per
platform, and you never write that difference down.

## The `let`-`else` pattern

There is no `throw` in Rust, so a component that needs a value simply leaves
when it does not have one yet:

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

`Poll` comes from the prelude, so no extra import is needed. Drawing nothing
while pending is the normal shape: the waiting state belongs to the boundary
above, not to every leaf.

## `<Suspense>`

```rust
rsx! {
    <Suspense fallback={view(|cx| { cx.ui().spinner(); })}>
        <Response url={url.as_str()} attempt={*attempt}/>
    </Suspense>
}
```

`<Suspense>` draws `fallback` instead of its children while even one
`use_future` below it is pending. It counts pending futures rather than
catching an exception, and the children are still drawn — offscreen and
invisible — so their hooks run and their futures start and finish. The switch
back happens inside the same frame, so you never see half-drawn children or a
one-frame gap.

Two differences from React worth knowing: `use_effect` in suspended children
*does* run (nothing is "uncommitted" here), and there is no `SuspenseList` or
`useTransition`.

## `spawn` and `Dispatch`

`use_future` is for a value a component is waiting on. For fire-and-forget work
that reports back later, use `spawn` with a `Dispatch`:

```rust
let (state, dispatch) = use_reducer(cx, reduce, State::default);

rsx! {
    <Button on_click={|| {
        // Cloned, not moved: the handler borrows `dispatch` from the pass,
        // and the future has to own its own copy.
        let dispatch = dispatch.clone();
        spawn(async move { dispatch.send(Msg::Saved(save().await)); });
    }}>"save"</Button>
}
```

`Dispatch` is `Clone + Send + 'static` and calls `request_repaint` when it
sends, which is what makes the screen change when the result lands.

## Stale results

If deps change while a future is still running, the old future is not stopped —
it cannot be — but its result is dropped when it arrives, and only the newest
generation is written. You never see an older response overwrite a newer one.

## Native and web

Native runs each future on a thread of its own with `pollster::block_on`; the
web runs it on `wasm_bindgen_futures::spawn_local`, the browser's own event
loop. That is the entire platform difference, and it never shows up in the
types you write. Futures that mostly wait — HTTP, file IO — are what this is
sized for; do CPU-heavy work on a thread you start inside the future.

## See it running

[fetch](/examples/fetch) is the whole of this page in one screen.
[patch](/examples/patch) uses `<Suspense>` around shader compilation.

The mechanism — generation tags, the pending counter, why the switch costs no
frame — is in
[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#58-suspense)
sections 4 and 5.8.
