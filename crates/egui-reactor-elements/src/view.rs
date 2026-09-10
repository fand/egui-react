//! [`View()`] and [`Text()`]: the flex/grid primitives.

use egui_reactor::layout::{Align, ContainerStyle, Direction, Display, Gap, ItemStyle, Justify};
use egui_reactor::prelude::*;

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
///
/// `font` picks the font family: the name of a stack registered with
/// `egui_reactor_app::fonts::Fonts` (a `FontFamily::Name`), or `"proportional"`
/// / `"monospace"` for egui's two built-in families. A name nothing
/// registered would make egui panic at layout, so it is checked against the
/// current font definitions first; an unknown name draws with the text
/// style's own family and logs one warning per name. Only `<Text>` has the
/// prop: a `<Button>` or `<Checkbox>` label follows `Fonts::default_proportional`,
/// or is passed as `RichText::new(..).family(..)`.
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
    font: Option<&str>,
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
    if let Some(family) = font.and_then(|name| font_family(cx.ctx(), name)) {
        text = with_family(text, family);
    }
    cx.text(&style, text, wrap, selectable);
}

/// The names `<Text font>` warned about, so each is logged once per
/// `Context` and not once per frame.
#[derive(Clone, Default)]
struct UnknownFonts(std::collections::HashSet<String>);

/// Turn the `font` prop into a family egui is guaranteed to know.
///
/// The check is one `BTreeMap` lookup under the fonts lock per `<Text font>`
/// per pass. It is not cached per pass on purpose: the lookup is a handful of
/// string compares, far below the cost of the galley it precedes, and a
/// cache would have to be invalidated when a font source arrives mid-session
/// and `set_fonts` swaps the definitions.
fn font_family(ctx: &egui::Context, name: &str) -> Option<egui::FontFamily> {
    match name {
        "proportional" => Some(egui::FontFamily::Proportional),
        "monospace" => Some(egui::FontFamily::Monospace),
        _ => {
            let family = egui::FontFamily::Name(name.into());
            if ctx.fonts(|f| f.definitions().families.contains_key(&family)) {
                Some(family)
            } else {
                warn_unknown_font_once(ctx, name);
                None
            }
        }
    }
}

fn warn_unknown_font_once(ctx: &egui::Context, name: &str) {
    let id = egui::Id::new("egui_reactor_text_unknown_fonts");
    let first_time = ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<UnknownFonts>(id)
            .0
            .insert(name.to_owned())
    });
    if first_time {
        log::warn!(
            "egui-reactor: <Text font={name:?}> names no registered font family; drawing with the \
             default. Register it with egui_reactor_app::fonts::Fonts, or use \"proportional\" / \
             \"monospace\"."
        );
    }
}

/// Select `family` on the text. A `LayoutJob` or a `Galley` already carries
/// its fonts per section, so those are left as they are.
fn with_family(text: egui::WidgetText, family: egui::FontFamily) -> egui::WidgetText {
    match text {
        egui::WidgetText::Text(s) => egui::RichText::new(s).family(family).into(),
        egui::WidgetText::RichText(rich) => {
            std::sync::Arc::unwrap_or_clone(rich).family(family).into()
        }
        other => other,
    }
}
