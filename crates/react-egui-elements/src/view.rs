//! [`View`] and [`Text`]: the flex/grid primitives.

use react_egui::layout::{Align, ContainerStyle, Direction, Display, Gap, ItemStyle, Justify};
use react_egui::prelude::*;

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
    let taffy_style = container.merge(&style);
    let id = cx.scope_id();
    cx.container(id, taffy_style, |cx| children.show(cx));
}

/// A text leaf.
///
/// The wrap mode defaults to `Extend`, so a `<Text>` inside a `<View>` reports
/// its full width to taffy instead of collapsing into one character per line.
/// Pass `wrap` to get egui's usual wrapping.
#[component]
pub fn Text(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    size: Option<f32>,
    color: Option<egui::Color32>,
    #[prop(default)] strong: bool,
    #[prop(default)] wrap: bool,
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
    let wrap_mode = if wrap {
        egui::TextWrapMode::Wrap
    } else {
        egui::TextWrapMode::Extend
    };
    cx.leaf(&style, |ui| {
        ui.add(egui::Label::new(text).wrap_mode(wrap_mode))
    });
}
