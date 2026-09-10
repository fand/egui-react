# Plan: spike

> `egui_taffy` below is historical. It was replaced in 2026-09 by egui-react's
> own layout engine over taffy (`crates/egui-react/src/engine.rs`, ARCHITECTURE
> section 6), which ports its measure function and node rules, so the layout
> behaviour described here still holds unless ARCHITECTURE says otherwise.

The task definition is in [task.md](task.md). The design rationale is in [docs/ARCHITECTURE.md](../../ARCHITECTURE.md). This document sets the implementation steps and how to verify them. If you decide to depart from this plan during implementation, update this document.

## 1. Workspace

```
Cargo.toml                      workspace. members = crates/*, examples/*
rust-toolchain.toml             channel = stable (at or above egui_taffy's MSRV), targets = ["wasm32-unknown-unknown"]
LICENSE-MIT / LICENSE-APACHE
README.md                       one-paragraph description and a link to ARCHITECTURE.md
.github/workflows/ci.yml
crates/egui-react/              core (the substance of this PR)
crates/egui-react-macros/       proc-macro crate. lib.rs is empty
crates/egui-react-elements/     empty
crates/egui-react-app/          empty
examples/spike/                 eframe binary. Shows Counter and Dialog
```

Pin the following in `[workspace.dependencies]`.

| crate | version |
|---|---|
| egui / eframe / egui_kittest | 0.36 |
| egui_taffy | 0.14 |
| rstml | 0.13 (only declared as a dependency of macros. Unused) |
| elsa | latest |
| log | latest |

CI (`ci.yml`, ubuntu-latest):

1. `cargo fmt --all --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. `cargo check -p egui-react --target wasm32-unknown-unknown`

egui_kittest does not enable the `wgpu` / `snapshot` features (run headless).

## 2. core implementation (`crates/egui-react/src/`)

### 2.1 `store.rs`

```rust
pub struct Store {
    slots: elsa::FrozenMap<egui::Id, Box<Slot>>,   // can insert with &self, and &Slot is stable
    pass: Cell<u64>,
    collisions: RefCell<Vec<Collision>>,
}
struct Slot {
    value: RefCell<Box<dyn Any>>,
    last_visited: Cell<u64>,
    cleanup: RefCell<Option<Box<dyn FnOnce()>>>,   // for use_effect
    deps_hash: Cell<Option<u64>>,                  // for use_effect
    location: &'static Location<'static>,          // for collision reports
}
pub struct Collision { pub id: egui::Id, pub location: &'static Location<'static> }
```

- `begin_pass(&mut self)`: `pass += 1`.
- `slot(&self, id, location, init) -> &Slot`: return it if it exists. If `last_visited == pass`, record it in `collisions` as a collision and `log::warn!`. If it does not exist, create with `init` and insert. In both cases update `last_visited = pass`.
- `end_pass(&mut self)`: list slots with `last_visited < pass`, run their cleanup, and remove them (get `&mut HashMap` via `FrozenMap::as_mut()`). Return `collisions` or make them retrievable.
- `end_pass` is not called while a guard is alive (the caller's responsibility. The runner calls it after leaving the component body).

### 2.2 `cx.rs`

```rust
pub struct Cx<'s, 'u> {
    pub store: &'s Store,
    pub ui: &'u mut egui::Ui,
    scope: egui::Id,
    contexts: &'s ContextStack,   // see 2.5. 's is fine (Store owns it)
}
impl<'s, 'u> Cx<'s, 'u> {
    pub fn new(store: &'s Store, ui: &'u mut Ui, scope: Id) -> Self;
    pub fn scope_id(&self) -> Id;
    /// Component boundary. Deepens scope and also does ui.push_id
    pub fn scope<R>(&mut self, source: impl Hash, f: impl FnOnce(&mut Cx<'s, '_>) -> R) -> R;
    /// Custom hook boundary. Only deepens scope (no ui.push_id)
    pub fn hook_scope<R>(&mut self, loc: &'static Location<'static>, f: impl FnOnce(&mut Cx<'s, '_>) -> R) -> R;
    pub fn ctx(&self) -> &egui::Context;
}
```

Inside an egui container closure (`ui.vertical(|ui| ..)`), copy `store` and `scope` and rebuild with `Cx::new(store, ui, scope)`. Rust 2021 closures capture struct fields one by one, so `cx.ui.vertical(|ui| { let mut cx = Cx::new(store, ui, scope); .. })` does not conflict with the mutable borrow of `cx.ui`. The macro is expected to generate this shape; in spike we write it by hand.

### 2.3 `state.rs`

```rust
pub struct State<'s, T> {
    inner: RefMut<'s, T>,      // RefMut::map(slot.value.borrow_mut(), downcast)
    dirty: bool,
    ctx: egui::Context,
}
impl Deref / DerefMut          // DerefMut sets dirty = true
impl Drop                      // if dirty, ctx.request_repaint()
impl State { pub fn into_handle(self) -> Handle<'s, T>; }   // consume the guard and turn it into a Handle

#[derive(Clone, Copy)]
pub struct Handle<'s, T> { slot: &'s Slot, ctx: &'s egui::Context?, _t: PhantomData<T> }
impl Handle { get()(T: Clone), set(v), update(|&mut T|), with(|&T| -> R) }   // each call borrows only for a moment. set/update request_repaint
```

For `Handle` to be `Copy`, it cannot own an `egui::Context` by value. Have `Store` hold one clone of `Context` and let `Handle` reference `&'s Context` (updated via `begin_pass(&mut self, ctx: &Context)`).

`State::handle(&self)` is not provided. Using a `Handle` while the guard is alive panics on a double borrow of the same `RefCell`, so always consume the guard with `into_handle`. Add this point to ARCHITECTURE.md 3.5.

### 2.4 `hooks.rs`

```rust
#[track_caller]
pub fn use_state<'s, T: 'static>(cx: &mut Cx<'s, '_>, init: impl FnOnce() -> T) -> State<'s, T>;
#[track_caller]
pub fn use_handle<'s, T: 'static>(cx: &mut Cx<'s, '_>, init: impl FnOnce() -> T) -> Handle<'s, T>;
#[track_caller]
pub fn use_effect<D: Hash, C: IntoCleanup>(cx: &mut Cx, deps: D, f: impl FnOnce() -> C);
```

- The Id is `cx.scope_id().with(Location::caller())`. `Location` hashes `(file, line, column)`.
- `use_effect`: hash deps with `std::hash::DefaultHasher` and compare with the slot's `deps_hash`. If different (or first time), run the existing cleanup, then call `f` **on the spot**, and store the returned cleanup. Implement `IntoCleanup` for `()` and `F: FnOnce() + 'static`.

### 2.5 `context.rs` (minimal)

```rust
pub fn provide_context<'s, T: 'static>(cx: &mut Cx<'s, '_>, value: Handle<'s, T>, children: impl FnOnce(&mut Cx<'s, '_>));
pub fn use_context<'s, T: 'static>(cx: &Cx<'s, '_>) -> Option<Handle<'s, T>>;
```

`Store` holds a stack like `ContextStack: RefCell<Vec<(TypeId, *const ())>>`, and `provide_context` does push -> children -> pop. `Handle<'s, T>` is `Copy` and `'s`, so putting `Handle` by value into `Vec<(TypeId, Box<dyn Any>)>` needs no unsafe. Always pop after `children` (panic safety is not considered in spike).

### 2.6 `events.rs` (hand-written version of macro output)

```rust
pub trait Handler<A> { fn call(self, a: A); }
impl<F: FnOnce()> Handler<()> for F;         // check how the argument-dropping and argument-taking versions can coexist
impl<F: FnOnce(A), A> Handler<A> for F;      // if this impl conflicts with the one above, split into Handler0 / Handler1 and let the macro pick by whether there is an argument

pub struct Emitter<'e, E> { sink: &'e RefCell<&'e mut dyn FnMut(E)> }
impl<E> Emitter<'_, E> { pub fn emit(&self, e: E); }
```

The two blanket impls of `Handler` are very likely to conflict under coherence. If they conflict, the policy is that the macro looks at the closure's argument count syntactically and picks `call0` / `call1`, and this is added to ARCHITECTURE.md 3.6. The spike settles which one holds.

### 2.7 `lib.rs`

Re-export the above. Put a `prelude` module.

## 3. Hand-written expanded code (`crates/egui-react/tests/common/`)

Write the code the macros should generate by hand, in the following shape. Both tests and examples use it, so put it in `tests/common/mod.rs` and `examples/spike/src/components.rs` (duplication is allowed. It goes away when macros arrive).

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

### Dialog (2 callback props)

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

// Expanded form on the parent side
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

### Custom hook (expanded form of `#[hook]`)

```rust
#[track_caller]
pub fn use_counter<'s>(cx: &mut Cx<'s, '_>) -> State<'s, i32> {
    let loc = Location::caller();
    cx.hook_scope(loc, |cx| use_state(cx, || 0))
}
```

## 4. Tests (`crates/egui-react/tests/`)

Write each test in the form `egui_kittest::Harness::new_ui_state(|ui, store: &mut Store| { .. }, Store::new())`. Inside the closure: `store.begin_pass(ui.ctx())` -> draw the component with `Cx::new(&*store, ui, Id::new("root"))` -> call `store.end_pass()` after the guards drop. Bundle this sequence into `run_app(ui, store, |cx| ..)` in `tests/common/run.rs`.

| # | Item in ARCHITECTURE.md section 10 | Test file | What to check |
|---|---|---|---|
| 1 | The guard lives only during the body, and sibling handlers can borrow the same state `&mut` in turn | `sibling_handlers.rs` | Click Counter's `+` and `-`, and the label goes 1 -> 2 -> 1 |
| 2 | A guard can be returned through `#[hook]` | `custom_hook.rs` | Call `use_counter` twice in the same component, and each increments independently |
| 3 | Fused closure and `Handler` | `fused_events.rs` | `open` becomes false for each of Dialog's OK / Cancel. The main goal is that both closures capture the same state and it compiles |
| 4 | `use_context`'s `Handle` coexists with the parent's guard | `context_handle.rs` | The parent holds theme with `use_handle` and calls `provide_context`, the child calls `use_context().set()`, and the parent reads the new value with `.get()` after children. At the same time the parent holds a `State` guard on another slot |
| 5 | In multi-pass, handlers fire only once and effects do not rerun | `multi_pass.rs` | (a) a manual version that calls `ctx.request_discard()` in pass 1, (b) a version where a click changes label width inside egui_taffy flex. In both, count is only +1 and effect run count is 1. Confirm the test actually ran 2 passes by the root closure's call count. Set `ctx.options_mut(|o| o.max_passes = 2)` |
| 6 | Id collision detection | `collision.rs` | (a) call a helper without `hook_scope` twice, (b) call `use_state` inside `for` without a key. In both, `store.collisions()` is not empty. As a control, the version wrapped in `cx.scope(i, ..)` is empty |
| 7 | sweep drops state and runs cleanup | `unmount.rs` | The child in `if show { child }` has `use_effect((), || { log.push("mount"); move || log.push("cleanup") })` and `use_state`. Set show to true -> false -> true; the log is `[mount, cleanup, mount]` and state returns to its initial value. The log is `Arc<Mutex<Vec<&str>>>` |
| 8 | Create a new `Cx` inside an egui container closure and hooks work | `nested_ui.rs` | Draw a child with `use_state` inside `ui.vertical`; outer and inner state are independent and persist across frames |
| 9 | repaint policy | `repaint.rs` | `ctx.has_requested_repaint()` is true only in frames that called `DerefMut`. Read-only frames are false |
| 10 | `use_effect` deps | `effect_deps.rs` | Not rerun in frames where deps are the same; when they change, cleanup -> body run in that order |

kittest operations take the form `harness.get_by_label("+").click(); harness.run();`. Labels are looked up via AccessKit, so make button texts unique.

## 5. examples/spike

In eframe's `App::ui`, `begin_pass` / `end_pass` the `Store`, and draw Counter and Dialog (with an open button). Set `Options::max_passes = 2`. This is for visual checks of behavior, not a test.

## 6. Steps

1. Create the workspace and CI, and confirm CI is green with empty crates.
2. Implement in the order `store.rs` -> `cx.rs` -> `state.rs` -> `hooks.rs`, and pass tests 1, 8, 9, 10.
3. Implement `events.rs`, settle the `Handler` coherence issue, and pass test 3.
4. Implement `hook_scope` and collision detection, and pass tests 2, 6.
5. Implement sweep and cleanup, and pass test 7.
6. Implement `context.rs`, and pass test 4.
7. Add egui_taffy as a dev-dependency, and pass test 5.
8. Write examples/spike and check visually with `cargo run -p spike`.
9. Reflect assumptions that broke during verification in ARCHITECTURE.md. At least `into_handle` (2.3) and the `Handler` conclusion (2.6) will need to be added.
10. Write the result per verification item and the ARCHITECTURE.md changes in the PR body.

## 7. Points likely to need a decision

- Policy if the `Handler` blanket impls conflict (described in 2.6).
- If `elsa::FrozenMap`'s `as_mut()` is awkward, `Store` may be implemented by hand with `slots: UnsafeCell<HashMap<Id, Box<Slot>>>`. In that case write the safety reasoning in a comment (insert does not move the contents of `Box`, removal happens only via `&mut self`).
- If `num_completed_passes` cannot be read directly in kittest, count the root closure's calls with a counter outside `Store`.
- If egui_taffy does not request discard in test 5(b), treat item 5 as met if the manual version (a) passes, delete (b), and write the reason in the PR body.

## 8. Differences found during implementation (steps 2 to 3)

Differences between this document's sketch and the actual implementation. Those with design meaning are already reflected in ARCHITECTURE.md.

- **2.1** Adopted `elsa::FrozenMap`. No unsafe. sweep gets `&mut HashMap` via the `AsMut::as_mut` trait and does `retain` (unlike the wording in this document, it is not an inherent method).
- **2.2** `Cx::scope`'s `source` is `impl Hash + Debug`, not `impl Hash`. `Ui::push_id` in egui 0.36 requires `AsIdSalt = Hash + Debug`. `rsx!`'s `key={..}` will also need `Debug`.
- **2.3** `State` holds `inner: Option<RefMut<'s, T>>`. A type that implements `Drop` cannot move a field out, so `into_handle` sets `inner = None` to release the borrow before creating the `Handle`. `ctx` is `&'s egui::Context`, not owned (`Store` holds one clone, and both `State` and `Handle` borrow it). At `Store::new()` it temporarily holds `Context::default()`, replaced on the first `begin_pass`.
- **2.4** The blanket impls of `IntoCleanup` for `()` and `FnOnce()` conflict with E0119. Distinguish them with marker types (`NoCleanup` / `FnCleanup`) as `IntoCleanup<Marker>`. The call side does not change.
- **2.6** `Handler` was solved the same way, and the `call0` / `call1` split became unnecessary. The final form is `Handler<A, Marker>`, with impls `(Arity0, R)` for `F: FnOnce() -> R` and `(Arity1, R)` for `F: FnOnce(A) -> R`. Without `R` in the marker, bodies that return non-`()` error. The test `fused_events::handler_call_shapes` pins down that inference is unambiguous in every shape (no argument type annotation, payload dropped, borrowed payload, `fn` item). The macro emits `::egui_react::Handler::call(closure, a)` fully qualified.
- **2.6** `Emitter` cannot be built with one lifetime (`RefCell<T>` is invariant, and the borrow of the local `RefCell` is shorter than the props lifetime). The final form is `EventSink<'e, E> = RefCell<&'e mut (dyn FnMut(E) + 'e)>` and `Emitter<'a, 'e, E> { sink: &'a EventSink<'e, E> }`.
- **3** The handler's `(|| ..)()` expansion triggers clippy's `redundant_closure_call`. Tests use a file-level `allow`. `rsx!` must attach `#[allow(clippy::redundant_closure_call)]` to its expansion.
- **4 test 9** Writing state every pass requests a repaint every pass, and `Harness::run` panics with `ExceededMaxSteps`. Tests that write every frame use `harness.step()`.
- **New borrow constraint** Passing a value prop that borrows state and a handler that mutates the same state to the same element gives E0502 (`title={&*title} on_rename={|s| *title = s}`). Tests clone the value first. Already added to ARCHITECTURE.md 3.7.

## 9. Differences found during implementation (steps 4 to 8)

- **2.5** `Handle<'s, T>` cannot go into `Vec<(TypeId, Box<dyn Any>)>` (`dyn Any` requires `'static`, and `Handle` borrows the store). Instead, push slot Ids as `(TypeId, egui::Id)`, and `use_context` rebuilds the `Handle` from `store.slot_by_id(id)`. Added `id` to `Slot` and `slot_id()` to `Handle`. No `Box`, no downcast, no unsafe, and `Store` gets no lifetime parameter.
- **2.3 / 4 test 4** The "use a `Handle` on the same slot while the guard is alive" panic is unreachable through the public API, so no `#[should_panic]` test was written. The only ways to get a `Handle` are `use_handle` and `into_handle`.
- **4 test 6** Collisions have two stages. If the first guard is alive, panic with a clear message including the call site (detected via `try_borrow_mut`). If it has dropped, silently reuse the same slot, with only a record and `log::warn!`. The latter is the main target of the overlay (Phase 2).
- **4 test 5** The egui_taffy version (b) also produced a discard, so it was not deleted. It also asserts that a steady frame is 1 pass, showing that the extra pass is requested by taffy.
- **7** Counting passes by the root closure's call count is wrong. kittest's `Node::click()` queues 2 events, press and release, and `Harness::step()` runs 1 frame per event, so one `step()` runs 2 frames. Use `egui::Context::current_pass_index()` (starts at 0 within a frame).
- **Note on 5.3** If an effect is placed before a handler and its deps depend on the state that handler changes, deps have changed in pass 2, so the effect runs. The invariant of one run per deps change holds (test `effect_deps_changed_during_pass_one_rerun_in_pass_two`).
- **8** `self.store.begin_pass(ui.ctx())` -> `Cx::new(&self.store, ..)` -> `self.store.end_pass()` inside `eframe::App::ui` passes as-is under NLL. `egui-react-app::run` can use this shape.
- **Note for Phase 4** egui_taffy's `tui.ui(..)` / `tui.label(..)` are methods of the `TuiBuilderLogic` trait, and need `use egui_taffy::TuiBuilderLogic as _;`.
- **8 / note for Phase 5** The root `Ui` that eframe's `App::ui` passes has no margin and no background. In light mode, the light theme's dark gray text is drawn on eframe's default black clear color and cannot be seen. examples/spike wraps it in `egui::CentralPanel::default().show(ui, ..)`. `egui-react-app::run` must also wrap in CentralPanel (or `Frame::central_panel`).
