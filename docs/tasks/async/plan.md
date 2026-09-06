# Plan: async

> `egui_taffy` below is historical. It was replaced in 2026-09 by egui-react's
> own layout engine over taffy (`crates/egui-react/src/engine.rs`, ARCHITECTURE
> section 6), which ports its measure function and node rules, so the layout
> behaviour described here still holds unless ARCHITECTURE says otherwise.

The task definition is in [task.md](task.md). The design rationale is in [docs/ARCHITECTURE.md](../../ARCHITECTURE.md). This document sets the implementation steps and how to verify them. If we decide to deviate from this during implementation, update this document, and update ARCHITECTURE.md too if the change matters for the design.

## 0. Overview

One phase (Phase 6) in one PR. Split into at least 2 commits (`use_future` / `Suspense` + example). We touch `egui-react` (one hook, `spawn`, the counter in `Store`), `egui-react-elements` (`Suspense`), `examples/fetch`, CI, and docs. `Slot` / `Cx` / the macros / app are not touched (if that becomes necessary, write it in section 9).

Added dependencies (pinned in `[workspace.dependencies]`).

| crate | Use | Where |
|---|---|---|
| pollster | native executor (`block_on`) | egui-react (`cfg(not(target_arch = "wasm32"))`) |
| ehttp 0.7 (feature `native-async`) | HTTP client for the fetch example (native = ureq, wasm = fetch API). `fetch_async` needs `native-async` on native | examples/fetch |

`wasm-bindgen-futures` is already in the workspace; add it to the wasm dependencies of `egui-react`.

## 1. `use_future` and `spawn` (`crates/egui-react/src/future.rs`)

### 1.1 API

```rust
use std::task::Poll;

/// Accepts a Send future on native and a non-Send future on wasm under the same name.
#[cfg(not(target_arch = "wasm32"))]
pub trait SpawnFuture<T>: Future<Output = T> + Send + 'static {}
#[cfg(not(target_arch = "wasm32"))]
impl<T, F: Future<Output = T> + Send + 'static> SpawnFuture<T> for F {}

#[cfg(target_arch = "wasm32")]
pub trait SpawnFuture<T>: Future<Output = T> + 'static {}
#[cfg(target_arch = "wasm32")]
impl<T, F: Future<Output = T> + 'static> SpawnFuture<T> for F {}

#[track_caller]
pub fn use_future<'s, D: Hash, T: Send + 'static>(
    cx: &mut Cx<'s, '_>,
    deps: D,
    f: impl FnOnce() -> impl SpawnFuture<T>,   // a generic F in the implementation
) -> &'s Poll<T>

/// Fire and forget. Send the result with `Dispatch`.
pub fn spawn(fut: impl SpawnFuture<()>)
```

- `f` is called on the spot at the call site (same as `use_effect`). It may read locals or a `State` guard to build the future. The future itself is `'static` (it holds cloned values via `move`).
- `T: Send + 'static` is required on wasm too. Only the future side changes its bound per platform, so the user's types stay one kind.
- `spawn` is `task::spawn` exposed as-is. While `use_future` is deps-driven, this is for starting imperatively from a handler (`spawn(async move { dispatch.send(Msg::Saved(api.await)) })`).
- Re-export `use_future` / `spawn` / `SpawnFuture` from `lib.rs`, and add `use_future` / `spawn` and `std::task::Poll` to `prelude`.

### 1.2 What goes in the slots

Two slots are used (same split as `use_reducer`).

- **State slot** (`scope_id.with(location_key)`). The value is `()`. `deps_hash` holds the deps hash, and `Poll<T>` is pushed onto the `memo` `FrozenVec`. One `Pending` at start, then one `Ready(T)` when it arrives. The return value is the downcast of `memo_last()`.
- **Inbox slot** (`id.with("__egui_react_future_inbox")`). The value is a struct holding `Arc<Mutex<Option<(u64, T)>>>` and the current generation `Cell<u64>`.

```rust
struct Inbox<T> {
    cell: Arc<Mutex<Option<(u64, T)>>>,   // (generation, result)
    generation: Cell<u64>,                // generation of the last started future
}
```

### 1.3 Steps on each visit

1. Take both slots with `store.slot(..)` (on the first visit, `Inbox { cell: None, generation: 0 }`).
2. `hash = deps_hash(&deps)`. If `slot.deps_hash() != Some(hash)`, **start**:
   - `generation += 1`, `fut = f()`.
   - `memo_push(Poll::<T>::Pending)`, `set_deps_hash(hash)`.
   - `task::spawn(async move { let v = fut.await; write to inbox; ctx.request_repaint(); })`. `cell` and `ctx` (`store.ctx().clone()`) are moved in. The write "overwrites only if the cell is empty or newer than the stored generation" (1.5).
3. If nothing was started, try to **receive**: if `lock(&cell).take()` is `Some((gen, v))` and `gen == generation`, `memo_push(Poll::Ready(v))`. If the generation differs, drop it.
4. Downcast `memo_last()` to `&Poll<T>`. If `Pending`, call `store.note_pending()` (2.1). Return it.

Right after starting, only return `Pending` and do not try to receive (even if it finished on the spot, it is picked up next frame. `request_repaint` fires, so nothing is missed). On the second pass of the same frame the deps match, so nothing starts and we only try to receive.

### 1.4 `task::spawn`

```rust
mod task {
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn spawn(fut: impl Future<Output = ()> + Send + 'static) {
        if let Err(err) = std::thread::Builder::new()
            .name("egui-react-future".into())
            .spawn(move || pollster::block_on(fut))
        {
            log::error!("egui-react: could not spawn a thread for use_future: {err}");
        }
    }
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn spawn(fut: impl Future<Output = ()> + 'static) {
        wasm_bindgen_futures::spawn_local(fut);
    }
}
```

If thread creation fails, the future is dropped and stays `Pending`. Only log (no panic).

### 1.5 Unmount and stale results

- When the slot drops on unmount, the `Inbox` drops too. The running future holds its own `Arc`, so the write succeeds and one `request_repaint` fires. Nobody reads it next frame, and the last reference to the `Arc` goes away when the future ends.
- If an old future finishes after a deps change, an old generation lands in `cell`. Each visit does `take` and checks the generation, so the receive step drops it.
- If both arrive before a visit in the order "new -> old", the old one would overwrite and lose the new result. To avoid that, the write on the future side "overwrites only if the cell is empty or newer than the stored generation" (do nothing if `Some((old, _)) if old > gen`).

### 1.6 `Cargo.toml`

```toml
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
pollster.workspace = true

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen-futures.workspace = true
```

## 2. `Suspense`

### 2.1 The suspense counter in `Store` (`store.rs`)

Hold `suspense: RefCell<Vec<usize>>`, placed the same way as the `contexts` stack. Clear it in `begin_pass`.

```rust
impl Store {
    /// Enter a boundary. Push a counter at 0.
    pub fn begin_suspense(&self);
    /// Leave a boundary. Return the pushed counter (= number of use_future calls that were Pending inside).
    pub fn end_suspense(&self) -> usize;
    /// Called when use_future returns Pending. Add 1 to the nearest boundary's counter. Do nothing if there is no boundary.
    pub fn note_pending(&self);
}
```

`use_future` calls `note_pending()` right before returning if the value is `Pending` (step 4 in 1.3). The same applies right after starting and when it stays `Pending` after a receive.

### 2.2 The element (`crates/egui-react-elements/src/suspense.rs`)

```rust
/// If even one `use_future` inside is Pending, draw fallback instead of children.
#[component(shares_ui)]
pub fn Suspense(cx: &mut Cx, fallback: impl View, children: impl View) {
    let (store, scope) = (cx.store, cx.scope_id());
    let suspended = use_handle(cx, || true);          // suspended at first (task.md decision)

    if suspended.get() {
        // 1. draw fallback on the visible surface
        fallback.show(cx);
        // 2. draw children into an offscreen invisible Ui (to run hooks)
        let mut ui = egui::Ui::new(
            store.ctx().clone(),
            scope.with("suspense_offscreen"),
            egui::UiBuilder::new()
                .max_rect(OFFSCREEN_RECT)          // off screen, a fixed rect large enough
                .invisible()
                .sizing_pass(),
        );
        store.begin_suspense();
        {
            let mut child = Cx::new(store, &mut ui, scope);
            children.show(&mut child);
        }
        let pending = store.end_suspense();
        // 3. once everything is ready, switch to visible. Redo within the same frame
        if pending == 0 {
            suspended.set(false);
            store.ctx().request_discard("suspense resolved");
        }
    } else {
        store.begin_suspense();
        children.show(cx);
        let pending = store.end_suspense();
        if pending > 0 {
            suspended.set(true);
            store.ctx().request_discard("suspense pending");
        }
    }
}
```

- It is `shares_ui` because `Suspense` does not make its own `Ui` / leaf; it streams children and fallback straight into the parent's surface (whether Ui or Taffy). Placed inside a `<View>`, the children's `<View>` becomes a child in the parent's taffy tree.
- The children's scope is `scope` (`Suspense`'s own `scope_id()`) both when visible and when offscreen. That is the third argument of `Cx::new(store, &mut ui, scope)`. Hook slots get the same Id on both paths, so state and futures survive the switch. The fallback draws into the same `cx`, but `rsx!` element Ids are split by line and column, so they do not clash with the children.
- `Handle::set` calls `request_repaint`, but it is only called at the moment of the switch (followed right away by `request_discard`), so a suspended boundary does not repaint every frame.
- `request_discard` redoes within the same frame, within egui's `max_passes` (runner default 3). If that is used up and the request is refused, the runner calls `request_repaint`, so it settles next frame. Since the initial state is suspended, what shows when refused is the fallback.
- The offscreen `Ui` rect is a fixed value like `Rect::from_min_size(pos2(-1.0e5, -1.0e5), vec2(4096.0, 4096.0))`. This lets egui_taffy see the same size every pass and avoids pointless discards.
- The children's `use_effect` runs while suspended (unlike React). Write this in rustdoc and ARCHITECTURE.md.
- The children's handlers do not fire offscreen (`invisible()` disables interaction too).
- Add `Suspense` to `prelude`.

## 3. Tests

### 3.1 `crates/egui-react/tests/future.rs`

Use `run_app` from `tests/common`. To finish a future, the future does `recv()` on a `std::sync::mpsc` `Receiver` and the test side does `send` (the future is on its own thread, so blocking is fine). Completion happens on another thread, so write a helper `wait_for_repaint(&harness)` that waits with `sleep(10ms)` for up to 2 seconds until `ctx.has_requested_repaint()` is set.

| # | Test | What it checks |
|---|---|---|
| 6-1 | `pending_then_ready_after_repaint` | The first `run` is `Pending` (label "loading"). After `send`, a repaint is requested, and the next `run` shows the `Ready` value |
| 6-2 | `ready_reference_lives_next_to_a_state_guard` | After `let mut n = use_state(..); let r = use_future(..);`, `*n += 1` and a read of `r` can be written together (compiles, and the values are right) |
| 6-3 | `deps_change_restarts_and_stale_result_is_dropped` | Changing deps (`State<u32>`) goes back to `Pending`, and the future starts twice. Finishing the old one later leaves the new one shown. With old first and new second, the final display is still the new one |
| 6-4 | `stale_result_does_not_overwrite_a_newer_one_before_visit` | Finish both futures, then `run` only once. The value of the new generation shows (the overwrite condition in 1.5) |
| 6-5 | `a_two_pass_frame_spawns_once` | `request_discard` on pass 1 so 2 passes run. The number of calls to `f` (`Rc<Cell<usize>>`) is 1 |
| 6-6 | `unmount_while_pending_does_not_panic_and_frees_the_slot` | Remove a child holding `use_future` with an `if`. `store.len()` goes back to the level before removal. Then `send` to finish it; no panic, and a repaint request is set |
| 6-7 | `immediately_ready_future_lands_on_the_next_frame` | An instantly finishing future like `async { 1 }`. The first `run` is `Pending`, and the `run` after waiting for repaint is `Ready(1)` |
| 6-8 | `spawn_with_dispatch_lands` | `spawn(async move { dispatch.send(Msg::Add(n)) })` reaches the reducer, and a repaint is requested |
| 6-9 | `pending_is_counted_by_the_nearest_boundary` | Using `begin_suspense` / `end_suspense` directly, 2 Pending + 1 Ready returns 2. A Pending inside a nested inner boundary is not counted by the outer one |

### 3.2 `crates/egui-react-elements/tests/suspense.rs`

Use the runner in `egui-react-elements/tests/common`, with `max_passes` at 3.

| # | Test | What it checks |
|---|---|---|
| 6-10 | `fallback_while_pending_then_children` | While 2 children are `Pending`, only the fallback label is visible, and the children's labels are not found by `query_by_label`. After both finish and `run`, the children show and the fallback is gone |
| 6-11 | `partial_children_are_never_shown` | With only one finished, `run` still shows the fallback |
| 6-12 | `switch_happens_in_one_frame` | Advance the first frame after completion by one `step` only, and the children are visible (the same-frame switch via `request_discard`). `current_pass_index` is 1 or more |
| 6-13 | `children_state_survives_suspension` | Bump a `use_state` counter in the children with a button, then change deps to suspend again; after it resolves, the counter value is still there |
| 6-14 | `nested_boundary_catches_its_own` | The outer children are Ready and only the inner is Pending. The outer children show, and only the inner shows a fallback |
| 6-15 | `inside_a_view_children_lay_out_in_the_parent_tree` | With `<View direction="row"><Suspense>..<View grow>..</Suspense></View>`, the child `<View>` after resolving fills the parent row's width (confirms the surface is inherited via `shares_ui`) |
| 6-16 | `offscreen_children_do_not_react_to_input` | Clicking the position of a children's button while suspended does not change the children's state |

Compilation of the wasm path is pinned by CI's `cargo check --workspace --target wasm32-unknown-unknown` and the `trunk build` of fetch. Running it is a visual check.

## 4. `examples/fetch`

Copy `examples/counter` and rename it to `fetch` (`Cargo.toml` / `index.html` / `Trunk.toml` / `src/main.rs`). Add `ehttp` (feature `native-async`) to the dependencies.

```rust
#[component]
fn App(cx: &mut Cx) {
    let mut url = use_state(cx, || String::from("https://httpbin.org/get"));
    let mut attempt = use_state(cx, || 0u32);

    rsx! {
        <View direction="column" gap={8} p={12} grow={1.0}>
            <View direction="row" gap={8} align="center">
                <TextEdit grow={1.0} bind={url.bind()} on_submit={|_: String| *attempt += 1}/>
                <Button on_click={|| *attempt += 1}>"fetch"</Button>
            </View>
            <Suspense fallback={|cx: &mut Cx| { cx.ui().spinner(); }}>
                <Response url={url.as_str()} attempt={*attempt}/>
            </Suspense>
        </View>
    }
}

#[component]
fn Response(cx: &mut Cx, url: &str, attempt: u32) {
    let response = use_future(cx, (url, attempt), || {
        let request = ehttp::Request::get(url);
        async move { ehttp::fetch_async(request).await }
    });
    let Poll::Ready(response) = response else { return };
    match response {
        Ok(res) => rsx! {
            <Text strong>{format!("{} {}", res.status, res.status_text)}</Text>
            <ScrollArea grow={1.0}>
                <Text>{res.text().unwrap_or("(binary)").chars().take(2000).collect::<String>()}</Text>
            </ScrollArea>
        },
        Err(err) => rsx! { <Text>{format!("error: {err}")}</Text> },
    }
}
```

- deps are `(&str, u32)`. They are `Hash`, so they can be passed as borrows. `attempt` is mixed in to trigger a refetch of the same URL.
- Leaving via `let-else` makes `Suspense` show the spinner. `Response` does not draw a pending state itself.
- The two `match` arms return different `rsx!` types, so if needed, rewrite it so each arm calls `View::show` (`rsx!{..}.show(cx)`) (check at implementation time whether the `#[component]` tail-expression rewrite accepts `match`).
- Set the `<title>` and `data-bin` in `index.html` to `fetch`.

## 5. CI / README

- `ci.yml`: add `trunk build --release --config examples/fetch/Trunk.toml` after counter.
- Add `fetch` (`use_future` + `Suspense` + `ehttp`) to the examples sentence in README.

## 6. Changes to reflect in ARCHITECTURE.md (known at the start)

- **Table in section 4** Match the `use_future` row to the real signature: `use_future(cx, deps, || async { .. }) -> &Poll<T>`. "Native is thread + `pollster`, wasm is `wasm_bindgen_futures::spawn_local`. `request_repaint` on completion. Rebuilt on deps change, stale results dropped. `Pending` is counted by the nearest `<Suspense>`". Add a row for `spawn(fut)` (not a hook, but in the same table).
- **Add a "`use_future` details" section to section 4** The content of 1.2 to 1.5 (two slots, why `Poll` goes on the `FrozenVec`, generation numbers, no receive right after start, handling after unmount, keeping the platform bound difference inside `SpawnFuture`, why `T: Send` is required on both platforms). Child components are written as `let Poll::Ready(x) = .. else { return };`.
- **Add "5.8 Suspense" to section 5** The content of 2.1 / 2.2 (the counter stack, initial suspended, the offscreen invisible `Ui`, why the same scope Id is used, the same-frame switch via `request_discard` and behavior when refused, the difference that `use_effect` runs, the mapping to React's throw).
- **5.6** Already says "`use_future` completion calls `request_repaint`". No change (confirm the implementation matches).
- **Element list in section 6** Add `Suspense` (`shares_ui`, `fallback: impl View`). Add `Suspense` to the paragraph "why panels and `Row` are `shares_ui`" (to inherit the surface).
- **Section 7** `pollster` (native) and `wasm-bindgen-futures` (wasm) in the dependencies of `egui-react`. `fetch` in examples.
- **Section 8** Add one line: "The async run mechanism is kept inside core's `task::spawn`. iOS / Android use the same thread path as native".
- **Section 11 (decision log)** Add 3 rows.
  - executor: adopt thread + `pollster`, reject requiring tokio. Reason: small dependencies, enough for futures that only wait. Apps that use tokio can use `Handle::current()` inside the future.
  - result representation: adopt `Poll<T>`, reject a custom `Loading / Ready / Error` enum. Reason: it is in std, and errors can be expressed with `T = Result`.
  - how Suspense works: adopt offscreen drawing + counter + `request_discard`, reject unwinding via panic / `catch_unwind`. Reason: Rust has no cheap unwinding, and one let-else line is enough.

## 7. Steps

1. `future.rs` (`SpawnFuture` -> `task::spawn` / `spawn` -> `use_future`), the counter in `store.rs`, re-exports, `Cargo.toml`. Tests 6-1 to 6-9. Make `cargo check --target wasm32-unknown-unknown -p egui-react` pass. Update ARCHITECTURE.md 4 / 7 / 8 / 11. Commit.
2. `suspense.rs` and `prelude`. Tests 6-10 to 6-16. Update ARCHITECTURE.md 5.8 / 6. Commit.
3. `examples/fetch`. Visually check `cargo run -p fetch` and `trunk serve` (only the spinner while fetching, the UI stays responsive, it updates on its own after completion, changing the URL and pressing Enter refetches, the fetch button refetches the same URL). CI and README. Commit.
4. Confirm all CI steps are green.
5. Write the results, the ARCHITECTURE.md changes, and anything dropped (if any) in the PR body.

## 8. Points that may need a decision

- **Inference of `impl FnOnce() -> impl SpawnFuture<T>`** (1.1). If an `async move { .. }` block awaits across a `State` guard or a `&str`, the `Send` check fails to compile on native. If the error message is hard to read, write in rustdoc "only put cloned values in the future" with an example. No help from the macro side.
- **Waiting on threads with kittest** (3.1). If the `wait_for_repaint` sleep loop is flaky in CI, put `tx.send(())` after the write inside the future to tell the test side "written" (if the order of write and `request_repaint` is fixed, `has_requested_repaint` after receiving `send` is deterministic).
- **The offscreen `Ui` and egui_taffy** (2.2). If a `<View>` (egui_taffy) inside a root `Ui` made with `Ui::new` calls `request_discard` every pass, then besides fixing the rect, drop `sizing_pass()`, or switch to hanging the children off the parent `Ui` with `ui.new_child(..)` (takes no space). Using up `max_passes` so the switch falls to the next frame is acceptable.
- **When the `#[component]` tail expression is a `match`** (section 4). If arms with different `rsx!` types cannot be returned, write it so each arm calls `.show(cx)`. Do not fix the macro.
- **The `ehttp` feature**. `fetch_async` on native needs `native-async` (pulls in `async-channel`). To avoid extra dependencies on the wasm side, write it separately under `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`. Check until the trunk build passes.
- **httpbin availability**. The example's default URL is for visual checks, so it is fine if it is down, but write "enter any URL and press Enter" in README.
- **The `Poll` re-export in `prelude`**. If someone reports a clash with their own `Poll`, remove it and write a `use` of `std::task::Poll` in the README example.

## 9. Differences found during implementation

### Step 1

- **1.1** The type of `f` cannot be written as `impl FnOnce() -> impl SpawnFuture<T>` (E0562 "`impl Trait` is not allowed in the return type of `Fn` trait bounds"). As the note in 1.1 said, it became a generic `F: SpawnFuture<T>`. The signature is `use_future<'s, D, T, F>(cx, deps, f: impl FnOnce() -> F) -> &'s Poll<T>`.
- **1.5** The overwrite condition "only if the cell is empty or newer than the stored generation" is written with `Option::is_none_or`. `request_repaint` is always called whether or not the write succeeds (a dropped result only costs one extra repaint, same treatment as unmount in 1.5).
- **1.6** `pollster` is pinned at 1.0.1 in `[workspace.dependencies]` (the plan's "latest at the start"). `wasm-bindgen-futures` uses the existing 0.4.56 as-is.
- **2.1** `begin_suspense` / `end_suspense` / `note_pending` are `pub` methods on `Store` (called from elements). `end_suspense` returns `0` on an empty stack.
- **3.1 tests** `wait_for_repaint` waits for `has_requested_repaint()` at 10ms intervals for up to 2 seconds (we did not fall back to the alternative in plan section 8). The implementation fixes the order of writing first and then `request_repaint`, so once a repaint is observed the result is already in the cell.
- **3.1 test 6-4** There is no way to observe from outside that the second (old) write in the "new -> old" order has finished, so after `wait_for_repaint` we sleep 100ms and then `run` once. If the implementation is correct the old write never happens, so a too-short sleep only lowers detection power and never makes the test flaky.
- **3.1 test 6-7** `Harness::new_ui_state` calls the app once at construction and then loops with `run_ok()` until stable. Starting an instantly finishing future there means "first visit is `Pending`" cannot be observed, so the test enables the hook call itself later via a flag and sees `Pending` in one `step()` frame.
- **3.1 test 6-8** `spawn` is called from a handler (a button). After the click, advance one frame with `step()` instead of `run()` (`run()` keeps going until the message arrives and is fully applied, so the repaint request cannot be observed).
- **Section 6 ARCHITECTURE.md** One sentence about where the suspense counter in `Store` (2.1) lives was put into the `use_future` details in section 4, because until 5.8 exists there is nothing to point to.

### Step 2

- **2.2** `Rect::from_min_size` is not a const fn, so the offscreen rect is written as a `Rect { min, max }` literal (`egui::pos2` is a const fn). The values are as planned: from `(-1.0e5, -1.0e5)`, 4096x4096.
- **2.2** The plan's `let pending = store.end_suspense(); if pending == 0 { .. }` was folded into `if store.end_suspense() == 0 { .. }`. Same content.
- **2.2 / 3.2 tests** egui creates accessibility nodes for widgets regardless of visibility, so the children are visible to `egui_kittest`'s `query_by_label` even while suspended (coordinates are off screen at `-100000`). "The children's labels are not found" cannot be tested, so the test side has `shows(harness, label)` (node exists and its rect is on screen), and the visibility checks of 6-10 / 6-11 / 6-12 / 6-13 / 6-14 / 6-15 / 6-16 use it. The switch via `request_discard` and the disabling of interaction via `invisible()` still work (6-12 / 6-16 are green). This limit is written in ARCHITECTURE.md 5.8. egui has no entry point for "do not create accessibility nodes for an invisible `Ui`", and `Context::disable_accesskit` only takes effect at the start of a pass, so there is no workaround.
- **3.2 test 6-16** The children are offscreen while suspended, so kittest's `Node::click` (clicks the node center) cannot get a position. The test remembers the button rect from when it was visible and clicks the same coordinates directly with `Harness::hover_at` / `drag_at` / `drop_at`. It first asserts that a click with the same helper goes through when visible, so this is not a no-op.
- **3.2 tests** The concern in plan section 8 (the offscreen `Ui` and egui_taffy calling `request_discard` every pass) did not happen. No need to drop `sizing_pass()` or switch to `ui.new_child`. 6-15 also passes with `shares_ui` as-is: inside `<View direction="row" w={300}>`, the `<Suspense>` child `<Text grow={1.0}>` fills the row and the neighbor `"end"` sits at the right edge (left = 285).
- **3.1 test fixes (flake fix for step 1)** `spawn_with_dispatch_lands` and `ready_reference_lives_next_to_a_state_guard` turned out not to be able to rely on `wait_for_repaint`. In the former, egui itself requests a repaint after pointer input; in the latter, the body writes state every pass; so in both, `has_requested_repaint` gets set regardless of the result. They were changed to repeat `run` / `step` until the display changes (2 second timeout). Both test binaries were run 80 times each and stayed green.

### Step 3

- **4** The `match` in `Response` was put inside `rsx! { match response { .. } }`, not "call `.show(cx)` in each arm" (the alternative in plan section 8). `rsx!` accepts `match` as a custom node (ARCHITECTURE.md 3.3), so this is shorter, and the `#[component]` tail-expression rewrite is only needed once. The macro was not touched.
- **4** The body preview (the first 2000 characters of `response.text()`) was split into `body_preview(&ehttp::Response) -> String` instead of an expression inside `rsx!`. Chaining methods inside `{expr}` makes the inference of `Text`'s `children: impl Into<WidgetText>` hard to read.
- **4** `ehttp` is pinned at 0.7.1 in `[workspace.dependencies]`. Its default features are empty, so only the native side pulls it with `features = ["native-async"]`, and the wasm side pulls it plain.
- **4** The `T` of `use_future` is `Result<ehttp::Response, String>`. The future returned by `ehttp::fetch_async` satisfies `Send` on native as-is, so the concern in plan section 8 ("the `Send` check fails to compile") did not happen.
- **5** Added one sentence about `use_future` / `<Suspense>` to Usage in README, and `fetch` to the examples list. CI got the `trunk build` of fetch after counter (with a comment saying it is the only check that the wasm `spawn_local` path actually links).
- **Visual check** `cargo run -p fetch` starts and keeps running, with no panic in the log. `trunk build --release --config examples/fetch/Trunk.toml` succeeds. Visual checks that involve clicking (only the spinner is visible / updates on its own after completion / refetch) are done on the coordinator side.

### Carry-over to later PRs

- **Accessibility nodes of the children while suspended** (step 2). egui creates accesskit nodes for widgets regardless of visibility, so the children while suspended are visible to screen readers and `egui_kittest` as "nodes at off-screen coordinates". They do not show on screen, but they are read aloud. egui has no entry point for this, so either file an upstream issue or find a way for `Suspense` to remove the accesskit nodes itself.
- **Extract the inside of `use_future` as `AsyncSlot`** (out of scope in task.md). When adding `use_query` / `use_action` / `use_debounced` / `use_stream`, share the handling of the generation-tagged inbox and the two slots.
- **Future cancellation**. On deps change and unmount we only drop the result; the running work runs to completion. Not a problem for short IO like `ehttp`, but long work needs an `AbortHandle` equivalent.
- **An executor override hook** (something like `Store::set_spawner`). Apps that use tokio call `Handle::current()` inside the future for now. When someone asks.
- **`use_effect` inside `<Suspense>`**. It runs while suspended (React does not run it). Stopping it would mean "queue effects while suspended and flush them when visible", but that loses the benefit of `use_effect` running on the spot.
- **Carry-over from PR2**: `use_persisted_reducer`, wasm automated tests for `use_persisted`, the dirty flag for `App::save`.

## 10. Material for the PR body

### Results per step

| Step | What was done |
|---|---|
| 1 (`use_future`) | `future.rs` (cfg switch of `SpawnFuture<T>`, `task::spawn`, `spawn`, `use_future`), the 3 suspense counter methods on `Store`, re-exports in `lib.rs` / `prelude` (including `Poll`), the `pollster` (native) / `wasm-bindgen-futures` (wasm) dependencies. Tests 6-1 to 6-9. |
| 2 (`Suspense`) | `suspense.rs` in `egui-react-elements` (`#[component(shares_ui)]`, initial suspended, the offscreen invisible `Ui`, `begin_suspense` / `end_suspense`, the same-frame switch via `request_discard`) and its addition to `prelude`. Tests 6-10 to 6-16. |
| 3 (example) | `examples/fetch` (`ehttp::fetch_async` + `<Suspense>` + refetch button, shared by native / wasm), `trunk build (fetch)` in CI, one sentence in README examples and Usage. |

### Changes to ARCHITECTURE.md

- **Table in section 4** Updated the `use_future` row to the real signature and behavior, and added a `spawn(fut)` row.
- **Added "`use_future` details" to section 4** The platform difference kept inside `SpawnFuture`, native thread + `pollster`, the two slots and the `Poll` pushed onto the `FrozenVec`, generation numbers and how stale results are dropped, why there is no receive right after start, handling after unmount, `note_pending`, how the child writes `let`-`else`.
- **Added 5.8 Suspense** The counter stack, why it starts suspended, the offscreen invisible `Ui` (and the limit that accessibility nodes remain), why both paths use the same scope Id, the same-frame switch via `request_discard` and behavior when refused, `shares_ui`, the difference from React that `use_effect` runs.
- **Section 6** Added a `Suspense` row to the element list and added `Suspense` to the `shares_ui` paragraph.
- **Section 7** `pollster` (native) and `wasm-bindgen-futures` (wasm) in the dependencies of `egui-react`. `fetch` in examples.
- **Section 8** The async run mechanism is kept inside core's `task::spawn`, and iOS / Android use the same thread path as native.
- **Section 11 (decision log)** 3 rows added. Executor (thread + `pollster` / requiring tokio rejected), result representation (`Poll<T>` / custom enum rejected), how Suspense works (offscreen + counter + `request_discard` / unwinding via panic rejected).

### Dropped

- None. Everything in the task.md scope is in. The things kept out of scope (tokio integration, cancellation, `use_query` / `use_action`, Error boundary, `SuspenseList`) stay in task.md's "Out of scope", and are listed with reasons under "Carry-over to later PRs" in section 9.
