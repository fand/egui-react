//! A tour of `<View>`'s flex and grid attributes.
//!
//! Every section is a box and every child is a filled chip, because the point
//! of the tour is *where the boxes end up*: with nothing painted, `justify`
//! and `grow` are invisible. The colours are egui's own, so both versions of
//! the example and both themes draw the same picture ([`look`]).

use egui_reactor::prelude::*;
use egui_reactor_elements::prelude::*;
use example_meta::Meta;

pub mod plain;

pub const META: Meta = Meta {
    name: "layout",
    summary: "Every flex and grid attribute `<View>` understands, one section each.",
    hooks: &[],
    elements: &[
        "View",
        "Text",
        "ScrollArea",
        "Grid",
        "Row",
        "Label",
        "Vertical",
    ],
    source: include_str!("lib.rs"),
    plain: Some(include_str!("plain.rs")),
};

/// The two fills the tour is drawn with, and the corner they share.
///
/// From egui's visuals, so the tour follows the theme, and in one place, so
/// the plain egui version paints the same picture — the snapshot test compares
/// the two pixel for pixel.
pub struct Look {
    /// The section's box: what the children are laid out inside.
    pub box_fill: egui::Color32,
    /// One child.
    pub chip: egui::Color32,
}

/// The corner radius of both.
pub const RADIUS: f32 = 4.0;

pub fn look(ctx: &egui::Context) -> Look {
    let visuals = &ctx.style_of(ctx.theme()).visuals;
    Look {
        box_fill: visuals.extreme_bg_color,
        chip: visuals.widgets.inactive.bg_fill,
    }
}

/// One labelled section of the demo: a title, and a box the children are laid
/// out inside.
#[component]
fn Section(cx: &mut Cx, title: &str, children: impl View) {
    let look = look(cx.ctx());
    rsx! {
        <View direction="column" gap={4} mb={12}>
            <Text strong>{title}</Text>
            <View
                direction="column"
                gap={4}
                p={8}
                w="100%"
                bg={look.box_fill}
                radius={RADIUS}
            >
                {children}
            </View>
        </View>
    }
}

/// A coloured box, so the layout is visible.
#[component]
fn Chip(cx: &mut Cx, #[prop(default)] style: ItemStyle, label: &str) {
    let look = look(cx.ctx());
    rsx! {
        // The shorthand attributes chain onto whatever `style=` passed in, so a
        // wrapper can take its caller's layout and add to it.
        <View style={style} p={6} bg={look.chip} radius={RADIUS}>
            <Text>{label}</Text>
        </View>
    }
}

#[component]
pub fn App(cx: &mut Cx) {
    rsx! {
        // The root fills whatever area it is given, so the gallery can drop it
        // into a column of its own.
        <View direction="column" grow={1.0}>
            <ScrollArea grow={1.0}>
                <View direction="column" p={12} w="100%">
                    <Text size={22.0} strong>"layout"</Text>

                    <Section title="direction=\"row\"">
                        <View direction="row" gap={8}>
                            <Chip label="one"/>
                            <Chip label="two"/>
                            <Chip label="three"/>
                        </View>
                    </Section>

                    <Section title="justify">
                        for justify in ["start", "center", "end", "space-between", "space-around"] {
                            <View key={justify} direction="row" justify={justify} w="100%" mb={4}>
                                <Chip label="a"/>
                                <Chip label="b"/>
                                <Chip label="c"/>
                            </View>
                        }
                    </Section>

                    <Section title="align + grow">
                        <View direction="row" align="center" gap={8} h={64.0} w="100%">
                            <Chip label="fixed"/>
                            <Chip grow={1.0} label="grow=1"/>
                            <Chip label="fixed"/>
                        </View>
                    </Section>

                    <Section title="wrap + gap">
                        <View direction="row" wrap gap={(8.0, 4.0)} w={260.0}>
                            for i in 0..8 {
                                <Chip key={i} label={&format!("item {i}")}/>
                            }
                        </View>
                    </Section>

                    <Section title="nested">
                        <View direction="row" gap={8} w="100%">
                            <View direction="column" gap={4} grow={1.0}>
                                <Chip label="left top"/>
                                <Chip label="left bottom"/>
                            </View>
                            <View direction="column" gap={4} grow={2.0}>
                                <Chip label="right top"/>
                                <Chip label="right bottom"/>
                            </View>
                        </View>
                    </Section>

                    <Section title="display=\"grid\" cols={3}">
                        <View display="grid" cols={3} gap={6} w="100%">
                            <Chip col_span={3} label="spans three columns"/>
                            for i in 0..6 {
                                <Chip key={i} label={&format!("cell {i}")}/>
                            }
                        </View>
                    </Section>

                    <Section title="egui containers as leaves">
                        <View direction="row" gap={8} align="start" w="100%">
                            <Grid cols={2}>
                                <Row>
                                    <Label>"grid a1"</Label>
                                    <Label>"grid b1"</Label>
                                </Row>
                                <Row>
                                    <Label>"grid a2"</Label>
                                    <Label>"grid b2"</Label>
                                </Row>
                            </Grid>
                            <Vertical>
                                <Label>"vertical 1"</Label>
                                <Label>"vertical 2"</Label>
                            </Vertical>
                        </View>
                    </Section>
                </View>
            </ScrollArea>
        </View>
    }
}
