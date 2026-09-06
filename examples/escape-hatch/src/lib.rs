//! Four ways out of `rsx!` and back into plain egui.
//!
//! egui-react wraps a useful subset of egui, not all of it, and it never has
//! to. Everything below is `&mut egui::Ui` in the end, so anything egui can do
//! is one call away. Which hatch to use:
//!
//! | want | use |
//! |---|---|
//! | ordinary code in the middle of a tree | `{view(\|cx\| ..)}` |
//! | to draw egui where you are | `cx.leaf(&style, \|ui\| ..)` |
//! | drawing of your own | a leaf that allocates a rect and paints into it |
//! | hooks inside an egui container's closure | a new `Cx` around the inner `Ui` |
//!
//! One trap, and it is the reason `leaf` exists. `cx.ui()` is safe to *read*
//! from anywhere, but inside a `<View>` it is the `Ui` the whole taffy tree
//! was started in, not the position you are at. Drawing through it puts the
//! widget outside the layout, at the tree's top-left corner. `cx.leaf` adds a
//! taffy node and hands you the `Ui` for that node instead.
//!
//! None of these is a workaround. The elements in `egui-react-elements` are
//! written with exactly the same calls; there is no private door.
//!
//! The painter section is the one to revisit later: `<Canvas>` will do the
//! allocate-and-paint dance for you, with `on_paint` / `on_drag` / `on_hover`,
//! and will be the way to reach wgpu.

use example_meta::Meta;
use egui_react::prelude::*;
use egui_react_elements::prelude::*;

pub const META: Meta = Meta {
    name: "escape-hatch",
    summary: "Four ways down to plain egui: a closure, a leaf, a painter, and a nested Cx.",
    hooks: &["use_state", "Cx::ui", "Cx::leaf", "Cx::new"],
    elements: &["View", "Text", "Slider", "Button", "Separator"],
    source: include_str!("lib.rs"),
    plain: None,
};

/// The size of the sparkline, in points. Fixed, so the leaf and the painter
/// agree without measuring each other.
const PLOT: egui::Vec2 = egui::vec2(220.0, 60.0);

/// The height of the progress bar, for the same reason as [`PLOT`].
const BAR_H: f32 = 20.0;

/// A deterministic sample, so the picture is the same on every machine.
fn sample(index: usize) -> f32 {
    (index as f32 * 0.7).sin()
}

#[component]
pub fn App(cx: &mut Cx) {
    let mut progress = use_state(cx, || 0.35f32);
    let mut colour = use_state(cx, || egui::Color32::from_rgb(0x6c, 0xa0, 0xdc));
    let mut samples = use_state(cx, Vec::<f32>::new);

    let value = *progress;
    let plotted: Vec<f32> = samples.clone();

    rsx! {
        <View direction="column" gap={8} p={12} grow={1.0}>
            <Text size={22.0} strong>"escape hatches"</Text>

            <Text strong>"1. a closure"</Text>
            <Text>"`{view(|cx| ..)}` is ordinary code in the middle of a tree."</Text>
            {view(|cx| {
                // Hooks work in here: this is the same `Cx` the component has,
                // and the slot is keyed by this line.
                let mut opened = use_state(cx, || 0u32);
                // Reading from `cx.ui()` is always fine.
                let dark = cx.ui().visuals().dark_mode;

                // Drawing is the part that needs care. Inside a `<View>`,
                // `cx.ui()` is the *tree's* `Ui`, not this position in it, so
                // painting through it lands outside the layout — in the
                // top-left corner of the tree, over whatever is there. `leaf`
                // is what puts a rectangle in the flow and hands you the `Ui`
                // for it. Note what is not in here: a spinner. It animates, so
                // it asks for a repaint every frame, which is fine in an app
                // and awkward in a test.
                cx.leaf(&ItemStyle::default(), |ui| {
                    ui.horizontal(|ui| {
                        ui.code("cx.leaf");
                        ui.small(if dark { "dark" } else { "light" });
                        if ui.link("open the docs").clicked() {
                            *opened += 1;
                            ui.ctx()
                                .open_url(egui::OpenUrl::new_tab("https://docs.rs/egui"));
                        }
                        ui.label(format!("opened {} times", *opened));
                    });
                });
            })}

            <Separator/>

            <Text strong>"2. a leaf"</Text>
            <Text>"Widgets with no element of their own, laid out by taffy."</Text>
            <Slider bind={progress.bind()} range={0.0..=1.0} label="progress"/>
            <View direction="row" gap={8} align="center">
                {view(move |cx| {
                    // `leaf_fill`, not `leaf`: a progress bar fills what it is
                    // given, so it is sized by the style rather than by what it
                    // drew last frame. Both axes need a size — an axis left
                    // `auto` on a filling leaf claims the whole window, because
                    // that is what "fill" means with nothing to measure.
                    cx.leaf_fill(&ItemStyle::default().w(PLOT.x).h(BAR_H), |ui| {
                        ui.add(egui::ProgressBar::new(value).show_percentage());
                    });
                })}
                // `egui::ProgressBar` puts nothing in the accessibility tree,
                // so the reading is spelled out for anyone (or any test) that
                // cannot see the bar.
                <Text>{format!("progress {:.2}", value)}</Text>
            </View>
            {view(move |cx| {
                cx.leaf(&ItemStyle::default(), |ui| {
                    ui.horizontal(|ui| {
                        ui.color_edit_button_srgba(colour.bind());
                        ui.label("egui's colour picker, unwrapped");
                    });
                });
            })}

            <Separator/>

            <Text strong>"3. a painter"</Text>
            <Text>"Allocate a rectangle, then draw into it."</Text>
            <View direction="row" gap={8} align="center">
                <Button on_click={|| {
                    let next = sample(samples.len());
                    samples.push(next);
                }}>"add sample"</Button>
                <Button on_click={|| samples.clear()}>"clear"</Button>
                <Text>{format!("{} samples", plotted.len())}</Text>
            </View>
            {view(move |cx| {
                cx.leaf_fill(&ItemStyle::default().w(PLOT.x).h(PLOT.y), |ui| {
                    sparkline(ui, &plotted);
                });
            })}

            <Separator/>

            <Text strong>"4. a nested Cx"</Text>
            <Text>"Hooks inside an egui container's closure."</Text>
            <Nested/>
        </View>
    }
}

/// Draw `values` as a polyline, filling whatever rectangle the leaf was given.
///
/// This is what `<Canvas>` will wrap: allocate, then paint. Nothing here needs
/// egui-react at all — it takes a `&mut egui::Ui` and no more.
fn sparkline(ui: &mut egui::Ui, values: &[f32]) {
    let (rect, _response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 2.0, ui.visuals().extreme_bg_color);

    if values.len() < 2 {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "add two samples",
            egui::TextStyle::Small.resolve(ui.style()),
            ui.visuals().weak_text_color(),
        );
        return;
    }

    let step = rect.width() / (values.len() - 1) as f32;
    let points: Vec<egui::Pos2> = values
        .iter()
        .enumerate()
        .map(|(i, value)| {
            // Samples run -1..1; the y axis points down.
            let t = (1.0 - value) / 2.0;
            egui::pos2(
                rect.left() + i as f32 * step,
                rect.top() + t * rect.height(),
            )
        })
        .collect();
    painter.add(egui::Shape::line(
        points,
        egui::Stroke::new(1.5, ui.visuals().hyperlink_color),
    ));
}

/// An egui container drawn by hand, with egui-react hooks inside it.
///
/// This is how every container element in `egui-react-elements` is written:
/// copy `store` and the scope id out of the `Cx`, call the egui container, and
/// build a new `Cx` around the `Ui` it hands back. `cx.scope` keys the inner
/// hooks so they do not collide with the outer ones.
#[component]
fn Nested(cx: &mut Cx) {
    let mut outer = use_state(cx, || 0i32);
    let (store, scope) = (cx.store, cx.scope_id());
    let outside = *outer;

    rsx! {
        <View direction="column" gap={4}>
            <View direction="row" gap={8} align="center">
                <Button on_click={|| *outer += 1}>"outer +"</Button>
                <Text>{format!("outer: {outside}")}</Text>
            </View>
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
        </View>
    }
}
