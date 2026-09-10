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

Use it as `<Card title="inbox" count={3}/>`. The macro generates a props
struct with a builder, so a missing prop is a compile error.

The body's tail expression is what gets drawn. Put branches inside one `rsx!`,
not one `rsx!` per branch. Each `rsx!` is its own closure type.

```rust
rsx! { if open { <Body/> } else { <Placeholder/> } }
```

## Props

| Written as | Meaning |
|---|---|
| `title: &str` | Required |
| `label: Option<&str>` | Optional, `None` when not given |
| `#[prop(default)] style: ItemStyle` | Optional, `Default::default()` when not given |
| `#[prop(default = true)] enabled: bool` | Optional with a default |
| `#[prop(default, into)] direction: Direction` | Setter takes `impl Into<Direction>`, so `direction="row"` works |
| `children: impl View` | The child nodes |

`children` always exists. Without a declaration it is `()`, and the element
must be empty. Declare `children: impl View` to accept a subtree, or
`children: impl Into<egui::WidgetText>` to accept text like `<Button>` does.

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

Take `#[prop(default)] style: ItemStyle` in any component the caller places.
`rsx!` fills it from the caller's `w`, `grow`, `p` and so on, so
`<Panelled grow={1.0}/>` lays out like any element.

## Events

Mark a prop with `#[event]` and it becomes an emitter. The type is the
payload:

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

The caller writes:

```rust
<Dialog title="Quit?" on_ok={|| *open = false} on_cancel={|| *open = false}/>
```

Both handlers borrow `open` as `&mut`. That works because `rsx!` merges every
`on_*` on one element into a single closure that matches on a generated event
enum. A handler may take the payload or ignore it: `on_change={|v: bool| ..}`
and `on_change={|| ..}` are both fine.

### The event enum must be in scope

The merged closure names `DialogEvent::Ok`, so import `DialogEvent` next to
`Dialog`. The elements prelude already exports `ButtonEvent`, `TextEditEvent`
and the rest.

```rust
use crate::components::{Dialog, DialogEvent};
```

A misspelled event name fails to compile.

### `events=`

To handle everything in one place, pass one closure and match yourself:

```rust
<Dialog title="Quit?" events={|e| match e {
    DialogEvent::Ok(_) => *open = false,
    DialogEvent::Cancel(_) => {}
}}/>
```

This is what `rsx!` generates from `on_*` attributes.

### Payloads may borrow

`#[event] on_change: &str` is allowed. The generated enum carries the
lifetime, so `<TextEdit on_submit={|text: String| ..}>` and friends do not
allocate more than they must.

## `shares_ui`

Each element normally gets its own child `egui::Ui`. Some egui containers
cannot live with that: a docked panel carves space out of the parent `Ui`, and
`Grid` rewrites it on row breaks. `#[component(shares_ui)]` keeps a deeper
hook scope but draws into the parent's `Ui`. `Panel`, `CentralPanel`, `Row`
and `Suspense` use it. You only need it when wrapping such a container.

## See it running

[form](/examples/form) is props and events over every bound widget.
[theme](/examples/theme) is a provider component. [notes](/examples/notes)
puts components, events and a reducer together. The generated code is
described in
[docs/ARCHITECTURE.md](https://github.com/fand/egui-react/blob/main/docs/ARCHITECTURE.md#33-components)
sections 3.3 and 3.6.
