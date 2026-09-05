//! One egui widget each, drawn through `cx.leaf` so taffy can place them.

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
        ui.add_enabled(enabled, egui::Button::new(children))
            .clicked()
    });
    if clicked {
        on_click.emit(());
    }
}

/// A text label with egui's default wrapping.
///
/// The only difference from [`Text`](crate::view::Text) is the wrap mode:
/// `Label` wraps, `Text` extends.
#[component]
pub fn Label(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    children: impl Into<egui::WidgetText>,
) {
    cx.leaf(&style, |ui| ui.label(children));
}

/// A single- or multi-line text field bound to a `String`.
///
/// `bind` is a `&mut String`, so the widget reads and writes the same state
/// through one borrow. A handler on the same element that also touches that
/// state would be a second borrow and will not compile; use `on_change` for
/// logging or a [`Dispatch`], not for writing back.
#[component]
pub fn TextEdit(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    bind: &mut String,
    #[prop(default)] multiline: bool,
    hint: Option<&str>,
    desired_width: Option<f32>,
    #[event] on_change: (),
    #[event] on_submit: (),
) {
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
        }
        ui.add(edit)
    });

    if response.changed() {
        on_change.emit(());
    }
    if response.lost_focus() && cx.ui().input(|i| i.key_pressed(egui::Key::Enter)) {
        on_submit.emit(());
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
