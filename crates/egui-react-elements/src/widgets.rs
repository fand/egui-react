//! One egui widget each, drawn through `cx.leaf` so taffy can place them.
//!
//! Every element takes the paint props through `style` (`bg` `border` `radius`
//! `shadow` `opacity`): the engine paints the box, before the widget draws and
//! after. Where a widget paints a box of its own, it hands that box over as
//! soon as the caller asks for one, so the two do not stack — see [`Button`],
//! [`TextEdit`] and [`ComboBox`].
//!
//! Every widget that carries text sets [`egui::TextWrapMode::Extend`]. A taffy
//! leaf is measured from the size it reported the last time it was drawn, and
//! the engine hands that one value back as both the min- and the max-content
//! size. The first draw happens in a zero-width `Ui`, so a widget left to wrap
//! reports one character wide, taffy keeps the node that narrow, and the label
//! ends up written downwards one letter per line. Where the widget has no
//! `wrap_mode` builder, the mode is set on the leaf's own `Ui` instead.

use std::ops::RangeInclusive;

use egui_react::layout::ItemStyle;
use egui_react::prelude::*;

/// A clickable button.
///
/// `label` is the name assistive technology reads. Pass it whenever the
/// children do not say what the button does — an icon or a single glyph like
/// `"x"` reads as that glyph and nothing more. It only renames the accessibility
/// node; what is drawn stays the children.
///
/// **`p` is the button's own padding.** Symmetric padding in points becomes
/// `Spacing::button_padding` instead of layout padding, so the pill the widget
/// draws, the area that reacts to the pointer and the box the layout reserved
/// are all the same rect. Asymmetric or percentage padding stays layout
/// padding, where the widget draws inside it. Either way the node is the same
/// size.
///
/// **`bg` and `border` hand the resting box over.** egui then draws no frame
/// of its own while the button is inactive, and its hovered and pressed frames
/// still go on top: `bg` is the resting colour, hover stays the widget's.
#[component]
pub fn Button(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    #[prop(default = true)] enabled: bool,
    label: Option<&str>,
    #[event] on_click: (),
    children: impl Into<egui::WidgetText>,
) {
    let paint = style.paint;
    // Only a padding the widget can express as its own: the same points either
    // way, but centred on the widget rather than around it.
    let button_padding = style
        .padding_px()
        .filter(|[top, right, bottom, left]| left == right && top == bottom)
        .map(|[y, x, _, _]| egui::vec2(x, y));
    let style = match button_padding {
        Some(_) => style.without_padding(),
        None => style,
    };

    let clicked = cx.leaf(&style, |ui| {
        if let Some(padding) = button_padding {
            // The leaf's own `Ui` in a tree, the scope's `push_id` child
            // outside: `spacing_mut` copies the style, so nothing leaks past
            // this element.
            ui.spacing_mut().button_padding = padding;
        }
        let mut button = egui::Button::new(children).wrap_mode(egui::TextWrapMode::Extend);
        if let Some(radius) = paint.radius {
            button = button.corner_radius(radius); // `CornerRadius: From<f32>`
        }
        if paint.bg.is_some() || paint.border.is_some() {
            // Not `fill(TRANSPARENT)`, which would kill the hovered fill too.
            button = button.frame_when_inactive(false);
        }
        let response = ui.add_enabled(enabled, button);
        if let Some(label) = label {
            // The widget has already written its node for this pass, so this
            // overwrites the label egui took from the children.
            // `Response::widget_info` would work too, but it pushes a second
            // `OutputEvent` on the frame the button is clicked.
            // Returns `None` when accesskit is off, which is the usual case.
            ui.ctx()
                .accesskit_node_builder(response.id, |node| node.set_label(label));
        }
        response.clicked()
    });
    if clicked {
        on_click.emit(());
    }
}

/// A plain text label.
///
/// [`Text`](crate::view::Text) is the same leaf with `size` / `color` /
/// `strong` on top. Both extend rather than wrap by default (see the module
/// docs); pass `wrap` for egui's usual wrapping, which needs a width on the
/// leaf (`w`, or `grow` in a sized container) to wrap against.
#[component]
pub fn Label(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    #[prop(default)] wrap: bool,
    children: impl Into<egui::WidgetText>,
) {
    let wrap_mode = if wrap {
        egui::TextWrapMode::Wrap
    } else {
        egui::TextWrapMode::Extend
    };
    cx.leaf(&style, |ui| {
        ui.add(egui::Label::new(children).wrap_mode(wrap_mode))
    });
}

/// A single- or multi-line text field bound to a `String`.
///
/// Inside a `<View>` the field fills the node taffy gave it, in both
/// directions for a `multiline` one. `desired_width` and `rows` override that.
///
/// `bind` is a `&mut String`, so the widget reads and writes the same state
/// through one borrow. A handler on the same element cannot touch that state as
/// well — that would be a second borrow and will not compile. `on_submit`
/// therefore *carries* the text, and `clear_on_submit` empties the field for
/// you, which together cover the usual "type, press Enter, add an item" flow.
///
/// `bg` or `border` hands the field's own box over: egui draws its frame with
/// no fill and no stroke, keeping only the margin it insets the text by, and
/// the engine paints the box the caller asked for.
#[component]
#[allow(clippy::too_many_arguments)]
pub fn TextEdit(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    bind: &mut String,
    #[prop(default)] multiline: bool,
    hint: Option<&str>,
    desired_width: Option<f32>,
    rows: Option<usize>,
    #[prop(default)] clear_on_submit: bool,
    #[event] on_change: (),
    #[event] on_submit: String,
) {
    // Inside a `<View>` the node's size is taffy's decision (`w` / `h`, `grow`,
    // or the space left over), and the widget should fill it. `egui::TextEdit`
    // otherwise draws at its own 280pt and four rows and leaves the rest of the
    // node empty. The explicit props still win: that is what they are for.
    // Outside taffy there is nothing to fill, so egui's defaults stand.
    let taffy = cx.in_taffy();
    let fill_width = desired_width.is_none() && taffy;
    let fill_rows = rows.is_none() && multiline && taffy;
    let paint = style.paint;
    let response = cx.leaf(&style, |ui| {
        let mut edit = if multiline {
            egui::TextEdit::multiline(bind)
        } else {
            egui::TextEdit::singleline(bind)
        };
        if let Some(hint) = hint {
            edit = edit.hint_text(hint);
        }
        if paint.bg.is_some() || paint.border.is_some() {
            // egui 0.36 has no `frame(false)`: a frame of its own, with the
            // margin egui's default frame uses, is how the box is given up
            // without moving the text.
            edit = edit.frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(4, 2)));
        }
        if let Some(width) = desired_width {
            edit = edit.desired_width(width);
        } else if fill_width {
            edit = edit.desired_width(ui.available_width());
        }
        if let Some(rows) = rows {
            edit = edit.desired_rows(rows);
        }
        if fill_rows {
            // `add_sized`, not `desired_rows`: a row count can only fill the
            // node to the nearest whole row, and the remainder would be a
            // sliver of a row hanging out of it.
            ui.add_sized(ui.available_size(), edit)
        } else {
            ui.add(edit)
        }
    });

    if response.changed() {
        on_change.emit(());
    }
    if response.lost_focus() && cx.ui().input(|i| i.key_pressed(egui::Key::Enter)) {
        let text = std::mem::take(bind);
        if !clear_on_submit {
            *bind = text.clone();
        }
        on_submit.emit(text);
    }
}

/// A checkbox bound to a `bool`.
#[component]
pub fn Checkbox(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    bind: &mut bool,
    label: Option<&str>,
    #[event] on_change: bool,
) {
    let (changed, value) = cx.leaf(&style, |ui| {
        // `egui::Checkbox` has no `wrap_mode` builder, so the mode goes on the
        // leaf's `Ui`.
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        let changed = ui
            .add(egui::Checkbox::new(bind, label.unwrap_or_default()))
            .changed();
        (changed, *bind)
    });
    if changed {
        on_change.emit(value);
    }
}

/// A slider bound to any numeric value.
#[component]
pub fn Slider<T: egui::emath::Numeric>(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    bind: &mut T,
    range: RangeInclusive<T>,
    label: Option<&str>,
    #[event] on_change: (),
) {
    let changed = cx.leaf(&style, |ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        let mut slider = egui::Slider::new(bind, range);
        if let Some(label) = label {
            slider = slider.text(label);
        }
        ui.add(slider).changed()
    });
    if changed {
        on_change.emit(());
    }
}

/// A dropdown bound to the index of the selected option.
///
/// `egui::ComboBox` has no fill or stroke builder, so `bg`, `border` and
/// `radius` are handed to it through the leaf's own visuals: the resting box
/// goes transparent when the caller paints one, and the radius reaches every
/// widget state so the open and the hovered box are rounded the same way.
#[component]
pub fn ComboBox<S: AsRef<str>>(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    bind: &mut usize,
    options: &[S],
    label: Option<&str>,
    #[event] on_change: usize,
) {
    let id = cx.scope_id();
    let paint = style.paint;
    let (changed, selected) = cx.leaf(&style, |ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
        if paint.bg.is_some() || paint.border.is_some() || paint.radius.is_some() {
            // A copy of the visuals, so nothing leaks past this element.
            let widgets = &mut ui.visuals_mut().widgets;
            if paint.bg.is_some() {
                widgets.inactive.weak_bg_fill = egui::Color32::TRANSPARENT;
            }
            if paint.border.is_some() {
                widgets.inactive.bg_stroke = egui::Stroke::NONE;
            }
            if let Some(radius) = paint.radius {
                let radius = egui::CornerRadius::from(radius);
                for state in [
                    &mut widgets.noninteractive,
                    &mut widgets.inactive,
                    &mut widgets.hovered,
                    &mut widgets.active,
                    &mut widgets.open,
                ] {
                    state.corner_radius = radius;
                }
            }
        }
        let mut combo = egui::ComboBox::from_id_salt(id);
        if let Some(label) = label {
            combo = egui::ComboBox::new(id, label);
        }
        let changed = combo
            .show_index(ui, bind, options.len(), |i| options[i].as_ref())
            .changed();
        (changed, *bind)
    });
    if changed {
        on_change.emit(selected);
    }
}

/// An image.
///
/// The loader for the source's scheme has to be installed by the application
/// (`egui_extras::install_image_loaders`); this element only draws.
///
/// `alt` is the alternative text: it names the image for assistive technology
/// and is drawn next to the ⚠ placeholder when the image fails to load. An
/// image without `alt` has no name at all, so pass one unless the image is
/// decoration.
#[component]
pub fn Image(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    source: egui::ImageSource<'_>,
    fit: Option<egui::Vec2>,
    alt: Option<&str>,
) {
    cx.leaf(&style, |ui| {
        let mut image = egui::Image::new(source);
        if let Some(size) = fit {
            image = image.fit_to_exact_size(size);
        }
        if let Some(alt) = alt {
            image = image.alt_text(alt);
        }
        ui.add(image)
    });
}

/// A separator line.
#[component]
pub fn Separator(cx: &mut Cx, #[prop(default)] style: ItemStyle, #[prop(default)] vertical: bool) {
    cx.leaf(&style, |ui| {
        let separator = egui::Separator::default();
        ui.add(if vertical {
            separator.vertical()
        } else {
            separator.horizontal()
        })
    });
}
