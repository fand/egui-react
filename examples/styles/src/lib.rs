//! Every attribute `style` takes, one row each: its name, the code that uses
//! it, and what that code draws.
//!
//! The code column is not typed in by hand. Each row is written once, as the
//! element it draws, and the [`row!`] macro `stringify!`s the same tokens for
//! the middle column, so the text and the picture cannot drift apart.
//!
//! Every attribute goes through the one `style` prop that every element
//! takes: the layout half (`w`, `grow`, `p`, ...) is resolved by the layout
//! engine, and the paint half (`bg`, `border`, `radius`, `shadow`,
//! `custom_shadow`, `opacity`) is painted by it on the node's own box.

use egui_react::prelude::*;
use egui_react_elements::prelude::*;
use example_meta::Meta;

pub const META: Meta = Meta {
    name: "styles",
    summary: "Every `style` attribute in a table: the name, the code that uses it, and what it draws.",
    hooks: &[],
    elements: &["View", "Text", "ScrollArea"],
    source: include_str!("lib.rs"),
    plain: None,
};

/// The stage every row draws on: wide enough for two chips side by side,
/// tall enough for a chip with room to move.
const STAGE_W: f32 = 180.0;
const STAGE_H: f32 = 48.0;

/// A shadow of our own, for the `custom_shadow` row.
const SHADOW: egui::Shadow = egui::Shadow {
    offset: [6, 6],
    blur: 4,
    spread: 0,
    color: egui::Color32::from_rgba_premultiplied(0, 0, 0, 160),
};

/// The colours the table is drawn with, read once from the theme.
#[derive(Clone, Copy)]
struct Look {
    /// A chip's background.
    chip: egui::Color32,
    /// The stage behind the chips.
    stage: egui::Color32,
    /// The line between two rows.
    line: egui::Color32,
}

impl Look {
    fn of(visuals: &egui::Visuals) -> Self {
        Self {
            chip: visuals.selection.bg_fill,
            stage: visuals.extreme_bg_color,
            line: visuals.widgets.noninteractive.bg_stroke.color,
        }
    }
}

/// The box a row draws its attribute on: a `<View>` with a background and a
/// little padding, so its edges are where the eye can see them.
///
/// The caller's `style` wins: a row about `bg` or `p` sets its own, and the
/// defaults only fill in what the row left unsaid.
#[component]
fn Chip(cx: &mut Cx, #[prop(default)] style: ItemStyle, look: Look) {
    let mut style = style;
    if style.paint.bg.is_none() {
        style = style.bg(look.chip);
    }
    if style.p.is_none() && style.px.is_none() && style.py.is_none() {
        style = style.p(6);
    }
    rsx! {
        <View style={style} justify="center" align="center">
            <Text>"box"</Text>
        </View>
    }
}

/// One row of the table: the attribute's name, the code, and the stage the
/// code draws on. `grid` swaps the stage for a three-column grid, twice as
/// tall, for the two grid attributes.
#[component]
fn Row(
    cx: &mut Cx,
    look: Look,
    name: &str,
    code: String,
    #[prop(default)] grid: bool,
    children: impl View,
) {
    // A flex stage lines its chips up at the top, so a chip's own height and
    // `align_self` show; a grid stage stretches them into their cells, so a
    // spanning chip fills what it spans.
    let (display, align, h) = if grid {
        ("grid", "normal", STAGE_H * 2.0)
    } else {
        ("flex", "start", STAGE_H)
    };
    rsx! {
        <View direction="column" w="100%">
            <View direction="row" align="center" gap={12} w="100%" py={6}>
                <Text w={90.0} strong>{name}</Text>
                // The code, in the code font, wrapped to whatever width is
                // left once the name and the stage have taken theirs.
                <Text grow={1.0} wrap>{egui::RichText::new(code).monospace()}</Text>
                <View
                    display={display}
                    cols={3}
                    direction="row"
                    align={align}
                    gap={4}
                    w={STAGE_W}
                    h={h}
                    p={4}
                    shrink={0.0}
                    bg={look.stage}
                    radius={4.0}
                >
                    {children}
                </View>
            </View>
            // The line under the row: a painted `<View>`, one point tall.
            <View h={1.0} w="100%" bg={look.line}/>
        </View>
    }
}

/// A heading between two groups of rows.
#[component]
fn Group(cx: &mut Cx, title: &str) {
    rsx! {
        <Text size={16.0} strong mt={16} mb={4}>{title}</Text>
    }
}

/// `stringify!` puts a space around every token; take them back out so the
/// code column reads as it was written.
pub fn tidy(code: &str) -> String {
    let mut out = code.to_owned();
    for (from, to) in [
        ("< ", "<"),
        (" >", ">"),
        (" / >", "/>"),
        (" />", "/>"),
        (" = ", "="),
        ("{ ", "{"),
        (" }", "}"),
        ("( ", "("),
        (" )", ")"),
        ("[ ", "["),
        (" ]", "]"),
        (" ,", ","),
        (" :: ", "::"),
        (" . ", "."),
    ] {
        out = out.replace(from, to);
    }
    out
}

/// One row, written once: the tokens draw the stage and, `stringify!`ed,
/// fill the code column. `key` is the name, so two rows expanded from the
/// same macro line do not share a scope.
macro_rules! row {
    ($look:expr, $name:literal, grid, $($body:tt)*) => {
        rsx! {
            <Row key={$name} look={$look} name={$name} code={tidy(stringify!($($body)*))} grid>
                $($body)*
            </Row>
        }
    };
    ($look:expr, $name:literal, $($body:tt)*) => {
        rsx! {
            <Row key={$name} look={$look} name={$name} code={tidy(stringify!($($body)*))}>
                $($body)*
            </Row>
        }
    };
}

#[component]
pub fn App(cx: &mut Cx) {
    let look = Look::of(cx.ui().visuals());
    rsx! {
        // The root fills whatever area it is given, so the gallery can drop it
        // into a column of its own.
        <View direction="column" grow={1.0}>
            <ScrollArea grow={1.0}>
                <View direction="column" p={12} w="100%">
                    <Text size={22.0} strong>"styles"</Text>
                    <Text mb={8}>
                        "Every attribute the `style` prop takes. `Chip` is a `<View>` with a background and `p={6}`."
                    </Text>
                    <View direction="row" align="center" gap={12} w="100%" py={6}>
                        <Text w={90.0} strong>"style"</Text>
                        <Text grow={1.0} strong>"code"</Text>
                        <Text w={STAGE_W} strong>"output"</Text>
                    </View>
                    <View h={1.0} w="100%" bg={look.line}/>

                    <Group title="size"/>
                    {row!(look, "w", <Chip look={look} w={120.0}/>)}
                    {row!(look, "h", <Chip look={look} h={40.0}/>)}
                    {row!(look, "min_w", <Chip look={look} min_w={120.0}/>)}
                    {row!(look, "min_h", <Chip look={look} min_h={40.0}/>)}
                    {row!(look, "max_w", <Chip look={look} w="100%" max_w={100.0}/>)}
                    {row!(look, "max_h", <Chip look={look} h="100%" max_h={24.0}/>)}

                    <Group title="flex item"/>
                    {row!(look, "grow", <Chip look={look} grow={1.0}/> <Chip look={look}/>)}
                    {row!(look, "shrink", <Chip look={look} w={120.0} shrink={0.0}/> <Chip look={look} w={120.0}/>)}
                    {row!(look, "basis", <Chip look={look} basis={100.0}/> <Chip look={look}/>)}
                    {row!(look, "align_self", <Chip look={look} align_self="end"/> <Chip look={look}/>)}

                    <Group title="margin"/>
                    {row!(look, "m", <Chip look={look} m={8}/>)}
                    {row!(look, "mx", <Chip look={look} mx={8}/>)}
                    {row!(look, "my", <Chip look={look} my={8}/>)}
                    {row!(look, "mt", <Chip look={look} mt={8}/>)}
                    {row!(look, "mr", <Chip look={look} mr={8}/> <Chip look={look}/>)}
                    {row!(look, "mb", <Chip look={look} mb={8}/>)}
                    {row!(look, "ml", <Chip look={look} ml={8}/>)}

                    <Group title="padding"/>
                    {row!(look, "p", <Chip look={look} p={12}/>)}
                    {row!(look, "px", <Chip look={look} px={12}/>)}
                    {row!(look, "py", <Chip look={look} py={12}/>)}
                    {row!(look, "pt", <Chip look={look} pt={12}/>)}
                    {row!(look, "pr", <Chip look={look} pr={12}/>)}
                    {row!(look, "pb", <Chip look={look} pb={12}/>)}
                    {row!(look, "pl", <Chip look={look} pl={12}/>)}

                    <Group title="grid item"/>
                    {row!(look, "col_span", grid, <Chip look={look} col_span={2}/> <Chip look={look}/> <Chip look={look}/>)}
                    {row!(look, "row_span", grid, <Chip look={look} row_span={2}/> <Chip look={look}/> <Chip look={look}/> <Chip look={look}/> <Chip look={look}/>)}

                    <Group title="paint"/>
                    {row!(look, "bg", <Chip look={look} bg={egui::Color32::from_rgb(0xd0, 0x60, 0x40)}/>)}
                    {row!(look, "border", <Chip look={look} border={egui::Stroke::new(2.0, egui::Color32::WHITE)}/>)}
                    {row!(look, "radius", <Chip look={look} radius={12.0}/>)}
                    {row!(look, "shadow", <Chip look={look} m={8} shadow/>)}
                    {row!(look, "custom_shadow", <Chip look={look} m={8} custom_shadow={SHADOW}/>)}
                    {row!(look, "opacity", <Chip look={look} opacity={0.4}/>)}
                </View>
            </ScrollArea>
        </View>
    }
}

#[cfg(test)]
mod tests {
    use super::tidy;

    #[test]
    fn tidy_puts_the_code_back_as_written() {
        assert_eq!(
            tidy(stringify!(<Chip look={look} w={120.0}/>)),
            "<Chip look={look} w={120.0}/>"
        );
        assert_eq!(
            tidy(
                stringify!(<Chip look={look} border={egui::Stroke::new(2.0, egui::Color32::WHITE)}/>)
            ),
            "<Chip look={look} border={egui::Stroke::new(2.0, egui::Color32::WHITE)}/>"
        );
        assert_eq!(
            tidy(stringify!(<Chip look={look} grow={1.0}/> <Chip look={look}/>)),
            "<Chip look={look} grow={1.0}/> <Chip look={look}/>"
        );
    }
}
