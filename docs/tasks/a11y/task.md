# Task: a11y (web accessibility, timing undecided)

## Goal

Make egui-react apps that run on the web (wasm) usable from assistive technology: screen readers and keyboard navigation. On native, egui passes the widget tree to the OS accessibility API through AccessKit. On the web, that tree is thrown away. Keep drawing on the canvas, and mirror only the accessibility tree into the DOM (the same approach as the semantics layer in Flutter web).

This is not an egui-react specific problem. It is an unfinished part of egui / AccessKit / eframe. So we assume the result goes upstream (an AccessKit web adapter, a hook in eframe). egui-react is in a position where "once it lands upstream, we get the benefit for free". This task builds a prototype to confirm the shape, and takes it as far as bringing it upstream.

## Background (as of 2026-09, egui / eframe 0.36)

| Layer | State |
|---|---|
| egui → AccessKit tree | Exists. Call `Context::enable_accesskit()` and every frame a diff comes out in `PlatformOutput.accesskit_update: Option<TreeUpdate>`. Works on the web too |
| AccessKit → OS (native) | Exists. macOS / Windows / Unix (AT-SPI) / Android. egui-winit calls `enable_accesskit` and hands it to the adapter |
| AccessKit → DOM (web) | **No released version.** There is no `accesskit_web` on crates.io. There is only one prototype on the `web-basics` branch that stopped in 2024-07 (plan.md 1.1) |
| A hook in eframe web to receive the tree | eframe has **none**. `eframe/src/web/app_runner.rs` discards it with `accesskit_update: _, // not currently implemented`, and `App` cannot touch `FullOutput`. But **you can pick it up from `Plugin::output_hook(&Context, &mut FullOutput)` in egui 0.36** (plan.md 1.2) |
| Web fallback | The `web_screen_reader` feature (on by default). Only when `Options.screen_reader = true`, it reads out a description of events that happened via `speechSynthesis`. No tree, no focus movement |
| Keeping the tree | `accesskit_consumer` can apply a `TreeUpdate` and walk it (`Tree::new` / `update_and_process_changes`, `Node::role() / label() / value() / bounding_box() / is_focused()`). kittest uses the same path |
| Reverse direction | Actions from assistive technology are `accesskit::ActionRequest` (Click / Focus / SetValue, etc.). egui receives and handles them as `Event::AccessKitActionRequest` |

egui-react elements use egui widgets as-is, so they get the native support for free (kittest finding elements by label is the proof).

## Scope

### In scope

- **Research**: check whether AccessKit has any discussion / implementation of a web adapter (issues, branches, attempts by other projects). Read the structure of the Flutter web semantics layer (element kinds, focus sync, how events go back, known weak points).
- **Prototype (closed inside egui-react)**:
  - Fork eframe or write our own web runner, and receive `accesskit_update`.
  - Keep the `TreeUpdate` with `accesskit_consumer`, and lay out transparent DOM elements over the canvas (`role` / `aria-label` / `aria-valuenow` / absolute coordinates).
  - Focus sync (egui → DOM `focus()`, DOM → egui `ActionRequest::Focus`).
  - Convert DOM click / keydown / input into `ActionRequest` and send it back to egui.
  - Target widgets: Button / Checkbox / Label / TextEdit / Slider / ComboBox (all widgets in egui-react-elements).
  - Check by eye that the gallery can be operated from VoiceOver (macOS Safari / Chrome).
- **Upstreaming**: split the shape confirmed by the prototype into PRs: an AccessKit web adapter (new crate) and a hook in eframe (a callback on `WebOptions`, etc.).
- **Work on the egui-react side** (what we can do now, independent of upstream):
  - Make element `label`s close to required (alt text for `Image`, an `aria-label` equivalent for icon-only `Button`s).
  - State in the non-goals of ARCHITECTURE.md section 1 that "web a11y depends on web support in egui / AccessKit", and add one sentence to the gallery description in the README.

### Out of scope

- **A backend that redraws in the DOM** (not using egui on the web). It conflicts with everything: the design without a reconciler (ARCHITECTURE 2.1), the design where handlers borrow `&mut` on the spot, and the escape hatch to raw egui. If you want that, use Dioxus.
- Text selection, find in page, translation, SEO. These are limits of canvas drawing, and Flutter web has not solved them either.
- Improvements on the native side (the domain of egui / AccessKit).
- IME improvements (a separate problem).

## Deliverables

- `docs/tasks/a11y/plan.md` (a detailed plan that reflects the research results. Written when work starts).
- Prototype code (a forked eframe or our own runner, and a prototype adapter). Not merged into egui-react main; kept on a branch or in a separate repository.
- Upstream issues / PRs (AccessKit, eframe).
- One sentence each in ARCHITECTURE.md / README.

## Done criteria

- In the gallery (web), counter / todo / form: VoiceOver reads button names, check state, and text field values; Tab moves focus; Enter / Space operates the widget (checked by eye).
- ~~A proposal for a web adapter and an eframe hook is submitted upstream (merging is not a condition).~~ Withdrawn on 2026-09-05. We do not submit upstream. We keep `accesskit-web` and `WebA11y` inside egui-react.
- ARCHITECTURE.md describes the current state and policy for web a11y.

## Decisions (assumptions at the start)

- The approach is "canvas drawing + DOM mirror". We do not replace drawing.
- Write the adapter independent of egui (it looks only at AccessKit's `TreeUpdate`), in the same shape as the other AccessKit adapters. Nothing specific to egui / egui-react goes in.
- Do the prototype closed inside the egui-react-app wasm runner. Once it works, split it out upstream. Do not add a forked eframe as a dependency of egui-react main.
- Priority is after Phase 8 (release prep). Until then, only write the limitation in the README.

## Estimate

- Adapter prototype (Button / Checkbox / Label / TextEdit, focus and click round trip): a few hundred lines, a few days.
- Production quality (Slider / ComboBox / lists / live regions / scrolling / consistency with IME): the Flutter web semantics layer is several thousand lines, which is the yardstick. This is upstream work.
