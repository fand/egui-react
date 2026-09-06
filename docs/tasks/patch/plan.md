# Plan: patch

The task definition is in [task.md](task.md). The design rationale is in [docs/ARCHITECTURE.md](../../ARCHITECTURE.md). This ships in one PR with [board](../board/plan.md); finish board first, then start here. If you decide to depart from this plan during implementation, update this document, and update ARCHITECTURE.md too if the change has design meaning.

## 0. Overview

```
examples/patch/src/
  lib.rs      App and the 3 panes. The patch canvas and nodes live here too (this is what gallery shows)
  graph.rs    Graph / Node / Kind / Msg / reduce. Pure functions, with unit tests
  codegen.rs  Graph -> WGSL string. Validation with naga. Pure functions, with unit tests
  prelude.wgsl The base the generated code sits on (uniform, vertex shader, color conversion). include_str!
  gpu.rs      setup(cc), PatchResources, PatchCallback: CallbackTrait
  preset.rs   The default patch loaded at startup (JSON via include_str!)
  main.rs     run(Options { setup: Some(Box::new(gpu::setup)), .. }, ..)
```

The key point is that `codegen.rs` knows nothing about the GPU. It is a pure function that builds a string, so tests can be written as string comparisons.

## 1. Data model (`graph.rs`)

```rust
pub type NodeId = u64;

pub struct Graph {
    pub nodes: Vec<Node>,          // draw order = z order. Move the grabbed node to the end
    next_id: u64,
    /// +1 only on changes that can alter the generated WGSL. deps for `use_memo`.
    pub topology_rev: u64,
    /// +1 on parameter-only changes. Only affects uniforms.
    pub param_rev: u64,
}

pub struct Node {
    pub id: NodeId,
    pub name: String,
    pub kind: Kind,
    /// Graph coordinates. `[f32; 2]`, not `egui::Pos2` (see below)
    pub pos: [f32; 2],
    pub inputs: [Option<NodeId>; 2],
    /// One uniform slot. Meaning differs per Kind.
    pub params: [f32; 4],
}

pub enum Kind {
    Shader { src: String },                 // src is on the topology side
    Level, Hsv, Grayscale { method: GrayMethod },
    Transform,
    Mix { mode: MixMode },
    Output,
}

pub enum Msg {
    AddNode { kind: Kind, pos: egui::Pos2 },
    RemoveNode(NodeId),
    MoveNode { node: NodeId, pos: egui::Pos2 },
    Connect { from: NodeId, to: NodeId, port: usize },
    Disconnect { to: NodeId, port: usize },
    SetParam { node: NodeId, index: usize, value: f32 },
    SetShaderSrc { node: NodeId, src: String },
    SetMode { node: NodeId, mode: MixMode },
    SetGray { node: NodeId, method: GrayMethod },
    Rename { node: NodeId, name: String },
    Raise(NodeId),
    /// Replace the whole graph with a preset (4.2). Advance both revs past the current values
    Load(Graph),
}
```

Like `board.rs`, `graph.rs` knows nothing about egui. That is why `pos` is `[f32; 2]` and not `egui::Pos2`. It also has a practical benefit: the JSON for `use_persisted` and `preset` no longer depends on the serde feature of `emath` (who enables it is decided by dependency layout). The screen side converts with `egui::pos2(node.pos[0], node.pos[1])`.

Node names appear as comments in the generated WGSL (`// level1`), so `Rename` is on the topology side.

**Splitting `topology_rev` and `param_rev` is the core of this example.** `reduce` decides which one to bump by the kind of change (`SetParam`, `MoveNode`, and `Raise` are on the param side; everything else is on the topology side). Test P-5 pins down that moving a slider does not regenerate the WGSL.

There is only one `Output`. `AddNode` does not create it; the initial graph has it.

## 2. WGSL generation (`codegen.rs`)

```rust
pub struct Generated { pub wgsl: String, pub slots: Vec<NodeId>, pub hash: u64 }
pub fn generate(graph: &Graph) -> Result<Generated, GenError>;
pub fn pack_params(graph: &Graph, slots: &[NodeId]) -> [[f32; 4]; 32];
pub enum GenError { Cycle(NodeId), TooManyNodes, Wgsl(String) }
```

`hash` is the hash of the generated string itself. It decides whether to rebuild the pipeline (section 3). Computing it on the GPU side would mean checking "is it the same string" every frame, so the side that builds it counts once.

- Each node becomes `fn n<id>(uv: vec2<f32>) -> vec4<f32>`. Only nodes reachable from `Output` are emitted, in an order where dependencies come first (topological order; WGSL cannot forward-reference).
- An unconnected input port is `vec4<f32>(0.0, 0.0, 0.0, 1.0)`. If nothing is connected to `Output`, emit only a function that returns black.
- If the same output connects to two places it is called twice. `Transform` changes uv, so we do not hoist (a decision in task.md).
- Cycles are detected by DFS before generation and give `GenError::Cycle`.
- uniform:

```wgsl
struct Uniforms {
    time: f32,
    _pad: f32,
    resolution: vec2<f32>,
    p: array<vec4<f32>, 32>,   // node slots. codegen bakes in the index
};
@group(0) @binding(0) var<uniform> U: Uniforms;
```

  Returns `slots[i] == node_id`, and the UI side packs `params` in this order. If nodes exceed 32, `TooManyNodes` (the palette stops letting you add).
- Body per node kind (`p` is that node's slot):

| Kind | params | Body |
|---|---|---|
| `Shader { src }` | p0 p1 p2 | `let t = U.time; let p0 = ..; return <src>;` |
| `Level` | brightness contrast gamma | `pow(clamp((c-0.5)*contrast+0.5+brightness, 0, 1), vec3(1/max(gamma, 1e-3)))` |
| `Hsv` | hue sat val | Pass through the prelude's `rgb2hsv` / `hsv2rgb` |
| `Grayscale { method }` | – | luma `dot(c, vec3(0.2126, 0.7152, 0.0722))` / average / max. Method is baked in |
| `Transform` | tx ty rot scale | Rotate, scale, and translate uv around center 0.5, then call the input |
| `Mix { mode }` | amount | mix / add / multiply / screen / difference. mode is baked in |
| `Output` | – | Return the input as-is |

- The prelude (uniform declaration, fullscreen triangle vertex shader, `rgb2hsv` / `hsv2rgb`) comes from `include_str!("prelude.wgsl")` and goes **before** the generated part. `fs_main` is emitted last by codegen, not by the prelude (it only calls `n<output>(uv)`). This order avoids any forward reference; if `fs_main` lived in the prelude, the prelude would call generated functions before they are defined.
- Cycles are detected over the whole graph (not just the part reachable from `Output`). A cycle in a disconnected area is reported with the same message, but in return we can say in one line: "a cycle anywhere is an error".
- Slots are handed out one at a time, in generation order, to nodes reachable from `Output`. `Grayscale` and `Output` have no parameters but still get one, so that codegen and uniform packing both follow "one at a time, in order".
- Validation is **naga on the CPU side** (`naga::front::wgsl::parse_str` -> `naga::valid::Validator`). It needs no GPU or device, so it runs inside `use_memo`, and errors become UI text as-is. Use `wgpu::naga` if available; if not, pin the version wgpu 30 uses in the workspace. Errors in user-written expressions stop here and never reach pipeline creation.

## 3. GPU side (`gpu.rs`)

Same shape as the `shader` example. The only difference is that the pipeline is not created at startup.

```rust
pub struct PatchResources {
    buffer: wgpu::Buffer, bind_group: wgpu::BindGroup, layout: wgpu::PipelineLayout,
    target_format: wgpu::TextureFormat,
    pipeline: Option<wgpu::RenderPipeline>,
    source_hash: u64,
}
pub fn setup(cc: &eframe::CreationContext<'_>);   // up to buffer / bind group / layout
pub struct PatchCallback { pub wgsl: Arc<str>, pub source_hash: u64, pub uniforms: Uniforms }
```

- In `prepare`, if `source_hash` differs, call `create_shader_module` + `create_render_pipeline` and swap. On failure keep the previous pipeline (the picture does not drop to black). Only naga-validated code arrives here, so failure here is an unexpected case.
- Uniforms go through `queue.write_buffer` every frame. `paint` draws 3 vertices.
- `callback_resources` is looked up by type, so it can live with `shader` in gallery (same reasoning as the comment in the `shader` example).

## 4. Screen (`lib.rs`)

```
App
└ PatchProvider              provide_context for Dispatch / Dnd / port positions (same shape as board)
  └ View row grow
    ├ Palette (w=150)        title, undo / redo, buttons per node kind. Click to add
    └ Suspense fallback      ★ only what is right of here waits for the preset (4.2)
      └ Stage                use_future that loads the preset, plus the 2 columns below
        ├ View column grow   ★ patch canvas (4.1 below)
        └ View column (w=300)
          ├ Preview          <Canvas> + wgpu callback. play / pause
          └ Inspector        tabs: params / wgsl
```

`Stage` is a `#[component]` but does not carve out a Ui (`cx.scope` in taffy mode only does `with_auto_id_prefix` and adds no node). `Suspense` is `shares_ui`, so these 2 columns become flex items of the row as-is. Undo / redo sits at the top of the palette instead of a toolbar so it is not inside the waiting side (inside Suspense).

- `Inspector`'s `params` is a separate component per selected node's `match kind` (`LevelParams`, `MixParams`, ..). The `wgsl` tab shows the generated code and errors as-is. **This is a feature for show, and also the way tests read the generated result from the UI** (section 6).
- Chain of derived values:

```rust
let gen = use_memo(cx, (graph.topology_rev, epoch), || codegen::generate(&graph));   // Result
let program: State<Option<Program>> = ..;   // swap only on Ok; keep the previous on Err
let params = use_memo(cx, (graph.param_rev, graph.topology_rev, epoch), || {
    codegen::pack_params(&graph, &program.slots)
});
```

  When a slider moves, only the second one runs. Explain this two-stage structure in the comment at the top of `lib.rs`.

  `epoch` in deps is the count of undo / redo. **rev rolls back on undo**, so with rev alone as deps, "undo then make a different edit" can attach the same rev as before to different content. In practice there is always one frame drawn between two messages, and memo sees that intermediate rev and rebuilds, so there is no accident; but that is timing, not a property of deps, and it breaks the moment one reducer visit takes two messages. epoch never goes back, so `(rev, epoch)` is monotonic and this question does not arise. board's `Board::rev` has the same shape (section 8).

  What holds the last successful result is not memo but `use_state<Option<Program>>`. memo only "rebuilds when deps change" and has no "keep the previous value" semantics, so state takes the role of keeping the previous pipeline and previous WGSL visible on `Err` (P-4). It writes only when `hash` changes, so it does not go dirty every frame.

### 4.1 Patch canvas

Node placement uses absolute coordinates, and `ItemStyle` has no absolute positioning. Drop down the same way as `Nested` in the `escape-hatch` example.

```rust
let (store, scope) = (cx.store, cx.scope_id());
// inside <View grow={1.0} h={0.0}>: {view(move |cx| cx.leaf_fill(&style, move |ui| { .. }))}
//   1. ui.allocate_exact_size(available, Sense::click_and_drag()) for the area and background response
//   2. background drag pans (return to the parent's use_state<Vec2> via on_pan)
//   3. reserve one spot in the painter for the wires (Shape::Noop)
//   4. per node:
//        ui.scope_builder(egui::UiBuilder::new().max_rect(node_rect), |ui| {
//            let mut cx = Cx::new(store, ui, scope);
//            cx.scope(node.id, |cx| rsx! { <NodeView ../> }.show(cx));   // <- this is the key
//        });
//   5. build wires from the port positions the nodes registered, and painter.set into the reserved spot
```

- `cx.scope(node.id, ..)` is what `key={..}` in rsx! stands for. Only here is it written by hand, so **the code shows what a key really is: something mixed into the scope Id**. Tie it to board's `key=` in a comment.
- Inside a node is a normal `<View>`. A `container` directly under Ui mode opens a new taffy tree, so the header row (name + collapse) and the vertical parameter list can be built with flex.
- Node-local state: `collapsed` / `editing_name` / `draft_name` / `hovered_port`. Test P-3 pins down that **deleting a node keeps the remaining nodes' state attached**.
- `use_identity` is not needed here. A node's scope is `canvas -> node.id -> NodeView`, and the position inside `graph.nodes` is not part of it (reordering with `Raise` or deleting a neighbor does not change that node's slot Id). board's `<Card key={id}/>` was not enough as identity because the card's scope had a **column** in between; here nothing is in between. Write this difference in a comment in lib.rs.
- **The canvas holds the offset during drag** (`use_state<Option<(NodeId, Vec2)>>`). The node rect must be decided **before** drawing the node, so only values that affect position belong to the canvas. The node just reports grab / move amount / release via `#[event]`, and `Msg::MoveNode` fires once on release (firing every frame during drag would make undo one step per pixel).
- **Port positions are registered every frame.** The node header row is flex, so the center of a port circle is not known until it is drawn. Pass `Rc<RefCell<HashMap<PortRef, Pos2>>>` via context; nodes write their own port positions, and the canvas reads them in step 5. `Handle` has no non-dirtying write, for the same reason as board 8.2, so we use the same workaround.
- z order is the order of `graph.nodes`. On grab, `Msg::Raise` moves it to the end.
- No zoom (a decision in task.md). Pan only.

### 4.2 Preset and Suspense

```rust
let loaded = use_future(cx, (), || async { preset::parse(preset::STARTER) });
let Poll::Ready(graph0) = loaded else { return };   // the nearest <Suspense> draws the fallback
```

`STARTER` is JSON via `include_str!`, so the wait is nearly zero, but `<Suspense>` starts suspended (ARCHITECTURE 5.8), so the fallback is drawn for at least one pass. `<Suspense fallback={..}>` wraps only the canvas and preview; the palette (including undo / redo) does not wait. The job here is to demo that **only part of the screen waits**; add a comment that a real app would fetch here.

The graph itself (`use_persisted` + `use_undoable`) sits **above** `Suspense`. It must be there so the palette can add nodes without waiting. The loaded preset flows from `Stage` once as `Msg::Load` (only when the saved patch is just one `Output` = untouched). `Load` is one message like any other edit, so undo returns to an empty patch.

## 5. Reuse from board

This example uses board's `hooks.rs` (`use_dnd` / `use_undoable`). There are two ways; decide when board is done.

- (a) Make `examples/board` a dependency of `patch`, and make `board::hooks` `pub`. No extra crate needed.
- (b) Split out `examples/hooks` (tentative) as a new crate, and have both board and patch depend on it.

**(a) is the default.** Gallery examples depending on each other is bad manners, but splitting out weakens the claim "the same hook was reused across two screens". Either way, there is one hook body for both examples. Fall to (b) only if board's hook gets bent out of shape for patch.

The result is **(a)**: `examples/patch/Cargo.toml` has `board = { path = "../board" }`, and we write `use board::hooks::{Dnd, Undoable, use_dnd, use_undoable};`. Not one character changed on the board side.

`use_dnd<P, T>` is used **only for wiring** (`P = T = PortRef { node, port }`: grab an output port and drop on an input port). It is not used to move nodes. `Dnd` is a tool for "drop the grabbed thing onto one of the registered slots", and moving, which needs no drop target, is just `Response::drag_delta`. The grabbed payload and the drop target have the same type because both are ports; that is not a degenerate case.

`use_undoable` is used on `Graph` as-is. `use_identity` is not used (4.1). So 2 of board's 3 hooks ran on another screen without changing shape.

## 6. Tests (`tests/patch.rs`)

kittest runs headless (no GPU). The wgpu callback put on `<Canvas>` is simply not drawn, and it does not affect UI verification (same as the `shader` example).

- **P-1** Adding a node from the palette adds a node to the canvas and shows it in the inspector.
- **P-2** Connecting the added node to Output makes that node's function and call appear in the body of the `wgsl` tab.
- **P-3** Deleting one node keeps the remaining nodes' collapsed state and name draft attached (board B-2's property, in a deeper tree).
- **P-4** Creating a cycle shows an error in the inspector, and the `wgsl` tab keeps the previous content.
- **P-5 (the highlight)** Moving a parameter slider does not change one character of the `wgsl` tab body. Rewiring a node changes it. = proof that recompile and parameter update are separate.
- **P-6** Loading the preset shows the `<Suspense>` fallback at least once, and then nodes line up on the canvas. `Harness` runs the app until it settles at construction, so the naive version is "already resolved by the time we look". On the test side, (a) do not draw the app during construction, and (b) set `max_passes = 1` for the first frame only, and look at **the very pass that started the future** (the future never becomes Ready in that pass).
- **P-7** undo goes back to the previous program, and a different edit after undo properly becomes a new program (epoch in section 4).
- **P-8** The 3 columns fit in the window (`grow` + `h={0}` for `<Canvas>` and `ScrollArea`. Same trap as board 8.3).
- **P-9** Editing the `Shader` node's expression changes the program; a broken expression shows in the inspector as naga's message, and the picture keeps the previous program (the "invalid as WGSL" side, next to P-4's "cycle").
- **P-10** Adding two nodes of the same kind does not collide hook Ids (`Store::collisions()` is empty). Since the canvas writes `cx.scope(node.id, ..)` by hand, this is worth watching.
- `codegen` unit tests: one per Kind, nested `Transform`, a DAG with the same output wired to 2 inputs, cycle detection, more than 32 nodes. Run the output through naga (this guarantees by test that "generated WGSL is always valid").
- `graph` unit tests: one line per `Msg`, checking which of `topology_rev` / `param_rev` `reduce` bumps.

## 7. gallery / README

- Add the dependency to `examples/gallery/Cargo.toml`, `patch::META` to `EXAMPLES`, and `"patch" => { <PatchApp/> }` to the `Running` `match` (no plain). Call `patch::gpu::setup` too in gallery's `Options::setup` (next to `shader`).
- Put `use_future` and `use_memo` in `Meta`'s `hooks`, and `Canvas` / `Suspense` / `ComboBox` / `TextEdit` in `elements`. Make it reachable from gallery's tag filter.
- One row in the README table (live / source, plain is `–`).
- No snapshot (GPU output varies across platforms). Same treatment as `shader`.

## 8. Differences found during implementation

Neither core (`egui-react` / `egui-react-macros`) nor `egui-react-elements` was touched. Below is "what could not be written / how we worked around it / what we would add". Where this overlaps with board section 8, the overlap itself is the information, so it is stated explicitly.

### 8.1 The escape hatch was enough for absolute positioning. `ItemStyle` needs no `position`

The canvas is one leaf, and inside it we drop down through `ui.scope_builder(UiBuilder::new().max_rect(node_rect), ..)` -> `Cx::new(store, ui, scope)` -> `cx.scope(node.id, ..)`. Inside a node is a normal `<View>`, and a `container` directly under Ui mode opens a new taffy tree, so both the header row and the vertical parameter list can be written with flex. The same shape as `Nested` in `escape-hatch` worked as-is as an "absolute positioning layer".

**The `key` here removes the need for `use_identity`.** The scope chain for a node's hooks is `canvas -> node.id -> NodeView`, and the position inside `graph.nodes` never enters it. Reordering with `Raise` or deleting a neighbor does not change the slot Id, so board 8.1's `use_identity` is not needed here (board needed it because the card's scope had a **column** in between). There are two tools for "holding state by identity", and **if the tree shape already contains the identifier, that is enough**. That is what we learned by putting the two examples side by side.

**In the other direction, only the drag offset belongs to the canvas.** The rect a node is drawn in must be decided before drawing the node, so a value that affects position cannot live "inside the part". The node raises the gesture via `#[event]`, the canvas holds the position, and on release sends `Msg::MoveNode` once (sending every frame makes undo one step per pixel). **It is not always wrong for the whole to hold a part's state; "values needed before drawing" belong to the whole.** That is the line we could draw.

### 8.2 `<View>` returns no rect (reconfirming board 8.4), so we register every frame

Wire endpoints are the centers of port circles, and the circles sit inside the header row's flex. Neither `<View>` nor `<Frame>` returns a `Response`, so the only way to get positions is "the drawn node reports its own". Pass `Ports` (`Rc<RefCell<HashMap<PortRef, Pos2>>>`) via context; nodes `put`, the canvas reads. `Handle` has no non-dirtying write, as in board 8.2, so we used the same workaround (`Handle::with` + interior mutability). **Two examples fell into the same hole, so `Handle::with_mut` is a real piece of homework.**

We want wires drawn **under** the nodes, but positions are only known **after** drawing the nodes. Reserve a spot with `let idx = painter.add(Shape::Noop);`, draw the nodes, then fill it with `painter.set(idx, Shape::Vec(wires))`. egui's painter had this, so no extra layer was needed.

What we would add is the same `#[event] on_rect` on `<View>` as board 8.4. With it, `Ports` would not be needed.

### 8.3 `leaf_fill` takes "all", not "the rest" (reconfirming board 8.3)

Both the canvas (leaf_fill) and `ScrollArea`, with only `grow={1.0}`, ask for the whole window height and push out the row below. `grow={1.0} h={0.0}` (same as `<Canvas>` in the `shader` example) becomes "take only the remainder". The column height is fixed because the row has `h="100%"` and the root has `min_h: 100%`; if any link in this chain becomes auto, it stops working. P-8 watches this.

### 8.4 An element with `bind` cannot "report its current value"

`<TextEdit>`'s `bind` holds the only `&mut String`, so a handler on the same element cannot read the same string (the second point in ARCHITECTURE 3.7). `on_change` carries no payload; only `on_submit` (Enter) carries the string. The `Shader` node's expression is meant to show "regenerate and run through naga on every keystroke", so Enter is too slow. So we wrote `SourceEdit` ourselves as one leaf, let it write into a per-frame copy, and `on_change.emit(text)` only on `changed()`.

What we would add is a payload (the new string) on `TextEdit`'s `on_change`. Since `bind` holds the `&mut`, one clone on the element side is enough.

### 8.5 Painted circles and glyph buttons do not exist unless you name them

Ports are `ui.allocate_exact_size` + painter, and node collapse / name / delete are "-" "name" "x" buttons. The former leaves nothing in the accessibility tree, and the latter puts the same "x" on 8 nodes. We named them "`shader1 out`" and "`delete level1`" with `Response::widget_info(|| WidgetInfo::labeled(..))`. This is about screen readers, not about tests (easier tests were a side effect).

Also learned along the way: `egui::Slider` creates **two** accessibility nodes (the slider itself and the drag value that shows the number). Both have the same label, so `get_by_label` always finds 2. In kittest, look it up with `get_by_role_and_label(Role::Slider, ..)`.

### 8.6 `<Suspense>` cannot be seen after `Harness` construction

`Harness::from_builder` calls `run_ok()` at the end and runs until the app settles. The preset future is one thread and an `include_str!` parse, so it finishes during that, and by the time the test first looks, the boundary is already resolved. P-6 uses two steps: (a) construct with an app that has a "do not draw yet" flag, and (b) set `max_passes = 1` and run one pass only. **A future never becomes Ready in the pass that started it**, so this relies on causality, not time.

This is not a hole in the library; it is a lesson in how to write the test. Tests for examples with `<Suspense>` take this shape.

### 8.7 naga cannot be relied on via wgpu

The `wgpu::naga` re-export sits under `cfg(wgpu_core)` or `cfg(naga)`, and disappears with some backend configurations (such as WebGPU-only on wasm). Validating the generated WGSL on the CPU is the core of this example, so we added `naga = { version = "30.0", features = ["wgsl-in"] }` (the same version wgpu 30 uses) to the workspace. `codegen.rs` knows only naga, not wgpu or egui.

`ParseError::emit_to_string` / `WithSpan::emit_to_string` become the UI text as-is (with line numbers and excerpts), so we wrote zero lines of error formatting.

### 8.8 Small things

- The per-kind components are **used twice**: inside the node and in the inspector (one `wide` prop switches between drag value and slider). "Separate components via `match kind`" and "different looks in narrow and wide places" can coexist.
- WGSL is joined in an order with no forward references (prelude -> node functions -> `fs_main`). The generator emits `fs_main`.
- `Msg::Load(Box<Graph>)` is boxed. If one message is the size of one `Graph`, the `Vec<Msg>` queue grows for every other message too.
- `graph.rs` knows nothing about egui (`pos: [f32; 2]`). Same policy as board's `board.rs`, with the practical benefit that the JSON for `use_persisted` and preset does not depend on emath's serde feature.
