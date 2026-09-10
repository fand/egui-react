# Plan: a11y (web accessibility)

The task definition is in [task.md](task.md). The design rationale is in [docs/ARCHITECTURE.md](../../ARCHITECTURE.md). This document records the research results and the work steps we decided from them. If we decide to depart from it during implementation, we update this document, and ARCHITECTURE.md too if the change has design meaning.

The research was done in 2026-09. Targets: egui / eframe 0.36.1, accesskit 0.24.1, accesskit_consumer 0.38.0 (the versions pulled in by this repository's `Cargo.lock`. Upstream latest is accesskit 0.25.0 / consumer 0.39.0, released 2026-08-29).

## 0. Overview

**This does not finish in one PR. Split it into what goes into main and what lives on the `a11y-spike` branch.** This keeps the task.md decision "do not add a forked eframe as a dependency of egui-reactor main". The boundary is "can it be done with crates.io dependencies alone".

**The split changed after work started.** At first we assumed everything from step 6 onward would go on `a11y-spike` as a whole. But 1.2 showed that "you do not need to fork eframe to receive a `TreeUpdate`" (option C in 2.2). **Steps 6 to 10 close with crates.io dependencies alone, so they go into main.** Only step 11 (F2) needs the fork, and only that stays in the spike.

| | Where | What |
|---|---|---|
| PR #4 (done) | main | This document, one sentence each in ARCHITECTURE sections 1 / 8, one sentence in the README, label hardening in `egui-reactor-elements` (`alt` on `Image`, `label` on `Button`), a kittest that detects unnamed widgets |
| PR #5 (the follow-up) | main | `accesskit-web` (adapter crate), `egui_reactor_app::a11y::WebA11y` (an egui plugin), enabling it in the gallery, DOM → `ActionRequest`, focus F1 |
| Later | `a11y-spike` branch | Step 11 only: F2, relax `has_focus` in a forked eframe |
| After that | Upstream | A web adapter PR to AccessKit, an issue for a hook in eframe / egui |

There are two reasons for this split.

1. **Label hardening works regardless of upstream.** It works for native VoiceOver / NVDA / Orca starting today, and it will keep working on the day a web adapter lands. In fact, the moment the adapter works, every "unnamed button" is exposed at once, so fixing them first makes the prototype easier to read.
2. **Whether a fork is needed narrowed down to one point.** At first we wrote "the prototype may not close with crates.io dependencies alone", but the only part that does not close is **the focus round trip** (2.3). Both receiving the tree and returning `ActionRequest`s are covered by egui's `Plugin`. So what stays in the spike is one commit for F2.

## 1. Research results

### 1.1 AccessKit web adapter: one prototype exists, and it stopped at focus

The background table in task.md said "AccessKit has no web adapter". **More precisely: "not released, but there is a prototype of about 500 lines on a branch".**

- Discussion: [AccessKit/accesskit discussions#514 "Web Adapter"](https://github.com/AccessKit/accesskit/discussions/514) (2025-02). An outside contributor (floers) asked "I want to write a web adapter, would that be duplicate work?". The maintainer (DataTriny) replied "It already exists on the [`web-basics` branch](https://github.com/AccessKit/accesskit/tree/web-basics). It works fairly well but I have not touched it in months. Are you willing to take it over?".
- **What came after is the main point.** In 2025-03 floers built a **Slint web demo** on top of this branch ([floers/accesskit `web-basics`](https://github.com/floers/accesskit/tree/web-basics), `examples/slint-web`), and got it to the point where pressing a DOM button made the Slint side react. **They reached the same conclusion on how to build the reverse direction**: "`action_handler` was not something to implement, but something to **use** to send actions. I attached a JS `on_click` to the DOM button and sent through `action_handler`, and it worked".
- **And the maintainer's verification (2025-03-09) ended like this.** "I tried it, but **as-is it is not usable with a screen reader. Focus movement is not reliable** (it may be a Slint-side problem). Good as a demo." The thread has been quiet since. **This is the same wall we found independently in 1.3.**
- The last commit on the branch is `51dbebd` (2024-07-15, "Set application role on the root node"). The contents are `platforms/web/` with `Cargo.toml` (crate name `accesskit_web` 0.1.0), `src/lib.rs` / `adapter.rs` (6.6 KB) / `node.rs` (11.3 KB) / `filters.rs`. The AccessKit README lists web under "Planned adapters".
- **There is no `accesskit_web` on crates.io** (`https://crates.io/api/v1/crates/accesskit_web` is 404). Web is not in the release list either. The latest release wave on 2026-08-29 covered common / consumer / windows / macos / unix / android / **ios** / winit, and the iOS adapter ([issue#565](https://github.com/AccessKit/accesskit/issues/565)) shipped as `accesskit_ios` 0.2.0. Only web is left open.
- Searching the whole repository for "web", "DOM", "wasm", "browser" turns up no other web adapter discussion. What comes up around the DOM is [issue#705 "Add a property to allow exposing DOM ID"](https://github.com/AccessKit/accesskit/issues/705) → [PR#776 `html_id` node property](https://github.com/AccessKit/accesskit/pull/776) (merged 2026-08-25), but this is about **Servo using AccessKit in the "web content → OS" direction** ([servo/servo#4344](https://github.com/servo/servo/issues/4344)), which is the opposite direction.

Reading the `web-basics` code, **the skeleton is usable as-is, but it never got to the point of working**.

| Has | Lacks |
|---|---|
| `Adapter::new(parent_id, ActivationHandler, ActionHandler)`, `update_if_active(impl FnOnce() -> TreeUpdate)`, `update_host_focus_state(bool)` | **Coordinates.** Nodes only create nested `<div>`s. It does not read `bounding_box()`, and has no `position: absolute` or any CSS at all. It does not overlap the canvas |
| Applies only diffs to the DOM via `Tree` / `TreeChangeHandler` (`node_added` / `node_updated` / `focus_moved` / `node_removed`) | **The DOM → egui path.** It only holds `action_handler` and **never calls it**. No listeners for click or keydown |
| A `Role` → ARIA role table (about 150 lines, `CheckBox`→`checkbox`, `TextInput`→`textbox`, `Slider`→`slider`, ...) | A Tab order. `tabindex` is fixed at `"-1"` if focusable |
| `aria-label` / `aria-checked` / `aria-valuemin` / `aria-valuemax` / `aria-valuenow` / `aria-valuetext`, `textContent` only for `Role::Label` | `aria-expanded` / `aria-selected` / live regions / text selection / scrolling |
| `element.focus()` / `blur()` in `focus_moved` | A path back to egui when focus moves on the DOM side |

Also, **the API has drifted by two years**. `Node::name()` became `label()`, `is_focusable()` became `is_focusable(&parent_filter)`, and accesskit 0.24 added multi-tree support ([PR#655](https://github.com/AccessKit/accesskit/pull/655)), which added `tree_id` to `TreeUpdate` and `target_tree` to `ActionRequest`. The branch depends on accesskit 0.16 / consumer 0.24, so it does not build as-is.

**Conclusion: the shortest path is not from zero. Port these 500 lines to the current API and add the missing "coordinates", "DOM → egui", and "Tab order".** When bringing it upstream, it also takes the form "we revived your branch", which makes the conversation easy. And since the maintainer wrote that **the reason the web adapter stalled is the single point of "focus"**, that is also where the prototype can give a valuable answer (1.3, 2.3). The main result of this prototype is to measure and report "the tree mirrors fine. Focus in this shape works / does not work".

Note that `Cargo.lock` contains three consumer versions: 0.35 / 0.36 / 0.38 (kittest 0.4 uses 0.35, accesskit_windows uses 0.35, atspi uses 0.36, accesskit_macos uses 0.38). All of them work against accesskit 0.24.1, so **the prototype adapter can be built with `accesskit = "0.24.1"` + `accesskit_consumer = "0.38"`** (the same combination as the macos adapter). The condition is that the types match the `TreeUpdate` egui holds, so only the `accesskit` version must match egui.

### 1.2 eframe 0.36 web: the tree is indeed discarded, but we can pick it up without a fork

Line 394 of `crates/eframe/src/web/app_runner.rs`, the destructuring pattern in `handle_platform_output`, has `accesskit_update: _, // not currently implemented` (locally at `~/.cargo/registry/.../eframe-0.36.1/src/web/app_runner.rs:394`). This part is as task.md says. But on investigation, **three of the surrounding assumptions were wrong.**

**(a) `accesskit` is not an egui feature.** eframe's `accesskit` feature is only `["egui-winit/accesskit"]`, and it is native only. egui itself has `accesskit = "0.24.1"` as an **unconditional dependency**, and `Context::enable_accesskit()` / `disable_accesskit()` / `accesskit_node_builder()` can be called at any time (`egui-0.36.1/src/context.rs:3698`). **On wasm too, calling `ctx.enable_accesskit()` starts building the tree right away.** Nobody calls it, but the feature is alive. The app can be the caller: in `egui_reactor_app::Options::setup`, write `cc.egui_ctx.enable_accesskit()` from the `&CreationContext`.

**(b) egui 0.36 has a `Plugin` trait, and it can grab `FullOutput` from the side.** `egui::plugin::Plugin` (`egui-0.36.1/src/plugin.rs`) has the following.

```rust
pub trait Plugin: Send + Sync + std::any::Any + 'static {
    fn debug_name(&self) -> &'static str;
    fn setup(&mut self, ctx: &Context) {}
    fn on_begin_pass(&mut self, ui: &mut Ui) {}
    fn on_end_pass(&mut self, ui: &mut Ui) {}
    fn input_hook(&mut self, ctx: &Context, input: &mut RawInput) {}
    fn output_hook(&mut self, ctx: &Context, output: &mut FullOutput) {}   // <- here
}
```

At the end of `Context::end_pass()`, `plugins.on_output(self, &mut output)` is called (around `context.rs:2440`), and at that point `platform_output.accesskit_update` is already filled (it is filled by `ContextImpl::end_pass`, around `context.rs:2692`). Registration is `Context::add_plugin(impl Plugin)`, which is public.

So **there is no need to fork eframe to receive a `TreeUpdate`.** The reverse direction (injecting `ActionRequest`s) is covered by `Plugin::input_hook(&mut RawInput)` too. eframe also has a public hook, `App::raw_input_hook(&Context, &mut RawInput)` (`eframe-0.36.1/src/epi.rs:279`), but the plugin is better because it does not depend on eframe and works as-is under kittest.

Two caveats.

- `Plugin` requires `Send + Sync`, but `web_sys::HtmlElement` and friends are `!Send`. **Keep the DOM-side state in a `thread_local!` registry, and have the plugin struct hold only an integer key** (wasm is single-threaded, so there is no need to write `unsafe impl Send` either).
- `output_hook` is called **per pass**. egui redoes a pass that called `request_discard` (the `loop` at `context.rs:833`), and in egui-reactor taffy normally runs 2 passes (ARCHITECTURE 5.3). **Drop** the `TreeUpdate` of a pass where `output.platform_output.requested_discard()` is set. If you push the tree of a discarded pass to the DOM, coordinates from before layout settled flash for a moment.

**(c) The "hidden DOM" on the web side that we can reuse is not the screen reader path but the text agent.** The `web_screen_reader` feature (on by default) is really just `speak(text)` in `web/screen_reader.rs`, which sends one line of `platform_output.events_description()` to `speechSynthesis`. No tree, no focus. Nothing to reuse.

Instead, what is useful as a reference, and also **conflicts**, is `web/text_agent.rs`. For IME and mobile keyboards, it places a transparent `<input>` as a sibling of the canvas with `position: absolute` at the canvas top-left. A miniature version of what the DOM mirror wants to do is already there.

### 1.3 eframe web pins focus to the canvas (this is the only real wall)

`AppRunner::has_focus()` is "is the canvas or the text agent `document.activeElement`", and returns **`false`** when any other element has focus (`app_runner.rs:227`, `has_focus` in `web/mod.rs:77`). Then at the top of `logic()`, `update_focus()` sets `input.raw.focused = false`. On top of that, while the app has focus, `handle_platform_output` calls `focus_without_scroll(self.canvas())` every frame to pull focus back to the canvas.

So **if you `element.focus()` a mirror `<div>`, egui decides "I lost focus" and stops accepting key input.** Running the `web-basics` adapter's `focus_moved` → `element.focus()` as-is on top of eframe breaks here.

There are two ways out.

| | How | Cost |
|---|---|---|
| F1 | Keep DOM focus on the canvas, and tell "which node we are on" through `aria-activedescendant` on the canvas | No fork needed. But assistive technology support for `aria-activedescendant` is limited to certain roles, and VoiceOver is known to drop announcements. The Tab key stops only on the one canvas |
| F2 | Give real focus to the mirror elements, and relax eframe's `has_focus` to "is activeElement inside the canvas's parent container" | **Needs an eframe fork** (a few lines). Assistive technology behaves naturally |

**Flutter takes the F2 side (real DOM focus + `tabindex="0"`) and does not use `aria-activedescendant`** (1.4). Following precedent, F2 is the main candidate, and F1 is a stalking horse to measure "how far can we get without a fork". **Build F1 first, check with VoiceOver, and fall back to F2 if it fails.** And if F2 turned out to be needed, that becomes the content of the "upstream proposal to eframe". The shape to send upstream is not "open an `accesskit` hook on `WebOptions`" but **eframe owning the adapter**. eframe owns the canvas, the text agent, and focus, so putting it inside eframe is shorter than plugging in from outside. Even when proposing a hook, note that relaxing `has_focus` is needed with it.

### 1.4 The Flutter web semantics layer

The only precedent running the same approach (canvas drawing + DOM mirror) in production. After the engine merge, it now lives at [`engine/src/flutter/lib/web_ui/lib/src/engine/semantics/`](https://github.com/flutter/flutter/tree/master/engine/src/flutter/lib/web_ui/lib/src/engine/semantics). **28 files, about 280 KB of Dart** (`semantics.dart` alone is 3,490 lines). Files are split per role: `checkable.dart` / `focusable.dart` / `incrementable.dart` / `label_and_value.dart` (24 KB) / `link.dart` / `live_region.dart` / `menus.dart` / `route.dart` / `scrollable.dart` / `table.dart` / `tabs.dart` / `tappable.dart` / `text_field.dart` (17 KB) / `platform_view.dart`, and so on. The task.md estimate "production quality is several thousand lines" refers to this scale, and it is right.

**DOM shape** (`semantics.dart` and [`view_embedder/dom_manager.dart`](https://github.com/flutter/flutter/blob/master/engine/src/flutter/lib/web_ui/lib/src/engine/view_embedder/dom_manager.dart)).

```
<flutter-view>
  <flt-glass-pane> -> #shadow-root { <flt-semantics-placeholder>, <flt-scene-host><flt-scene>(canvas) }
  <flt-text-editing-host>
  <flt-semantics-host>     <- appended last. "Because it must come first in hit-test order"
```

- **The mirror is a sibling of the canvas, at the end of the DOM.** This is the same placement as eframe's text agent, which sits as a sibling of the canvas, and having a separate `<flt-text-editing-host>` is also just like it.
- One `<flt-semantics id="flt-semantic-node-N">` per node (`SemanticRole.createElement`, line 724). `position: absolute` + `overflow: visible` (`_initElement`, line 727), size is the rect in px, position is `transform-origin: 0 0 0` + a CSS `transform` matrix. Children use coordinates relative to the parent's rect minus the scroll offset (`recomputePositionAndSize` / `recomputeChildrenAdjustment`).
- **The host absorbs the DPR.** `<flt-semantics-host>` gets `position:absolute; left:0; top:0; transform-origin:0 0 0; transform: scale(1/devicePixelRatio)`. The comment says "the framework emits semantics in physical px, but CSS uses logical px". **We use the same trick in 2.1** (egui's tree also has `Affine::scale(pixels_per_point)` on the root, so one host can divide it back, and no per-node division is needed).
- **We can learn how to make it transparent.** `filter: opacity(0%)` and `color: rgba(0,0,0,0)` on **the root node only**. The comment gives the reason: "Use `filter` instead of the `opacity` attribute. `filter` is stronger, and there are elements where `opacity` does not work, such as the thumb / track of iOS sliders" and "Use transparency instead of `visibility: hidden` / `display: none`, so that screen readers do not ignore these elements". **The host CSS in 2.1 follows this.**
- **Real elements are used where they help.** link → `<a href>`, heading → `<h1>` to `<h6>` (margin / padding 0, font-size 10px), form → `<form>`, text fields → child `<input>` / `<textarea>`, slider → child `<input type=range role=slider>`. On the other hand, **checkbox / radio / switch are not real `<input>`s**; they are built with `role` + `aria-checked`. Neither "all divs" nor "all real elements".
- **`role="application"` appears nowhere** (0 hits in the whole repository). This is the opposite decision from `web-basics`, which put it on the root.
- There are three ways to expose a label (`LabelRepresentation` in `label_and_value.dart`). `ariaLabel` (cheapest, but "does not work for most web crawlers and JAWS on Windows") / `domText` (a text node. button / link / heading) / `sizedSpan` (a `<span>` stretched to actual size with a CSS transform. The screen reader focus frame matches the widget rect. Most expensive). The lesson here is that **`aria-label` alone is not enough**.
- `pointer-events` has three levels (`acceptsPointerEvents`, line 684). `all` = interactive or `SemanticsHitTestBehavior.opaque` / `none` = `transparent` or a container with children / `auto` = a non-interactive leaf with `defer` (leaves overlap resolution to the browser's z-index). With 2 or more children, the hit-test order is built by reversing z-index (DOM order is used for reading order, so the two orders are held separately).
- Announcements do not put `aria-live` on nodes. They are gathered in `<flt-announcement-host>` (polite / assertive) directly under `document.body`. This is the shape needed when doing egui#2647 (live regions) on the web.
- For debugging, `debugShowSemanticsNodes` draws a green `outline` (`border` is not used because it moves layout). Putting the same mechanism into the prototype first speeds things up.

**Focus and event round trip** (`focusable.dart` / `tappable.dart` / `pointer_binding.dart`). This is the actual handling of "focus is not reliable" from 1.1, so it is worth copying as-is.

- **framework → DOM**: `changeFocus()` **delays** `focusWithoutScroll()` **until the post-update callback**. And **it never calls `blur()`** ("blurring an element is very error-prone"). Setting the same value again is swallowed with `_lastSetValue`. **The `focus_moved` in `web-basics` calls `blur()`** (table in 1.1), so drop that when porting.
- **DOM → framework**: `tabindex="0"` + `focus` / `blur` listeners send `SemanticsAction.focus`, but **not if it is the focus we just requested ourselves** (`_lastEvent == requestedFocus`). This stops the echo infinite loop.
- **Click**: `ClickDebouncer` buffers events from `pointerdown`, and either flushes them after 200ms or turns them into a `tap` when a `click` arrives ("screen readers synthesize clicks much faster than 200ms"). A `click` right after a flush within 50ms is dropped. **This is a time window to tell synthesized clicks apart from real pointer operations**; without it you get double firing (#130162).
- **Scroll**: `role=group` + DOM `scroll` event → `SemanticsAction.scrollToOffset`. One overflowing child is inserted to fake the scroll range.
- **Route transitions**: "web screen readers do not do it for you", so the engine itself focuses the first focusable descendant.

**Enabling** (`semantics_helper.dart`). By default the semantics layer is not built. It is built after detecting that assistive technology is present, because keeping it always on is expensive.

- Desktop (`DesktopSemanticsEnabler`): places `<flt-semantics-placeholder role="button" aria-live="polite" tabindex="0" aria-label="Enable accessibility">` outside the window with `position:absolute; left:-1px; top:-1px; width:1px; height:1px`, and enables semantics **when it is clicked**. The comment says "do not enable just because Tab entered it. Make them actually click".
- Mobile (`MobileSemanticsEnabler`): stretches the same placeholder over the full screen, and enables with the heuristic **"if the click coordinate is exactly the center of the placeholder, interpret it as a click sent by a screen reader"** (line 331).
- **The placeholder is still alive as of 2026**, and it is still off by default ([web accessibility on docs.flutter.dev](https://docs.flutter.dev/ui/accessibility/web-accessibility): "For performance reasons, web accessibility is not on by default. To enable it, the user must press the invisible button with `aria-label="Enable accessibility"`"). Automatic screen reader detection is **not** included. The escape is `SemanticsBinding.instance.ensureSemantics()`, which enables it explicitly from the app.
- Flutter 3.32 (2025-05) made semantics construction about 80% faster and cut web frame time by about 30%, but **it still has not reached on-by-default**. "The mirror is expensive" is still a fact.

**Known weak points.** The content of "Flutter web has not solved it either" is also exactly the holes we will step in.

| | What |
|---|---|
| Focus ring is invisible | The mirror is transparent, so you cannot see which node is focused ([#186044](https://github.com/flutter/flutter/issues/186044), open) |
| The mirror eats the mouse | Calling `ensureSemantics()` breaks `GestureDetector`'s `Tap*Details` ([#188859](https://github.com/flutter/flutter/issues/188859)); on mobile web, the semantics onTap also hits elements behind it ([#160560](https://github.com/flutter/flutter/issues/160560)) |
| The browser collapses nodes on its own | Safari implicitly merges bare text nodes with no role ([#166787](https://github.com/flutter/flutter/issues/166787)) |
| Tab order | Not natural when embedded in an iframe ([#162871](https://github.com/flutter/flutter/issues/162871)) |
| Text fields | `excludeSemantics` breaks the `aria-label` of `<input>` ([#172206](https://github.com/flutter/flutter/issues/172206)). The reason TextField handling is 17 KB |
| Find in page | Ctrl+F cannot search the body ([#65504](https://github.com/flutter/flutter/issues/65504), open since 2020). The cause is clear: **"because of lazy rendering, the engine only knows the part of the UI visible right now" and "there is no line back from layers or pictures to widgets"**. egui is the same (even more so, since it is immediate mode) |
| Translation | Page translation does not work ([#131984](https://github.com/flutter/flutter/issues/131984)) |
| SEO | Not indexed ([#46789](https://github.com/flutter/flutter/issues/46789), open since 2020). a11y is listed as a prerequisite |
| Performance | Building the mirror and repositioning on scroll are expensive (#163204, #159358). Still not on by default even after the 3.32 improvements |

Find in page / translation / SEO are what task.md put in "Out of scope" from the start, and this confirms that decision was right. **The fact that many bugs are of the kind "enabling it breaks app behavior"** is also worth remembering as a trait of this approach (double tap firing #147050 / #153924, punching through overlapping elements #163576, cannot type #129324, dialogs closing on their own #149001).

### 1.5 Other canvas-drawn UIs

| | a11y handling |
|---|---|
| Google Docs ([moved](https://workspaceupdates.googleblog.com/2021/05/Google-Docs-Canvas-Based-Rendering-Update.html) from DOM drawing to canvas in 2021) | Places an **invisible SVG overlay** as a sibling of the canvas, and lines up `<rect aria-label="..." x y width height transform>` over the text (the so-called "annotated canvas"). Off by default; it appears only when an allowed extension sets `window['_docs_annotate_canvas_by_ext']` ([Chromium's `gdocs_script.js`](https://chromium.googlesource.com/chromium/src/+/main/chrome/browser/resources/chromeos/accessibility/common/gdocs_script.js)). For third-party screen readers, a **separate off-screen DOM** is shown via a user setting. **The shape differs, but the skeleton "canvas + invisible DOM with coordinates" is the same as Flutter** |
| makepad | The web runner's `CxOsOp::AccessibilityUpdate(_) => {}` is **empty** (`platform/src/os/web/web.rs`). [makepad#196 "On Accessibility"](https://github.com/makepad/makepad/issues/196) has been open since 2023 |
| Bevy | `bevy_a11y` builds the AccessKit tree even on the web, but the receiver `accesskit_winit` is **an adapter that does nothing on wasm32**, so nothing reaches the web. Exactly the same shape as egui |
| Slint | The official docs [state](https://docs.slint.dev/latest/docs/slint/guide/platforms/web/) that "accessibility features such as screen readers are not available on the web". The floers prototype in 1.1 was built with Slint, and **it stalled trying to fill this hole** |
| iced | No web a11y. [iced#552](https://github.com/iced-rs/iced/issues/552) has been open since 2020, and the intermediate PRs target native |
| Dioxus | Draws a real DOM on the web, so this problem does not exist (as task.md said: "if you want that, use Dioxus") |
| HTML canvas fallback content | The spec for reading `<canvas>` child elements as an alternative ([HTML Standard 4.12.5](https://html.spec.whatwg.org/multipage/canvas.html#the-canvas-element)) and `drawFocusIfNeeded()` ([4.12.5.1.14](https://html.spec.whatwg.org/multipage/canvas.html#drawing-focus-rings-and-scrolling-paths-into-view)). **Not enough.** Fallback elements have no box, so they **have no position**, and magnifier or braille display routing does not work. `addHitRegion`, which was supposed to handle coordinates, was removed from the spec in 2016, and `scrollPathIntoView` in 2024, as "no browser implemented it". The Chromium canvas owner wrote in [whatwg/html#7490](https://github.com/whatwg/html/issues/7490#issuecomment-1039495724) that "many apps that draw interactive UI on canvas (such as the cell grid in Google Sheets) implement accessibility with **invisible DOM elements that are not fallback**". MDN also says "canvas content is not exposed to accessibility tools. In general, canvas should be avoided". The future solution is [WICG HTML-in-Canvas](https://github.com/WICG/html-in-canvas) |

**Conclusion: there are two precedents, Flutter web and Google Docs, and both arrived at "canvas + invisible DOM with coordinates". That the standard canvas fallback cannot be relied on is also backed by the spec removal history and implementer statements.** On the Rust side, egui / Bevy / Slint / iced / makepad all stop at the same place (they can build an AccessKit tree, but there is no adapter to expose it on the web), so **one web adapter would make all of them work at once**. That is where the value of going upstream lies.

### 1.6 Related issues on the egui / eframe side

| | What |
|---|---|
| [emilk/egui#167](https://github.com/emilk/egui/issues/167) | The original a11y request. Closed. Closed by the AccessKit integration ([PR#2294](https://github.com/emilk/egui/pull/2294)) |
| [emilk/egui#2391](https://github.com/emilk/egui/issues/2391) | Fix the a11y wording in the README. The wording "platforms without AccessKit support, including web, have an experimental built-in screen reader" was proposed, and it was closed |
| [emilk/egui#2647](https://github.com/emilk/egui/issues/2647) | Wants live regions. **Open.** Needed on both web and native |
| [emilk/egui#7679](https://github.com/emilk/egui/pull/7679) | An RFC to let apps build their own AccessKit subtree under a `Ui`. Closed / not merged |
| [emilk/egui#8410](https://github.com/emilk/egui/pull/8410) | Attach adapters to viewports other than the root. Open |

There is **no issue in egui or eframe** that deals with web a11y itself. The `accesskit_update: _` comment is the only record. The upstream issue will be new.

## 2. Design

### 2.1 `accesskit_web` (adapter crate, independent of egui)

As decided in task.md, it looks only at AccessKit's `TreeUpdate`. It knows nothing about egui or eframe. It inherits the `web-basics` layout as-is.

```
accesskit_web/
  src/lib.rs      pub use adapter::Adapter
  src/adapter.rs  Adapter: State { Pending | Active { tree, host, elements } }
  src/node.rs     NodeWrapper: Role -> ARIA role, attributes, coordinates
  src/filters.rs  accesskit_consumer::common_filter
```

```rust
pub struct Adapter { /* .. */ }

impl Adapter {
    /// Create a hidden host under `parent`. Put it in the same containing block as the canvas.
    pub fn new(
        parent: &web_sys::Element,
        activation_handler: impl ActivationHandler,
        action_handler: impl 'static + ActionHandler,
    ) -> Self;

    /// Once per frame. Do not call on discarded passes.
    pub fn update_if_active(&mut self, update: impl FnOnce() -> TreeUpdate);

    /// Whether the host (canvas) has focus.
    pub fn update_host_focus_state(&mut self, is_focused: bool);

    /// Align the mirror with the canvas position and scale.
    pub fn set_viewport(&mut self, offset: (f64, f64), pixels_per_point: f64);
}
```

What changes from `web-basics`.

- **Add coordinates.** Read `Node::bounding_box()` (consumer 0.38, `node.rs:315`) and write `position: absolute` + `left/top/width/height` in px. egui's tree has `Affine::scale(pixels_per_point)` on the root node (`egui/src/context.rs:525`), so `bounding_box()` comes out in physical px. **Do not divide per node. Apply `transform: scale(1/pixels_per_point)` once on the host to get back to logical px** (the same as what Flutter does with `<flt-semantics-host>`, 1.4).
- **Host CSS follows Flutter** (1.4). `position: absolute` + the same rect as the canvas; children get `position: absolute` + `overflow: visible`. Transparency is via **`filter: opacity(0%)` and `color: rgba(0,0,0,0)`**; do not use `visibility: hidden` / `display: none` (screen readers ignore the whole element). We use `filter` rather than the `opacity` attribute because some elements ignore it. Default is `pointer-events: none` (mouse passes through to the canvas), and only elements that take F2 are raised to `auto`. Include a debug flag from the start that draws `outline: 1px solid green` (do not use `border`, it moves layout).
- **DOM → `ActionRequest`.** This is the part missing entirely from `web-basics`. Attach listeners per element and feed `ActionHandler::do_action`.

  | DOM event | `ActionRequest` to send |
  |---|---|
  | `click` | `Action::Click` |
  | `focus` (focus moved on the DOM side) | `Action::Focus` |
  | `keydown` Enter / Space (button-like role) | `Action::Click` |
  | `input` / `change` (slider / spinbutton role) | `Action::SetValue` + `ActionData::NumericValue` |
  | `change` (checkbox / switch role) | `Action::Click` (egui receives toggle as Click) |

  `target_tree` is `TreeId::ROOT`, `target_node` is the `NodeId` tied to the element. floers reached the same conclusion (1.1): `ActionHandler` is not something the adapter implements; it is **the outlet the adapter calls to send things out**.

  **Watch out for double click firing.** Both the `click` a screen reader synthesizes and a real pointer press on the canvas by the user can reach the same widget. Flutter tells them apart with a 200ms / 50ms time window in `ClickDebouncer` (1.4). In our case the mirror defaults to `pointer-events: none`, so the split "a DOM `click` arrived = it was synthesized by assistive technology" works from the start. Only elements raised to `auto` for F2 get the same problem, so think about it then.
- **Element kinds.** Follow Flutter and use real elements where they help. `Role::Slider` → `<input type="range" role="slider">`, `Role::TextInput` → `<input>`, `Role::Link` → `<a href>`. A checkbox can be a `<div>` with `role="checkbox"` + `aria-checked` (Flutter does the same). Do not rely on `aria-label` alone (`LabelRepresentation` in 1.4. `aria-label` does not work for JAWS or crawlers). Start with `aria-label`, and switch to text nodes if VoiceOver is not satisfied.
- **Tab order.** `tabindex="0"` if `is_focusable(&filter)` is true (`web-basics` fixes it at `-1`). Tree order is DOM order, so Tab order becomes egui's widget order. If we take F1, set the whole host to `tabindex="-1"` and make only the one canvas a Tab stop.
- **The filter stays `accesskit_consumer::common_filter`.** `common_filter` in 0.38 already does "if the parent clips children, my rect is outside the parent, and the siblings before and after are also outside, exclude the subtree" (`filters.rs:54`). Rows that went outside a `ScrollArea` do not appear in the mirror either. It keeps one row before and after right after scrolling, so assistive technology has a foothold to send `ScrollIntoView`.
- **Use the role table mostly as-is.** The 150-line `match` is valid apart from new `Role`s added to accesskit. `web-basics` set `Role::Window` (egui's root node) to `application` (the last commit on the branch is exactly "Set application role on the root node"), but `role="application"` turns off the screen reader's browse mode, so **decide after checking with VoiceOver**. Related: Flutter reports that Safari merges bare text with no role on its own ([#166787](https://github.com/flutter/flutter/issues/166787)), so set an explicit role on `Role::Label` too.

Target widgets are as in task.md: Button / Checkbox / Label / TextEdit / Slider / ComboBox. **Only TextEdit is a special case.** What egui receives via `ActionRequest` is `Action::SetValue` (Slider / DragValue only, `widgets/slider.rs:755`) and `Action::SetTextSelection` (`text_selection/cursor_range.rs:188`); **there is no path to receive text input itself**. On the web, eframe's text agent (a hidden `<input>`) already receives the keyboard, so the mirror of a TextEdit node stays a `role="textbox"` that "gets the name and value read out", and input is left to the text agent (focus is handed to the text agent).

### 2.2 How the runner receives `accesskit_update`

There are three options.

| Option | How | Verdict |
|---|---|---|
| A | Fork eframe, add `accesskit_sink: Option<Box<dyn FnMut(TreeUpdate)>>` to `WebOptions`, and call it from `handle_platform_output` | Needs a fork. Cannot go into main until it lands upstream |
| B | Write our own web runner (`egui::Context` + `egui-wgpu` + all the input bridging by ourselves) | We would rewrite the text agent / IME / touch / resize / storage. eframe web is 15 files. **Not taken** |
| C | **Pick it up with egui's `Plugin::output_hook`** (1.2 (b)) | Works with crates.io eframe / egui as-is. The same code runs on native and under kittest |

**We take C.** As 1.2 found, this needs neither a fork nor our own runner. The prototype can start with 2 lines from `Options::setup` in `egui-reactor-app`.

```rust
// gallery/src/main.rs on the spike branch
Options {
    setup: Some(Box::new(|cc| {
        cc.egui_ctx.enable_accesskit();
        cc.egui_ctx.add_plugin(egui_reactor_app::a11y::WebA11y::new("egui_reactor_canvas"));
    })),
    ..Default::default()
}
```

`WebA11y` is thin glue on the `egui-reactor-app` side (`#[cfg(target_arch = "wasm32")]`). It holds `accesskit_web::Adapter` in a `thread_local!` and only wires up the two `Plugin` holes.

```rust
impl egui::plugin::Plugin for WebA11y {
    fn debug_name(&self) -> &'static str { "egui_reactor_web_a11y" }

    fn output_hook(&mut self, _ctx: &egui::Context, output: &mut egui::FullOutput) {
        if output.platform_output.requested_discard() { return; }   // the discarded pass from 5.3
        let Some(update) = output.platform_output.accesskit_update.take() else { return; };
        with_adapter(self.key, |a| a.update_if_active(|| update));
    }

    fn input_hook(&mut self, _ctx: &egui::Context, input: &mut egui::RawInput) {
        with_pending_actions(self.key, |req| {
            input.events.push(egui::Event::AccessKitActionRequest(req));
        });
    }
}
```

**One more reason not to take B.** ARCHITECTURE section 8 says "the library core touches only `&mut egui::Ui`, so platform support is closed inside the runner layer". Our own runner would fatten that enclosure for the web only, and the situation differs from the iOS runner (section 7) (on iOS there is no choice, since eframe does not support it). The web has eframe.

**The shape to send upstream is not A but "eframe owns the adapter".** As in 1.3, focus handling lives inside eframe, so just adding a callback does not let callers write it correctly. C is "an external attachment until upstream lands", not the upstream proposal itself.

### 2.3 The focus round trip

**This is the only place in this task where "nobody has the answer".** The AccessKit maintainer stopped at the `web-basics` prototype with "focus movement is not reliable" (1.1), and Flutter barely keeps it running with three tricks (delayed focus / never calling blur / echo suppression) (1.4). We consider the prototype's deliverable to be the measurements here.

F1 / F2 from 1.3. Implementation order is F1 → measure → F2 if needed.

- egui → DOM: in `TreeChangeHandler::focus_moved`, for F1 swap the canvas's `aria-activedescendant`, for F2 call `element.focus()`. **Follow Flutter on three points**: (a) call `focus()` only after all tree updates are applied, (b) **never call `blur()`** (`web-basics` does), (c) swallow re-setting the same node.
- DOM → egui: the element's `focus` event (F2), or catch Tab on the canvas's `keydown` and decide the next node ourselves (F1). Both send `Action::Focus`. egui receives this in `Memory` and puts it in `id_requested_by_accesskit` (`egui/src/memory/mod.rs:610`). **Do not send back the `focus` event for the focus we just requested ourselves** (echo causes an infinite loop. Same as Flutter's `_lastEvent == requestedFocus`).
- Host-wide focus: pass eframe's "does the canvas or the text agent have focus" to `Adapter::update_host_focus_state(is_focused)` every frame. From the plugin it can be read with `ctx.input(|i| i.focused)`.

### 2.4 When to enable it

`ctx.enable_accesskit()` builds an `accesskit::Node` for every widget every frame, so we do not keep it always on. Flutter web decides by "when the 1px `<button aria-label="Enable accessibility">` outside the window is clicked" (1.4).

**The prototype keeps it always on.** The goal is to confirm "is it usable from assistive technology", and checking the enabling protocol at the same time would make it impossible to isolate causes. Leave the enabling design as a discussion point for going upstream. AccessKit's `ActivationHandler` (`request_initial_tree`) is already a hole of that shape, so align the adapter API with it (2.1). Adding `Options.a11y: bool` on the egui-reactor side can wait until the upstream shape is decided.

### 2.5 Label hardening that goes into main

Works with or without the adapter. **An "unnamed widget" cannot be read out even with a mirror**, so fix them first.

After checking the egui 0.36 API, the needed tools are all there.

- `egui::Image::alt_text(impl Into<String>)` goes into `WidgetInfo.label` and becomes the AccessKit node's `label` as-is (`egui/src/widgets/image.rs:272, 407`). It is also drawn next to the ⚠ placeholder on error.
- To override any widget's name afterward, use `Context::accesskit_node_builder(id, |node| node.set_label(..))` (`context.rs:3681`, public. Returns `None` if accesskit is disabled). `Response::widget_info` is another way, but it pushes one more `OutputEvent` on the frame you click, so we do not take it.

Only two changes.

| Element | Prop to add | Implementation |
|---|---|---|
| `Image` | `alt: Option<&str>` | `image = image.alt_text(alt)` |
| `Button` | `label: Option<&str>` | `ui.ctx().accesskit_node_builder(resp.id, \|n\| n.set_label(label))`. children stay as drawn |

`Checkbox` / `Slider` / `ComboBox` already have `label: Option<&str>`. The real problem is places that have it but do not pass it, such as the inline checkbox in `examples/todo` (`examples/todo/src/lib.rs:118`. The name is empty). The `x` button (line 124 of the same file) is also read only as "x". Fix the examples, and **block the same hole from coming back with kittest** (section 4).

The sentences to add to ARCHITECTURE / README go like this.

- Non-goals in ARCHITECTURE section 1: "Web screen reader support depends on web support in egui / AccessKit. egui emits the widget tree to AccessKit, and on native it reaches the OS accessibility API, but on the web there is no upstream adapter that mirrors it into the DOM (`docs/tasks/a11y/`)."
- ARCHITECTURE section 8 (platforms): one line saying the same thing, pointing at `accesskit_update: _` in `eframe/src/web/app_runner.rs`.
- The gallery section of the README: "The gallery draws on a canvas, so on the web it cannot be read by screen readers. On native it can."

## 3. Steps

### PR #4 (main, done)

1. **Fix this document and task.md.** Reflect the existence of the `web-basics` branch found in 1.1 in the task.md background table ("none" → "no released version. There is one prototype branch"). Commit.
2. **`alt` on `Image`, `label` on `Button`.** `crates/egui-reactor-elements/src/widgets.rs`. Two kittests in `tests/widgets.rs` (A-1, A-2). Commit.
3. **Fill in labels in examples.** The todo checkbox and `x` button, and go through the other examples to fix unnamed widgets. Retake snapshots if they run. Commit.
4. **A kittest that finds unnamed nodes** (A-3). Draw every gallery example one by one, and check that focusable nodes have a non-empty name. Commit.
5. **One sentence each in ARCHITECTURE sections 1 / 8 and the README.** In the README, add to the end of the gallery paragraph in the "Examples" section (`Every example but one runs in the browser ..`). Commit. PR.

### PR #5 (main, the follow-up)

As in section 0, this does not need a fork, so it goes into main.

6. **Port the `accesskit_web` skeleton.** Put the 4 `web-basics` files in `crates/accesskit-web/` and fix them for accesskit 0.24.1 / consumer 0.38 (`name()` → `label()`, `is_focusable(&filter)`, `TreeId`). No coordinates or events yet. Only check that it builds.
7. **Coordinates and host CSS** (2.1). Turn `bounding_box()` into `position: absolute` and overlay the canvas. Check by eye in DevTools that the rects sit on top of the widgets.
8. **`egui_reactor_app::a11y::WebA11y` (plugin) and startup from `Options::setup`** (2.2). Build the gallery with trunk, and check by eye that the DOM updates every frame. Check that the condition that rejects discarded passes really works, using an example where taffy runs 2 passes (layout).
9. **DOM → `ActionRequest`** (table in 2.1). Up to the point where click and Enter / Space increment the counter's `+`.
10. **Focus F1** (2.3). The `aria-activedescendant` version. Try counter / todo / form with VoiceOver (macOS Safari / Chrome).

### `a11y-spike` branch (after PR #5)

Branch from `main`. The forked eframe is closed inside this branch.

11. **F2 if F1 is not enough.** Fork eframe and relax `has_focus` to "is activeElement inside the canvas's parent". Point to the git fork with `[patch.crates-io]`. **This commit lives only on the spike branch.**

### Upstream

12. **Upstream.** Write to AccessKit as a follow-up to discussions#514: "we revived web-basics on the current API and added coordinates and events. Can you take it?". Open an issue on eframe / egui: "the AccessKit tree is discarded on the web. We propose eframe owning the adapter".

## 4. Tests / verification

### main (headless, runs in CI)

kittest walks the AccessKit tree as-is (`egui_kittest::Harness::root()` returns a `kittest::AccessKitNode` = `accesskit_consumer::Node`). All of label hardening can be covered headless.

| # | Where | What |
|---|---|---|
| A-1 | `crates/egui-reactor-elements/tests/widgets.rs` | `<Image alt="a cat"/>` can be found with `harness.get_by_label("a cat")`. Without `alt` it cannot |
| A-2 | Same file | `<Button label="delete">"x"</Button>` can be found with `get_by_role_and_label(Role::Button, "delete")`. The look (the rect that `get_by_label("x")` finds) does not change |
| A-3 | `examples/gallery/tests/` | Loop over `gallery::EXAMPLES`, draw `<App start={meta.name}/>`, walk the tree from the root, and check there is no node that is "focusable and `label()` is empty". If found, print the example name, role, and rect. **This is the regression guard for step 3** |

A-3 is a mechanism so that "if someone adds an example later and forgets the name, it goes red", so the failure message says "please pass `label`". The window is the same 1280x1000 as the existing `examples/gallery/tests/gallery.rs` (widgets a `ScrollArea` did not draw are not in the tree either, so a small window misses them).

### web (checked by hand)

Automated wasm tests with `wasm-bindgen-test` can cover the DOM shape (node count, `role`, `aria-label`, rects), but **how assistive technology actually reads it cannot be automated**. VoiceOver behavior is everything, so we keep a checklist and run it by hand.

- `wasm-bindgen-test` (headless Chrome): build a `TreeUpdate` by hand, feed it to `Adapter`, and check that the expected elements, attributes, and coordinates line up in the DOM. Check that nodes are added and removed by diff updates. The adapter is independent of egui, so egui does not appear in this test.
- VoiceOver checklist (macOS, both Safari and Chrome. counter / todo / form in the gallery).

  **Status: run (2026-09-05, macOS). All 8 items passed. F1 works, and step 11 (F2, eframe fork) is not needed.** For item 1, at first I pressed `Cmd+Option+→`, which only switched browser tabs; it worked with the VO keys (`Ctrl+Option`). The canvas is `role="application"`, so if the VO cursor does not descend into it, `VO+Shift+↓` goes in one level. Browser versions were not recorded.

  | | What to check | Result |
  |---|---|---|
  | 1 | VO+Right reads widgets in order, and button names are read | Pass |
  | 2 | Tab moves focus, and the VoiceOver cursor follows | Pass |
  | 3 | Enter / Space presses the button, and the result (counter value) is read | Pass |
  | 4 | The checkbox "on/off" is read and can be toggled | Pass |
  | 5 | Text can be typed into the text field, and the typed value is read | Pass |
  | 6 | The slider value is read and can be moved with arrow keys | Pass |
  | 7 | The mirror does not get in the way of mouse operation (`pointer-events`. The hole Flutter stepped in with [#188859](https://github.com/flutter/flutter/issues/188859) / [#160560](https://github.com/flutter/flutter/issues/160560)) | Pass |
  | 8 | The focused node is **also** visible **by eye** (egui's focus ring shows). Flutter web does not show this because of the transparent mirror, and [#186044](https://github.com/flutter/flutter/issues/186044) is still open | Pass |

  Record a video of the read-out and attach it to the upstream issue.

## 5. Mapping to the done criteria

task.md has three done criteria. The third was done in PR #4, the first is step 10 (11 if needed), the second is step 12.

| Done criterion | Where |
|---|---|
| counter / todo / form in the gallery (web) read by VoiceOver, Tab, Enter / Space | Step 10 (11 if needed), the checklist in section 4 |
| A proposal for a web adapter and an eframe hook is submitted upstream | Step 12 |
| ARCHITECTURE.md describes the current state and policy for web a11y | Step 5 (**done in PR #4**) |

## 6. Differences found during implementation

Steps 2 to 5 (PR #4, label hardening) are 6.1 to 6.5, steps 6 to 9 (PR #5, the mirror itself) are 6.6 to 6.9, step 10 (focus F1) is 6.10.

### 6.1 `label` on `Button` is as in section 3. `alt` on `Image` is the same

`ui.ctx().accesskit_node_builder(response.id, |node| node.set_label(label))` works as-is on egui 0.36.1 (`Context::accesskit_node_builder(Id, impl FnOnce(&mut accesskit::Node) -> R) -> Option<R>`). Call it after the widget writes its own node and it overrides the name; if accesskit is disabled it just returns `None` and nothing happens. What is drawn does not change, so even in examples that sit next to a raw egui version, no pixel moves.

### 6.2 Widgets that cannot be named remain

Writing A-3 (`examples/gallery/tests/a11y.rs`) and running all examples produced 13 focusable nodes with empty names. **None of them can be named "without changing what is drawn" with the current element API.**

| What came up | Why it cannot be named |
|---|---|
| `TextInput` (showcase / todo / form / custom-hook / list-10k) | `egui::TextEdit` writes no label at all on the AccessKit node. `hint_text` only goes to `PlatformOutput` (the read-out text for the web screen reader) and is not on the node. `<TextEdit>` has no name prop either |
| `CheckBox` x2 / `Slider` / `SpinButton` / `ComboBox` in `form` | The label is drawn by the neighboring column (`<Field>`). In AccessKit this is a `labelled_by` relation, and the elements do not emit `Response::labelled_by`. The `label` prop each element has **draws text**, so the name would appear twice on screen and diverge from the raw egui version next to it |
| `ColorWell` in `escape-hatch` | Raw `ui.color_edit_button_srgba`. egui gives it no name |
| `Unknown` in `shader` | The leaf of `<Canvas>`. It is a drawing surface, not a control, so wait until we decide how to treat canvas in general |

So A-3 is not "zero focusable nodes with empty names". Instead **it lists these 13 as `KNOWN_UNNAMED` and compares by exact match**. It fails if unnamed widgets increase, and also fails when a known one is fixed (if you forget to remove it from the table). The list only moves in the shrinking direction.

What is needed as follow-up. Both grow the element API, so they do not go into this PR.

1. Add a "name that is not drawn" to `<TextEdit>`, like `Button`. The 5 above go away.
2. A way to connect a "neighboring label" like `<Field>` to AccessKit (a `Response::labelled_by` equivalent). The 5 in form go away.

### 6.3 The inline checkbox in `examples/todo` could not be fixed

The place 2.5 called "the real problem". The only way to give it a name, the `label` prop, draws text, so the row's look changes and diverges from the raw egui version (and the `snapshot` image) that is compared against the same picture. It needs the same "name that is not drawn" as item 1 in 6.2. Note that the default todo starts with an empty list, so this row does not appear in A-3's tree.

The `x` button (the other one in 2.5) was fixed with `label="remove"`. The raw egui version got the same name by hand with `accesskit_node_builder`. This is because the test that drives both with the same steps finds it by label, and because they should be "the same app". The `x` in `list-10k` is the same.

### 6.4 `+` / `-` were left as-is

The buttons in counter and custom-hook. The names are not empty (they are read as "plus" / "minus"), and the number is shown next to them, so the meaning comes through. Fixing them would make only one side diverge in the test that drives the raw egui version and the egui-reactor version with the same steps, so we judged the cost to be greater.

### 6.5 A-3 uses `run_steps(2)`, not `run`

`shader` (and `clock`) request a repaint every frame, so `Harness::run` panics with "did not settle in 4 steps". `fetch` sends a real HTTP request the moment it is drawn, so it is excluded for the same reason as the existing `gallery.rs`.

### 6.6 Step 6: what changed in the port

- In `accesskit_consumer` 0.38, `TreeChangeHandler` / `TreeState` remained as aliases of `ChangeHandler` / `State`, so they could be used as-is. What changes is the `HashMap` key: not `accesskit::NodeId` but **the consumer-side `NodeId` that includes the tree index**.
- `Role::Directory` does not exist in accesskit 0.24, so it was dropped from the role table. The other 150 lines pass as-is.
- `Adapter::new` is **`-> Option<Self>`**, not `-> Self`. This avoids bringing `unwrap()` of `window()` / `document()` into wasm. The parent is also taken as `&Element` rather than an id string (as in 2.1). All `unwrap()`s on `set_attribute` were dropped too (attribute names are constants, so they cannot fail).
- **`focus_moved` was left empty.** `element.focus()` is exactly the wall from 1.3, and `blur()` is not called per the lesson from 1.4. egui → DOM focus is step 10's job.
- `role="application"` is not set on the root host (as deferred in 2.1). egui's root node appears one level down as `role="window"`.
- **`web-sys` builds on native too**, so the crate was left in place without `cfg`-gating the whole thing. The workspace's `clippy --all-targets` and `test` see accesskit-web too.
- clippy complained with `large_enum_variant`, so the `Tree` in `State::Active` was put in a `Box`.

### 6.7 Step 7: coordinates

- **A child's `left/top` had to be relative to the parent's rect.** An absolutely positioned parent becomes the containing block of an absolutely positioned child, so writing `bounding_box()` as-is doubles the offset for each level of nesting. Skip ancestors without a rect and use the nearest "ancestor with a rect" as the origin. This is the same story as Flutter's `recomputeChildrenAdjustment`, and 2.1 had left it out.
- Debug display is `Adapter::set_debug(bool)`. It removes `filter: opacity(0%)` and shows a green `outline`. The plugin side turns it on if the URL has `?a11y-debug`.
- `tabindex` is `"0"` if `is_focusable(&filter)` is true. But this lets Tab enter the mirror, so hitting the wall from 1.3 remains as homework for step 10.
- `Role::Label` gets an explicit `role="paragraph"` (the Safari merge workaround from 1.4).
- **The text of a `Role::Label` is in `value()`, not `label()`** (`Node::label_comes_from_value`. egui writes it that way too). I did not notice until seeing it in the browser, and all the mirror text was empty. I also stopped emitting the same text again in `aria-valuetext`.

### 6.8 Step 8: plugin

- **`requested_discard()` alone is not enough to detect a discarded pass.** On the last pass when `max_passes` is used up, the flag is still set, and dropping it there means the final shape never reaches the DOM. I used `requested_discard() && num_completed_passes < max_passes`, the same as the loop exit condition in `Context::run`. **`ctx.will_discard()` cannot be used**: the plugin is called after `end_pass` has `mem::take`n the viewport's output, so it is always false.
- `Options::setup` is a single `FnOnce`, so in the gallery `shader::gpu::setup(cc)` and `add_plugin` sit in the same closure.
- `accesskit` uses `egui::accesskit` (egui's re-export). The versions cannot drift. `accesskit-web` is a wasm32-only dependency, and the native `WebA11y` is an empty plugin.

### 6.9 Step 9: DOM → `ActionRequest`

- Instead of listeners per element, **one set on the host with delegation**. Elements carry `data-accesskit-node` / `data-accesskit-tree`, and events walk up from there to build the target. This avoids holding one closure per node. `focus` does not bubble, so `focusin` is used.
- `ActionHandler` is shared with the listeners as `Rc<RefCell<Box<dyn ActionHandler>>>`. The plugin-side implementation only pushes to a queue, and `input_hook` feeds it into `RawInput`.
- **`ctx.request_repaint()` is needed.** egui draws only when needed, so just pushing to the queue does not bring the next frame. To egui, a click from assistive technology is the same as "nothing happened". Without this, pressing does nothing, and I did not notice until measuring.
- Of the table in 2.1, `input` / `change` do not actually arrive because the mirror is currently `<div>`s (they are there for when we switch to real `<input>`s). Checkbox toggles arrive via the synthesized `click`.
- **Measured in headless Chrome** (gallery, `?a11y-debug`): `click()` on a mirror button switches the example, and `+` in counter increments the number. `keydown` Enter / Space works too. `elementFromPoint` returns the canvas, not the mirror (`pointer-events: none` works).
- `crates/accesskit-web/tests/mirror.rs` is wrapped in `#![cfg(target_arch = "wasm32")]`, so `cargo test --workspace` passes through it. `wasm-pack test --headless --chrome crates/accesskit-web` passes 4 tests.

### 6.10 Step 10: focus F1, and real elements for value widgets

**There was nothing to write for DOM → egui.** 2.3 said "catch Tab on the canvas's `keydown` and decide the next node ourselves", but measuring showed **eframe and egui already do both**.

- eframe's `keydown` listener is on the canvas (`web/events.rs:83`). It passes Tab to egui as-is as `egui::Key::Tab` and then calls `prevent_default()` (line 269 of the same file; the comment says "egui uses Tab to move focus within the egui app"). The browser never moves focus to the next HTML element.
- On the egui side, `Memory::begin_pass` turns Tab / Shift+Tab into `FocusDirection::Next` / `Previous` (`memory/mod.rs:596`), and `end_pass` picks the widget.

So **the right answer for the DOM → egui direction of F1 is "do nothing"**; writing our own Tab traversal would run twice alongside egui. Step 10b writes not one line on the mirror side, and only makes `aria-activedescendant` follow in `focus_moved`. Echo suppression is not needed either (the mirror never calls `focus()`, so no focus event comes back).

**Two things were missing to make `aria-activedescendant` valid.** On investigation, the referenced element must be either "a DOM descendant of the referencing element" or "an element the referencing element owns via `aria-owns`" (`aria-activedescendant` in WAI-ARIA 1.2, "Developing a Keyboard Interface" in the APG). The mirror host is a **sibling** of the canvas, so I put `aria-owns="<host id>"` on the canvas. Also, this attribute is allowed only on the roles `application` / `combobox` / `composite` (and their derivatives) / `group` / `textbox`, and a bare `<canvas>` is none of them. So **I put `role="application"` on the canvas**. That means the decision deferred in 2.1 as "decide after checking with VoiceOver" was taken here. I confirmed in Chrome's a11y tree that the whole mirror hangs under the canvas (application).

- **An alternative is "make the mirror a child of the canvas"** (canvas fallback content). It becomes a descendant, so neither `aria-owns` nor `role` is needed. I did not take it because, as in 1.5, fallback content elements **have no rendering box**, so the coordinates we added in step 7 would lose their meaning. It remains an option when going upstream.
- **Assistive technology support is thin.** VoiceOver + Safari **ignored `aria-activedescendant` until it was fixed in Safari 18 (2024)** ([WebKit#167680](https://bugs.webkit.org/show_bug.cgi?id=167680)). `aria-owns` does not show up in VoiceOver before macOS 14.3 / iOS 17.3. Touch-based assistive technology on iOS / Android traces the a11y tree directly, so it effectively does not see this attribute. **F1 is only "might work on current Safari"**, and the estimate from 1.3 (F2 is the main candidate) has not changed. No conclusion until a person fills in the table in section 4.

**The three rules from 1.4 were kept as-is.** (a) `focus_moved` **only records**; the attribute is written after `update_and_process_changes` returns. The consumer calls `focus_moved` before `node_removed`, so without recording it could point at a DOM node in the middle of being removed. (b) The equivalent of `blur()` is "removing the attribute when the app reports no focus", so **do not remove it**. (c) Re-setting the same node is swallowed. There is exactly one exception: **when the referenced element is removed from the tree, remove the attribute** (a dangling reference is worse than none).

**`aria-selected` was not added.** The instructions for step 10 had it, but this attribute has meaning only for some roles, like listbox options and grid rows, and putting it on a button would lie to assistive technology. The focus claim is narrowed to the single `aria-activedescendant` on the canvas, and the mirror side only gets a **`data-focused="true"`** marker. With `?a11y-debug`, only the focused node gets a magenta outline (the others stay green).

**`tabindex` was lowered from `"0"` to `"-1"`** (the host is `-1` too). This clears the homework from step 7 (the last line of 6.7). Emitting the attribute on focusable nodes itself is kept, so "which one is the app's focus target" is visible from the DOM.

**Value widgets became real elements** (the hole left in step 9, the 4th point in 6.9).

- `Role::Slider` / `Role::SpinButton` → `<input type="range">`. `min` / `max` / `step` come from the node's numeric values. Without `step`, range rounds to integers, so `any` is set when it is absent.
- `Role::TextInput` → `<input type="text" readonly>`. **readonly is so that the mirror does not take keystrokes**; input is left to eframe's text agent as in 2.1.
- **Values are written as properties, not attributes.** After assistive technology moves the range, the `value` attribute is only `defaultValue`, and the display drifts from egui. It overwrites only when the DOM value differs from the app value.
- The `role` attribute is kept, so the `NUMERIC_ROLES` check written in 6.9 (`input` / `change` → `SetValue`) works as-is. `aria-valuenow` / `valuemin` / `valuemax` are still emitted on all nodes, which serves the roles that stay `<div>`s.
- The checkbox stays `<div role="checkbox">` + `aria-checked`, as in Flutter.
- When a role change crosses `<div>` and `<input>`, the element is recreated and the children are moved over (the tag cannot be changed afterward). egui's node ids are stable per widget, so in practice this almost never happens.

**Measured in headless Chrome** (a `trunk build` of the gallery, served with `?a11y-debug`).

- **counter**: click the canvas, then send Tab, and the canvas's `aria-activedescendant` moves `window` → `showcase` → `counter`, and Shift+Tab goes back. **`document.activeElement` stays the canvas**, and the node with `data-focused` and the referenced element always match. Tab to `+` and press Enter, and the count went from 1 to 2.
- **form**: the mirror slider is `<input type="range" min="0" max="100" step="1">` with value 50. Set `value = 80` and fire an `input` event, and on the next frame the egui side became 80 (the summary line says "volume 80"). TextEdit is `<input type="text" readonly value="anon">`.
- **`elementFromPoint` returns the canvas.** `pointer-events` is inherited, so the host's `none` also applies to the `<input>` (item 7 in section 4, the hole Flutter stepped in).
- **Blurring the canvas keeps `aria-activedescendant`, and so does focusing it again.** Only `data-focused` goes away and comes back (the check for 10d).
- **Only at the moment focus moves to a TextEdit does `document.activeElement` become an `<input>`**, but that is **eframe's text agent**, not the mirror. eframe's `has_focus` counts this as "focused" too (1.3), so no problem arises.
- No errors in the console.

**Remaining holes.**

- **Real VoiceOver passed all 8 items** (section 4). Send it upstream as F1.
- `Role::MultilineTextInput` stays `<div role="textbox">`. Whether to make it a `<textarea>` will be decided after seeing how text fields are read once.
- The `SpinButton` (egui's `DragValue`) next to the slider in form gets a `step` equal to the drag increment (1.12...). That is too coarse for arrow keys, so there is room to decide per role whether to use `numeric_value_step`.
- `aria-owns` reparents the mirror under the canvas, so the order relative to the canvas's own children (if eframe ever places something there) is not guaranteed.

### Step 12: no upstreaming

Withdrawn by the decision on 2026-09-05. Not submitted to AccessKit (discussions#514) or eframe. `crates/accesskit-web` and `egui_reactor_app::a11y::WebA11y` are maintained as part of egui-reactor. If upstream ships an equivalent, we switch to it.
