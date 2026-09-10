---
title: Escape hatches
---

# Escape hatches

egui-react wraps a subset of egui, and that is fine. Everything ends in
`&mut egui::Ui`, so anything egui can do is one call away. The elements in
`egui-react-elements` are written with these same calls.

| You want | Use |
|---|---|
| plain code in the middle of a tree | `{view(\|cx\| ..)}` |
| to draw egui where you are | `cx.leaf(&style, \|ui\| ..)` |
| drawing of your own | `<Canvas>`, or a leaf that allocates a rect and paints |
| hooks inside an egui container's closure | a new `Cx` around the inner `Ui` |

## The one trap

`cx.ui()` is safe to read from anywhere: visuals, style, input. But inside a
`<View>` it is the `Ui` the whole tree started in, not where you are. Draw
through it and the widget lands in the top-left corner of the tree.

`cx.leaf` puts a rect in the flow and hands you the `Ui` for it. Use it
whenever you draw.

## 1. A closure

```rust
rsx! {
    <View direction="column" gap={8}>
        <Text>"above"</Text>
        {view(|cx| {
            // Hooks work here: same `Cx`, slot keyed by this line.
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

For a widget with no element of its own. A leaf is a layout node, so it takes
the same style attributes:

```rust
{view(move |cx| {
    cx.leaf(&ItemStyle::default(), |ui| {
        ui.color_edit_button_srgba(colour.bind());
    });
})}
```

`cx.leaf` measures what it drew and reports that size. For a widget that fills
what it is given, like a progress bar or scroll area, use `cx.leaf_fill` and
give it a size. Otherwise it claims everything:

```rust
{view(move |cx| {
    // Both axes need a size. An `auto` axis on a filling leaf claims the
    // whole window.
    cx.leaf_fill(&ItemStyle::default().w(220.0).h(20.0), |ui| {
        ui.add(egui::ProgressBar::new(value).show_percentage());
    });
})}
```

## 3. A painter

`<Canvas>` allocates a rect and hands it to you:

```rust
rsx! {
    <Canvas grow={1.0} paint={|ui: &mut egui::Ui, rect: egui::Rect| {
        ui.painter().rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
    }}/>
}
```

The same rect is where a wgpu callback goes
(`egui_wgpu::Callback::new_paint_callback(rect, ..)`). That is how
[shader](/examples/shader) gets a fragment shader on screen. `sense` defaults
to `Sense::hover()`. `on_drag` needs `Sense::drag()`.

By hand, it is a filling leaf:

```rust
fn sparkline(ui: &mut egui::Ui, values: &[f32]) {
    let (rect, _response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);
    // ..
}
```

This function is plain egui. It knows nothing about the library.

## 4. A nested `Cx`

Inside an egui container's closure you get a new `Ui`. Build a new `Cx` around
it and hooks work again:

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

`cx.scope("inner", ..)` gives the nested tree its own id, so its hooks do not
collide with the outer ones.

## A whole plain-egui screen inside a tree

Keep the screen's state in one hook and draw it into a filling leaf. This is
how the gallery runs the "plain egui" version of each example:

```rust
#[component]
fn PlainScreen(cx: &mut Cx) {
    let mut state = use_state(cx, plain::PlainState::default);
    cx.leaf_fill(&ItemStyle::default().grow(1.0), |ui| {
        plain::ui(ui, state.bind());
    });
}
```

Use `bind()`, not `&mut *state`. The plain `ui` writes into the state every
frame, and `&mut *state` would ask for a repaint every frame, forever.

## See it running

[escape-hatch](/examples/escape-hatch) is these four in one screen.
[shader](/examples/shader) and [patch](/examples/patch) take the painter
route to wgpu.
