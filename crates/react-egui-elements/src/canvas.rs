//! [`Canvas`]: a rectangle taffy sizes and you paint yourself.

use react_egui::layout::ItemStyle;
use react_egui::prelude::*;

/// A blank rectangle you draw into.
///
/// The element allocates the space taffy gave it and calls `paint` with the
/// `Ui` and the rect it got. What goes in there is up to you: `ui.painter()`
/// for lines and shapes, or a paint callback for a wgpu pipeline.
///
/// ```ignore
/// <Canvas
///     grow={1.0}
///     sense={egui::Sense::drag()}
///     on_drag={|d: egui::Vec2| *offset += d}
///     paint={|ui: &mut egui::Ui, rect: egui::Rect| {
///         ui.painter().circle_filled(rect.center(), 20.0, egui::Color32::RED);
///     }}
/// />
/// ```
///
/// The size comes from the style, not the content (`leaf_fill`), so give it
/// `grow` or a `w` / `h`; with neither it fills the window.
///
/// `sense` is what egui asks of the rect, and it decides which events can fire:
/// the default `hover()` gives `on_hover` only, `drag()` (or `click_and_drag()`)
/// is needed for `on_drag`.
///
/// This element knows nothing about wgpu. A shader is drawn by pushing
/// `egui_wgpu::Callback::new_paint_callback(rect, ..)` onto `ui.painter()` from
/// inside `paint`, which keeps the graphics API on the caller's side.
#[component]
pub fn Canvas(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    #[prop(default = egui::Sense::hover())] sense: egui::Sense,
    // The bound is spelled out rather than elided, as in `VirtualList`:
    // `#[component]` rewrites an elided lifetime in a prop to the props
    // struct's own, and the `Ui` here is the leaf's, not the props'.
    paint: impl for<'a> FnOnce(&'a mut egui::Ui, egui::Rect),
    #[event] on_drag: egui::Vec2,
    #[event] on_hover: egui::Pos2,
) {
    let response = cx.leaf_fill(&style, move |ui| {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), sense);
        paint(ui, rect);
        response
    });

    if response.dragged() {
        on_drag.emit(response.drag_delta());
    }
    if let Some(pos) = response.hover_pos() {
        on_hover.emit(pos);
    }
}
