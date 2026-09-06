//! [`View()`] and [`Text()`]: the flex/grid primitives.

use egui_react::layout::{Align, ContainerStyle, Direction, Display, Gap, ItemStyle, Justify};
use egui_react::prelude::*;

/// A flex or grid container: one taffy node, with every child a taffy node.
///
/// ```ignore
/// <View direction="row" justify="space-between" align="center" gap={8} p={12}>
///     <Text grow={1}>"Title"</Text>
///     <Button on_click={|| *open = true}>"Open"</Button>
/// </View>
/// ```
///
/// `display="grid"` together with `cols={n}` lays the children out in `n`
/// equal-width columns; `col_span` / `row_span` on a child widen it.
#[component]
#[allow(clippy::too_many_arguments)]
pub fn View(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    #[prop(default, into)] display: Display,
    #[prop(default, into)] direction: Direction,
    #[prop(default)] wrap: bool,
    #[prop(default, into)] justify: Justify,
    #[prop(default, into)] align: Align,
    #[prop(into)] align_content: Option<Justify>,
    #[prop(default, into)] gap: Gap,
    cols: Option<u16>,
    children: impl View,
) {
    let container = ContainerStyle {
        display,
        direction,
        wrap,
        justify,
        align,
        align_content,
        gap: (gap.column, gap.row),
        cols,
    };
    // The layout id, not the hook scope: inside a reused list slot the two
    // differ, and the tree has to stay with the slot.
    let id = cx.layout_id();
    cx.container(id, &container, &style, |cx| children.show(cx));
}

/// A text leaf.
///
/// The wrap mode defaults to `Extend`, so a `<Text>` inside a `<View>` reports
/// its full width to taffy instead of collapsing into one character per line.
/// Pass `wrap` to get egui's usual wrapping.
///
/// Inside a `<View>` this is a taffy node holding a galley rather than an
/// `egui::Label` in a `Ui` of its own; see [`Cx::text`]. Outside one it is
/// `ui.add(egui::Label::new(..))` and nothing else.
///
/// `selectable` is `egui::Label::selectable`: leave it off to follow the
/// style's `interaction.selectable_labels`, which is what a `Label` does. A
/// `<Text>` drawn for the first time is not selectable until the next frame,
/// because its place on screen is only known once the layout is computed.
#[component]
#[allow(clippy::too_many_arguments)]
pub fn Text(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    size: Option<f32>,
    color: Option<egui::Color32>,
    #[prop(default)] strong: bool,
    #[prop(default)] wrap: bool,
    selectable: Option<bool>,
    children: impl Into<egui::WidgetText>,
) {
    let mut text: egui::WidgetText = children.into();
    if let Some(size) = size {
        text = text.size(size);
    }
    if let Some(color) = color {
        text = text.color(color);
    }
    if strong {
        text = text.strong();
    }
    cx.text(&style, text, wrap, selectable);
}
