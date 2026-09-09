---
title: Escape hatches
---

# Escape hatches

egui-react wraps a useful subset of egui, not all of it, and it never has to.
Everything is `&mut egui::Ui` in the end, so anything egui can do is one call
away. None of what follows is a workaround: the elements in
`egui-react-elements` are written with exactly these calls.

| You want | Use |
|---|---|
| ordinary code in the middle of a tree | `{view(\|cx\| ..)}` |
| to draw egui where you are | `cx.leaf(&style, \|ui\| ..)` |
| drawing of your own | `<Canvas>`, or a leaf that allocates a rect and paints |
| hooks inside an egui container's closure | a new `Cx` around the inner `Ui` |

## The one trap

`cx.ui()` is safe to *read* from anywhere — visuals, style, input. But inside a
`<View>` it is the `Ui` the whole taffy tree was started in, not the position
you are at. Draw through it and the widget lands outside the layout, in the
top-left corner of the tree, over whatever is there.

`cx.leaf` is what puts a rectangle in the flow and hands you the `Ui` for that
rectangle. Use it whenever you draw.

## 1. A closure

```rust
rsx! {
    <View direction="column" gap={8}>
        <Text>"above"</Text>
        {view(|cx| {
            // Hooks work in here: this is the same `Cx` the component has,
            // and the slot is keyed by this line.
            let mut opened = use_state(cx, || 0u32);
            let dark = cx.ui().visuals().dark_mode;

            cx.leaf(&ItemStyle::default(), |ui| {
                ui.horizontal(|ui| {
                    ui.small(if dark { "dark" } else { "light" });
                    if ui.link("open the docs").clicked() {
                        *opened += 1;
                    }
                });
            });
        })}
    </View>
}
```

## 2. A leaf

For a widget with no element of its own. The leaf is a layout node, so it takes
the same style attributes as anything else:

```rust
{view(move |cx| {
    cx.leaf(&ItemStyle::default(), |ui| {
        ui.color_edit_button_srgba(colour.bind());
    });
})}
```

`cx.leaf` measures the widget from what it drew and reports that size to the
layout. For a widget that *fills* what it is given rather than reporting a size
— a progress bar, a scroll area — use `cx.leaf_fill` and give it a size, or it
will claim everything:

```rust
{view(move |cx| {
    // Both axes need a size: an axis left `auto` on a filling leaf claims the
    // whole window, because that is what "fill" means with nothing to measure.
    cx.leaf_fill(&ItemStyle::default().w(220.0).h(20.0), |ui| {
        ui.add(egui::ProgressBar::new(value).show_percentage());
    });
})}
```

## 3. A painter

Allocate a rectangle, then draw into it. `<Canvas>` does the allocate-and-hand-
over for you:

```rust
rsx! {
    <Canvas grow={1.0} paint={|ui: &mut egui::Ui, rect: egui::Rect| {
        ui.painter().rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
    }}/>
}
```

The same rect is where a wgpu callback goes
(`egui_wgpu::Callback::new_paint_callback(rect, ..)`), which is how the
[shader](/examples/shader) example gets a fragment shader on screen. `sense`
defaults to `Sense::hover()`; `on_drag` needs `Sense::drag()`.

By hand, the same thing is a filling leaf:

```rust
fn sparkline(ui: &mut egui::Ui, values: &[f32]) {
    let (rect, _response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
    // ..
}
```

Note what that function does *not* take: it is plain egui, and knows nothing
about this library.

## 4. A nested `Cx`

Inside an egui container's closure you have a new `Ui`, so build a new `Cx`
around it and hooks work again:

```rust
let (store, scope) = (cx.store, cx.scope_id());

rsx! {
    {view(move |cx| {
        cx.leaf(&ItemStyle::default(), move |ui| {
            ui.group(|ui| {
                let mut cx = Cx::new(store, ui, scope);
                cx.scope("inner", |cx| {
                    let mut inner = use_state(cx, || 0i32);
                    let value = *inner;
                    let ui = cx.ui();
                    ui.horizontal(|ui| {
                        if ui.button("inner +").clicked() {
                            *inner += 1;
                        }
                        ui.label(format!("inner: {value}"));
                    });
                });
            });
        });
    })}
}
```

`cx.scope("inner", ..)` gives the nested tree an id of its own, so its hooks do
not collide with the outer ones.

## A whole plain-egui screen inside a tree

The pattern scales up: keep the screen's entire state in one hook and draw it
into a filling leaf. This is exactly how the gallery runs the "plain egui"
version of each example next to the egui-react one:

```rust
#[component]
fn PlainScreen(cx: &mut Cx) {
    let mut state = use_state(cx, plain::PlainState::default);
    cx.leaf_fill(&ItemStyle::default().grow(1.0), |ui| {
        plain::ui(ui, state.bind());
    });
}
```

`bind()` and not `&mut *state`: the plain `ui` writes into the state every
frame whether anything changed or not, and `&mut *state` would read that as a
change and ask for another repaint, for ever.

## See it running

[escape-hatch](/examples/escape-hatch) is these four, in order, in one screen.
[shader](/examples/shader) and [patch](/examples/patch) take the painter route
all the way to wgpu.
