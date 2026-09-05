//! Plan 1.8: `Length` and the layout enums parse the CSS-ish spellings `rsx!`
//! will hand them, and the margin/padding shorthands resolve most-specific-wins.

use react_egui::prelude::*;
use react_egui::taffy;

#[test]
fn length_parses_the_css_spellings() {
    assert_eq!(Length::from("auto"), Length::Auto);
    assert_eq!(Length::from("12"), Length::Px(12.0));
    assert_eq!(Length::from("12px"), Length::Px(12.0));
    assert_eq!(Length::from("50%"), Length::Percent(0.5));
    assert_eq!(Length::from(8.0), Length::Px(8.0));
    assert_eq!(Length::from(8), Length::Px(8.0));
}

#[test]
#[should_panic(expected = "cannot parse")]
fn length_rejects_nonsense() {
    let _ = Length::from("wide");
}

#[test]
fn enums_parse_the_css_spellings() {
    assert_eq!(Direction::from("column"), Direction::Column);
    assert_eq!(Justify::from("space-between"), Justify::SpaceBetween);
    assert_eq!(Align::from("center"), Align::Center);
    assert_eq!(Display::from("grid"), Display::Grid);
    assert_eq!(Direction::parse("sideways"), None);
}

#[test]
#[should_panic(expected = "is not a valid Justify")]
fn enums_reject_nonsense() {
    let _ = Justify::from("middle");
}

#[test]
fn margin_shorthands_resolve_most_specific_first() {
    let style = ItemStyle::default().m(1).mx(2).ml(3).to_taffy();
    assert_eq!(style.margin.left, taffy::LengthPercentageAuto::length(3.0));
    assert_eq!(style.margin.right, taffy::LengthPercentageAuto::length(2.0));
    assert_eq!(style.margin.top, taffy::LengthPercentageAuto::length(1.0));
    assert_eq!(
        style.margin.bottom,
        taffy::LengthPercentageAuto::length(1.0)
    );

    let style = ItemStyle::default().py(4).to_taffy();
    assert_eq!(style.padding.top, taffy::LengthPercentage::length(4.0));
    assert_eq!(style.padding.left, taffy::LengthPercentage::length(0.0));
}

#[test]
fn item_style_maps_onto_taffy() {
    let style = ItemStyle::default()
        .w("50%")
        .h(20.0)
        .max_w(100)
        .grow(1.0)
        .shrink(0.0)
        .basis("auto")
        .align_self("center")
        .to_taffy();
    assert_eq!(style.size.width, taffy::Dimension::percent(0.5));
    assert_eq!(style.size.height, taffy::Dimension::length(20.0));
    assert_eq!(style.max_size.width, taffy::Dimension::length(100.0));
    assert_eq!(style.flex_grow, 1.0);
    assert_eq!(style.flex_shrink, 0.0);
    assert_eq!(style.flex_basis, taffy::Dimension::auto());
    assert_eq!(style.align_self, Some(taffy::AlignItems::Center));
}

#[test]
fn container_style_merges_over_the_item_half() {
    let style = ContainerStyle::default()
        .direction("column")
        .justify("space-between")
        .align("center")
        .wrap(true)
        .gaps(4.0, 8.0)
        .merge(&ItemStyle::default().grow(2.0));

    assert_eq!(style.display, taffy::Display::Flex);
    assert_eq!(style.flex_direction, taffy::FlexDirection::Column);
    assert_eq!(
        style.justify_content,
        Some(taffy::JustifyContent::SpaceBetween)
    );
    assert_eq!(style.align_items, Some(taffy::AlignItems::Center));
    assert_eq!(style.flex_wrap, taffy::FlexWrap::Wrap);
    assert_eq!(style.gap.width, taffy::LengthPercentage::length(4.0));
    assert_eq!(style.gap.height, taffy::LengthPercentage::length(8.0));
    // The item half survives the merge.
    assert_eq!(style.flex_grow, 2.0);

    // Unset alignment stays unset, so taffy's own defaults apply.
    let plain = ContainerStyle::default().merge(&ItemStyle::default());
    assert_eq!(plain.justify_content, None);
    assert_eq!(plain.align_items, None);
}

#[test]
fn grid_columns_are_only_built_for_display_grid() {
    let grid = ContainerStyle::default()
        .display("grid")
        .cols(3)
        .merge(&ItemStyle::default());
    // taffy expresses "3 equal columns" as one `repeat(3, 1fr)` track.
    assert_eq!(grid.grid_template_columns.len(), 1);

    let flex = ContainerStyle::default()
        .cols(3)
        .merge(&ItemStyle::default());
    assert!(flex.grid_template_columns.is_empty());
}
