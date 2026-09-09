---
title: Components and events
---

# Components and events

## `#[component]`

A component is a function whose first argument is `cx: &mut Cx`. Every other
argument is a prop.

```rust
#[component]
fn Card(cx: &mut Cx, title: &str, count: i32) {
    rsx! {
        <View direction="column" gap={4} p={8}>
            <Text strong>{title}</Text>
            <Text>{format!("{count}")}</Text>
        </View>
    }
}
```

Write it as `<Card title="inbox" count={3}/>`. The macro generates a props
struct (`CardProps`) with a builder, so a missing required prop is a compile
error, not a runtime surprise.

The tail expression of the body is what gets drawn. If different branches
should draw different things, put the branch *inside* one `rsx!` rather than
returning a different `rsx!` per arm — each `rsx!` is its own closure type:

```rust
// yes
rsx! { if open { <Body/> } else { <Placeholder/> } }
```

## Props

| Written as | Meaning |
|---|---|
| `title: &str` | Required |
| `label: Option<&str>` | Optional, `None` when not given |
| `#[prop(default)] style: ItemStyle` | Optional, `Default::default()` when not given |
| `#[prop(default = true)] enabled: bool` | Optional with an explicit default |
| `#[prop(default, into)] direction: Direction` | The setter takes `impl Into<Direction>`, so `direction="row"` works |
| `children: impl View` | The child nodes |

`children` always exists; if you do not declare it you get `children: ()` and
the element must be written self-closing or empty. Declare
`children: impl View` to accept a subtree, or
`children: impl Into<egui::WidgetText>` to accept text the way `<Button>` does.

```rust
#[component]
fn Panelled(cx: &mut Cx, #[prop(default)] style: ItemStyle, children: impl View) {
    rsx! {
        <View style={style} direction="column" gap={8} p={12} bg={egui::Color32::DARK_GRAY}>
            {children}
        </View>
    }
}
```

Taking `#[prop(default)] style: ItemStyle` is worth doing for any component
meant to be placed by its caller: `rsx!` fills that prop from the caller's `w`,
`grow`, `p` and friends, so `<Panelled grow={1.0}/>` lays out like any element.

## Events

Mark a prop with `#[event]` and it becomes an emitter. The type after the colon
is the payload:

```rust
#[component]
fn Dialog(cx: &mut Cx, title: &str, #[event] on_ok: (), #[event] on_cancel: ()) {
    rsx! {
        <View direction="column" gap={8}>
            <Text strong>{title}</Text>
            <View direction="row" gap={8}>
                <Button on_click={|| on_ok.emit(())}>"OK"</Button>
                <Button on_click={|| on_cancel.emit(())}>"Cancel"</Button>
            </View>
        </View>
    }
}
```

The caller writes it like React:

```rust
<Dialog title="Quit?" on_ok={|| *open = false} on_cancel={|| *open = false}/>
```

Both handlers capture `open` with `&mut`, and that is fine: `rsx!` fuses every
`on_*` on one element into a single closure that matches on a generated event
enum, so only one mutable borrow exists. A handler may take the payload or
ignore it — `on_change={|v: bool| *flag = v}` and `on_change={|| *dirty = true}`
are both accepted.

### The event enum has to be in scope

The fused closure names `DialogEvent::Ok`, so wherever you write
`<Dialog on_ok=../>` you need `DialogEvent` imported alongside `Dialog`. Glob
importing the module is the usual answer; the elements prelude already exports
`ButtonEvent`, `TextEditEvent` and the rest for exactly this reason.

```rust
use crate::components::{Dialog, DialogEvent};
```

A misspelled event name has no variant, so it fails to compile rather than
silently doing nothing.

### The `events=` escape hatch

If you would rather handle everything in one place — forwarding a child's
events upward, say — pass a single closure and match yourself:

```rust
<Dialog title="Quit?" events={|e| match e {
    DialogEvent::Ok(_) => *open = false,
    DialogEvent::Cancel(_) => {}
}}/>
```

This is the same thing `rsx!` generates for you from `on_*` attributes.

### Payloads may borrow

`#[event] on_change: &str` is allowed; the generated enum carries the lifetime.
That is how `<TextEdit on_submit={|text: String| ..}>` and friends hand values
back without allocating more than they must.

## `shares_ui`

Normally each element gets a child `egui::Ui` of its own, with `push_id` so
widget ids stay stable. Some egui containers cannot live with that: a docked
panel carves space out of the *parent* `Ui`, and `Grid`'s row breaks rewrite it.
`#[component(shares_ui)]` says "give me a deeper hook scope, but draw into my
parent's surface as-is". `Panel`, `CentralPanel`, `Row` and `Suspense` are
written that way. You need it only when wrapping an egui container with that
property; write your own components without it.

## See it running

[form](/examples/form) is props and events across every bound widget,
[theme](/examples/theme) shows a provider component owning state for its
children, and [showcase](/examples/showcase) puts components, events and a
reducer together.

The generated code — props struct, event enum, fused closure — is described in
[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#33-components)
sections 3.3 and 3.6.
