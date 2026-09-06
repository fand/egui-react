//! [`VirtualList`]: a long list that only draws the rows you can see.

use egui_react::layout::ItemStyle;
use egui_react::prelude::*;

/// A scrolling list that draws only the rows in view.
///
/// `<ScrollArea>` with a `for` loop inside draws every row, on screen or not,
/// and each row is a handful of taffy nodes. At ten thousand rows that is the
/// difference between a frame taking eighty milliseconds and taking a fifth of
/// one. This element is the immediate-mode answer, wrapped: it asks egui's
/// `ScrollArea::show_rows` which rows the viewport covers, reserves the height
/// of the rest, and calls `render` for the few that are visible.
///
/// ```ignore
/// <VirtualList
///     rows={items.len()}
///     row_h={20.0}
///     grow={1.0}
///     render={|cx: &mut Cx<'_, '_>, i: usize| {
///         rsx! { <Row item={&items[i]}/> }.show(cx);
///     }}
/// />
/// ```
///
/// Where a short list is written as `for` + `key`, a long one is written as
/// render-by-index — the same trade React makes with react-window. What you
/// give up is a `for` loop over what you already have; what you get back is a
/// frame time that does not depend on the length.
///
/// `render` is an ordinary component body. It may open a `<View>`, call hooks,
/// and hold state: each row is entered under `cx.scope(i, ..)`, so row 7 keeps
/// its own state as it scrolls in and out — the same keying `key={i}` gives a
/// `for` loop.
///
/// **Every row must be `row_h` tall.** That is what lets `show_rows` work out
/// the range without measuring anything, and it is the one thing this element
/// cannot check for you: a row that draws taller will overlap the next.
///
/// The size comes from the style, not the content (`leaf_fill`), so give it
/// `grow` or an `h`; with neither it fills the window on that axis.
#[component]
pub fn VirtualList(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    rows: usize,
    row_h: f32,
    // The bound is spelled out rather than elided. `#[component]` rewrites an
    // elided lifetime in a prop to the props struct's own, and a closure taking
    // a `Cx` has to be callable with whatever lifetimes the row's `Cx` has.
    render: impl for<'a, 's, 'u> FnMut(&'a mut Cx<'s, 'u>, usize),
) {
    let (store, scope) = (cx.store, cx.scope_id());
    let mut render = render;
    cx.leaf_fill(&style, move |ui| {
        egui::ScrollArea::vertical().show_rows(ui, row_h, rows, move |ui, range| {
            // The same shape as any container element: rebuild a `Cx` around
            // the `Ui` egui handed back, then enter a scope per row.
            let mut cx = Cx::new(store, ui, scope);
            for i in range {
                cx.scope(i, |cx| render(cx, i));
            }
        });
    });
}
