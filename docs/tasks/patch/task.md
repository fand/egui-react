# Task: patch (PR8 = second half of Phase 6.6. One PR together with [board](../board/task.md))

## Goal

Show that **complex UI can be built as-is with egui-reactor**, using a TouchDesigner-style node editor. [board](../board/task.md) proved each property one at a time (local state, keys, custom components, custom hooks, context). The job here is to show those properties still hold on a screen the size of three panes + a node graph + a GPU preview.

Building a full VJ app is not the goal. Nodes are limited to "a node where you can write a shader + five standard effects".

The technical core is **graph -> WGSL generation -> one fragment shader**. There is no offscreen compositing pass, so the implementation is light, and it fits the claim egui-reactor makes.

- `use_memo` rebuilds the WGSL only when the graph shape changes, and the pipeline is rebuilt only when the WGSL changes.
- Moving a slider does not regenerate the shader. The value just flows into a uniform.
- There are two stages on the "state changes, so the picture changes" path, and both show up in code as derived values (memo).

The `use_dnd` that board builds is reused for moving nodes and wiring ports. This is also why board comes first.

Do not touch core (`egui-reactor`, `egui-reactor-macros`).

## Scope

### In scope

- `examples/patch` (lib + bin, no raw egui version).
- Screen: node palette on the left, patch canvas in the center (pan, node drag, wiring), output preview and inspector on the right (stacked vertically).
- Node kinds (each has 0 to 2 inputs and 1 output):
  - `Shader`: write a WGSL expression directly. Also acts as a source. Three parameters can be referenced from the expression.
  - `Level`: brightness / contrast / gamma.
  - `Mix`: 2 inputs. Blend mode (mix / add / multiply / screen / difference) and amount.
  - `HSV`: hue / saturation / value.
  - `Transform`: translate / rotate / scale applied on the UV side.
  - `Grayscale`: luma / average / max.
  - `Output`: the terminal. Only one.
- Node-local state (collapsed, inline name edit, drag offset, port hover) lives in the node's own `use_state`.
- Polymorphic rendering by node kind. Both the node body and the inspector split into separate components via `match kind`.
- Chain of derived values: graph -> topological order -> WGSL generation -> validation with naga -> pipeline. Put a `use_memo` at each stage. Validation errors show as text in the inspector.
- Wiring: drag from a port and drop on another port. Use board's `use_dnd` with only the payload type swapped. Draw wires with the `<Canvas>` painter.
- Preview: `<Canvas>` + `egui_wgpu::Callback`. Store by type in `callback_resources`, same as the `shader` example.
- Load one preset patch with `use_future`, and only the preview shows a spinner via `<Suspense>` (a demo of only part of the screen waiting. This is the only place this example uses async).
- The graph uses `use_reducer` + `use_persisted`. Undo / redo reuses board's `use_undoable`.
- Register in gallery, kittest, README table, update ARCHITECTURE (if needed).

### Out of scope

- Raw egui version. Writing the same thing twice at this scale is not worth it; board has the job of showing the difference.
- Offscreen render targets, multiple passes, feedback, texture / video / camera input. The only source is the `Shader` node.
- Node copy & paste, grouping, comments, auto-layout, minimap, box selection of multiple nodes.
- Save format compatibility, file read/write, OSC / MIDI.
- Hot reload at runtime, highlighting the error location in the shader (we stop at showing the message).
- Changes to core. If a need comes up, write it in plan.md and split it into a separate PR.

## Deliverables

- `examples/patch/` (`src/lib.rs` / `graph.rs` / `codegen.rs` / `gpu.rs` / `main.rs` / `tests/patch.rs` and others).
- Registration in `examples/gallery` (`EXAMPLES`, the `Running` `match`, addition to `setup`).
- One row in the README table. The "Differences found during implementation" section in plan.md.

## Done criteria

- kittest: add a node -> wire it -> change a parameter, and the generated WGSL changes as expected (verify as a string). Creating a cycle gives a validation error, and the previous pipeline stays without crashing. Reordering / deleting nodes keeps the collapsed state and name draft attached to the remaining nodes (same property as board B-2, in a deeper tree).
- `codegen` unit tests: one per node kind, nested Transform, shared DAG, cycle detection.
- `cargo run -p patch` shows the preview, sliders change the picture, and editing the `Shader` node's expression recompiles (visual check). Same in the browser with `trunk serve` (visual check. For browsers without WebGPU, check with the WebGL fallback).
- `patch` appears in gallery, and lives with `shader` without `callback_resources` clashing.
- CI (fmt / clippy / test / wasm check / trunk loop) is green.

## Decisions (assumptions at the start)

- **Generate each node as `fn n<id>(uv: vec2<f32>) -> vec4<f32>`, and call them by walking from `Output`.** `Transform` transforms uv and then calls its input, so we do not hoist common subexpressions (a different uv gives a different result). If the same output connects to two places it is called twice, but correctness does not change.
- **Parameters are uniforms, topology means recompile.** The uniform is a fixed-length `array<vec4<f32>, 32>`, and each node's slot number is baked in at codegen time. Blend mode and Grayscale mode change the expression, so they are on the recompile side.
- Do not create the pipeline in `Options::setup` (the graph is not known yet). `setup` only places an empty `PatchResources`, and `CallbackTrait::prepare` rebuilds when the WGSL hash changes. If it cannot be built, keep drawing with the previous pipeline.
- Validate WGSL with naga before creating the pipeline, and only call `create_shader_module` when it passes. This avoids pipeline creation crashing on user-written expressions, and errors become UI text.
- Node placement uses absolute coordinates, and `ItemStyle` has no `position: absolute`. The patch canvas uses the escape hatch (create a child `Ui` with `max_rect` per node, and recreate `Cx` inside it), and **the inside of a node is built with `<View>` as usual** (the shape in ARCHITECTURE 3.1 and the `escape-hatch` example). Consider adding absolute positioning to `ItemStyle` only once we find we cannot write it this way.
- If zoom goes in, lean on `Context::set_transform_layer`. If it needs real work, drop it and keep only pan.
- Since it goes into gallery, do not use `Panel` / `CentralPanel`; fill the given area. Do not use `std::time::Instant`. `use_persisted` keys are `"patch/..."`.

## Steps

Bundle into one PR with board. Finish board to its done criteria before starting patch. The shape of `use_dnd` and the custom components is settled in board, so update section 5 of [plan.md](plan.md) to match the real thing at that point.
