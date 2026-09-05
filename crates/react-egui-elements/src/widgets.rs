//! One egui widget each, drawn through `cx.leaf` so taffy can place them.
//!
//! Every widget that carries text sets [`egui::TextWrapMode::Extend`]. A taffy
//! leaf is measured from the size it reported the last time it was drawn, and
//! egui_taffy hands that one value back as both the min- and the max-content
//! size. The first draw happens in a zero-width `Ui`, so a widget left to wrap
//! reports one character wide, taffy keeps the node that narrow, and the label
//! ends up written downwards one letter per line. Where the widget has no
//! `wrap_mode` builder, the mode is set on the leaf's own `Ui` instead.

use std::ops::RangeInclusive;

use react_egui::layout::ItemStyle;
use react_egui::prelude::*;

/// A clickable button.
#[component]
pub fn Button(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    #[prop(default = true)] enabled: bool,
    #[event] on_click: (),
    children: impl Into<egui::WidgetText>,
) {
    let clicked = cx.leaf(&style, |ui| {
        let button = egui::Button::new(children).wrap_mode(egui::TextWrapMode::Extend);
        ui.add_enabled(enabled, button).clicked()
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
/// `bind` is a `&mut String`, so the widget reads and writes the same state
/// through one borrow. A handler on the same element cannot touch that state as
/// well — that would be a second borrow and will not compile. `on_submit`
/// therefore *carries* the text, and `clear_on_submit` empties the field for
/// you, which together cover the usual "type, press Enter, add an item" flow.
#[component]
#[allow(clippy::too_many_arguments)]
pub fn TextEdit(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    bind: &mut String,
    #[prop(default)] multiline: bool,
    hint: Option<&str>,
    desired_width: Option<f32>,
    #[prop(default)] clear_on_submit: bool,
    #[event] on_change: (),
    #[event] on_submit: String,
) {
    // Inside a `<View>` the node's width is taffy's decision (`w`, `grow`, or
    // the space left over), and the widget should fill it. `egui::TextEdit`
    // otherwise draws at its own 280pt default and leaves the rest of the node
    // empty. An explicit `desired_width` still wins: that is what the prop is
    // for. Outside taffy there is nothing to fill, so egui's default stands.
    let fill = desired_width.is_none() && cx.in_taffy();
    let response = cx.leaf(&style, |ui| {
        let mut edit = if multiline {
            egui::TextEdit::multiline(bind)
        } else {
            egui::TextEdit::singleline(bind)
        };
        if let Some(hint) = hint {
            edit = edit.hint_text(hint);
        }
        if let Some(width) = desired_width {
            edit = edit.desired_width(width);
        } else if fill {
            edit = edit.desired_width(ui.available_width());
        }
        ui.add(edit)
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
    let (changed, selected) = cx.leaf(&style, |ui| {
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
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
#[component]
pub fn Image(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    source: egui::ImageSource<'_>,
    fit: Option<egui::Vec2>,
) {
    cx.leaf(&style, |ui| {
        let mut image = egui::Image::new(source);
        if let Some(size) = fit {
            image = image.fit_to_exact_size(size);
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
