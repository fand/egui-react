# Task: async (PR3 = Phase 6)

## Goal

Add the async base on top of core (PR2). The `use_future` hook lets you write "work that spans frames" as one `async` block inside the component body, and the UI repaints on its own when the result arrives. Also add a `<Suspense fallback=..>` element, the counterpart of React's `<Suspense>`, so all pending work inside a boundary is replaced by one fallback. Add a `fetch` example that runs on both native and wasm, to confirm the same code works on both (the difference in the run mechanism stays inside core).

PR2 made `Dispatch` `Send + 'static`. This PR adds, on top of that, a path that "writes the result back to a slot on completion and calls `request_repaint`", plus a suspense counter in `Store`. `Cx` and the macros are not touched.

## Scope

### In scope

- `use_future(cx, deps, || async { .. }) -> &Poll<T>` (core, `future.rs`).
  - Each time the deps hash changes, rebuild and start the future. The comparison uses the same Hash as `use_effect` / `use_memo`.
  - A started future runs on one thread with `pollster::block_on` on native, and with `wasm_bindgen_futures::spawn_local` on wasm.
  - On completion, write the result into the slot's shared cell and call `request_repaint`. On the next visit the hook reads the cell and switches to `Poll::Ready(T)`.
  - Drop stale results that arrive after deps changed (generation number). Do not stop a running future (we cannot).
  - The return value is `&'s Poll<T>`, same as `use_memo`. It lives alongside a `State` guard.
  - Do not start twice on the second pass of the same frame. Do not panic on a result that arrives after unmount.
  - When returning `Pending`, add 1 to the suspense counter in `Store` (the one of the nearest `<Suspense>`).
- `egui_reactor::spawn(future)`: expose core's `task::spawn`. Combined with `Dispatch`, this lets you write "start imperatively and send the result" (mutation, optimistic update).
- Keep the native / wasm run mechanism difference inside the `SpawnFuture<T>` trait (cfg-switched bound) and `task::spawn`.
- A stack of suspense counters in `Store` (same shape as the `provide_context` stack).
- `<Suspense fallback={..}>children</Suspense>` (elements, `suspense.rs`).
  - If even one `use_future` inside the boundary is `Pending`, draw `fallback` instead of children. Once all are `Ready`, draw children.
  - The switch happens within the same frame via `request_discard`, so half-drawn children are never visible.
  - While suspended, children keep drawing into an offscreen invisible `Ui` (hooks run, futures start and finish).
  - With nesting, the nearest boundary catches it.
  - `fallback` is `impl View` (`rsx!{..}` / closure / `&str`).
- Child components are written as `let Poll::Ready(x) = use_future(..) else { return };`. This replaces React's throw.
- `examples/fetch`: GET a URL with `ehttp::fetch_async`. A `Response` component inside `<Suspense>` waits with let-else, and the fallback is a spinner. After completion it shows the status and the start of the body. Runs on both native and trunk. A "refetch" button bumps a counter mixed into deps.
- kittest tests (table in plan.md).
- CI: add fetch to `trunk build` (confirms the wasm `spawn_local` path actually links).
- Add fetch to the examples list in README. Update ARCHITECTURE.md section 4 (the `use_future` row and the "details" section), section 5 (Suspense section), section 6 (element list), section 7, section 8, and section 11 (decision log).

### Out of scope

- tokio integration. The native default is thread + `pollster`; apps that want tokio call `Handle::current().spawn(..).await` inside the future. An executor override hook (something like `Store::set_spawner`) will be added when someone asks for it.
- Future cancellation (an `AbortHandle` equivalent). On deps change and unmount we only drop the result; the running work runs to completion.
- `use_query` (keyed cache, sharing across components, stale-while-revalidate), `use_action` (imperative start + pending), `use_debounced`, `use_stream`. Next PR (async-2). Extracting the inside of `use_future` as `AsyncSlot` also happens then.
- State representations other than `Poll` (an enum like `Loading` / `Error`). Errors are expressed as `T = Result<..>`.
- Error boundary. Errors are values, so the child does `match`.
- Stopping the children's `use_effect` while `Suspense` is suspended (React does not commit, so they do not run). In egui-reactor they run. Write this in the docs.
- `SuspenseList`, `useTransition` equivalents.
- A `Spinner` element. The fetch example calls `cx.ui().spinner()` in a closure.
- Carry-overs from PR2 (`use_persisted_reducer`, wasm automated tests for `use_persisted`, the dirty flag for `App::save`). Separate PR.
- Android / iOS (PR4). English docs, API review, crates.io publish (Phase 8).

## Deliverables

- `crates/egui-reactor/src/future.rs` (`use_future`, `SpawnFuture`, `spawn`), the suspense counter in `store.rs`, re-exports in `lib.rs` / `prelude` (`use_future`, `spawn`, `std::task::Poll`).
- `crates/egui-reactor/tests/future.rs` (kittest).
- `crates/egui-reactor-elements/src/suspense.rs` (`Suspense`) and its addition to `prelude`, `crates/egui-reactor-elements/tests/suspense.rs`.
- `examples/fetch` (`Cargo.toml` / `src/main.rs` / `index.html` / `Trunk.toml`).
- Add the trunk build of fetch to `.github/workflows/ci.yml`.
- Update the examples sentence in README.
- Update `docs/ARCHITECTURE.md` (changes known at the start are in [plan.md](plan.md) section 6; changes found during implementation are added as they come up).

## Done criteria

- All tests in the [plan.md](plan.md) test table are green. All tests up to PR2 still pass as-is.
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`, `cargo check --workspace --target wasm32-unknown-unknown`, `trunk build` (counter and fetch) pass in CI.
- `cargo run -p fetch` fetches the URL. While fetching, only the spinner is visible (a half-drawn body never shows even for a moment), the UI does not freeze, and after completion the screen updates without moving the mouse (visual check).
- `trunk serve --config examples/fetch/Trunk.toml` shows the same behavior in the browser (visual check).
- ARCHITECTURE.md matches the implementation. List the changes in the PR body.

## Decisions (assumptions at the start)

- The native executor is one thread + `pollster::block_on`. One thread per future, no pool. Reason: minimal dependencies, and enough for "just waiting" futures like `ehttp` / file IO. CPU-heavy work spawns its own thread inside the future.
- The future bound is `Future<Output = T> + Send + 'static` on native and `Future<Output = T> + 'static` on wasm. `T` is `Send + 'static` on both (to return a `JsValue` on wasm, convert it to a `Send` type inside the future). This difference stays inside the `SpawnFuture<T>` trait and does not show up in user-written types.
- Results are passed via `Arc<Mutex<Option<(u64, T)>>>` (with generation). Same shape as the `Dispatch` queue; poison is ignored.
- The return value is `&'s Poll<T>`. Values go on the same `FrozenVec` as `use_memo` (one `Pending`, then one `Ready` when it arrives). So a reference handed out earlier in the same pass may point at an old value, the sweep at the end of the pass keeps only the latest one (the existing `prune_memo`).
- Rebuilding the future after a deps change happens at visit time. The old running future runs to completion, but its result is dropped due to a generation mismatch. On unmount the slot disappears, so nobody reads the arriving result and only one extra `request_repaint` fires.
- `Suspense` lives in elements (a "wraps children" element like `Collapsing`). Core only gets the three suspense counter methods.
- The initial state of `Suspense` is suspended (on first draw, render offscreen first; if nothing is pending, discard and switch to visible). Reason: when `max_passes` is used up and the discard is refused, we fall on the side of showing the fallback rather than half-drawn children.
- Offscreen drawing uses `egui::Ui::new(ctx, id, UiBuilder::new().max_rect(a large rect off screen).invisible().sizing_pass())`. `invisible()` disables both drawing and interaction (checked with egui 0.36). It takes no space from the parent `Ui`.
- In both the suspended and visible paths, children draw under the same scope Id (`Suspense`'s own `scope_id()`). Hook slots are shared by both paths, and that is why state and futures survive the switch.
- The example's HTTP client is `ehttp` (ureq on native, the fetch API on wasm. `fetch_async` returns a future. Native needs the `native-async` feature). `reqwest` needs tokio, so it is not used.
- Added dependencies: `pollster` (core, native only), `ehttp` (examples/fetch). Pin the latest version at the start in `[workspace.dependencies]`.
