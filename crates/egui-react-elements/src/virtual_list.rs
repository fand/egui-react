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
/// `for` loop. The layout underneath is keyed the other way, by the slot the
/// row sits in, so that scrolling reuses the same handful of row layouts.
///
/// Rows are laid out by the lite solver, not by taffy: a row is a single-line
/// flex box, and a retained tree for it costs more than its layout does
/// (`crates/egui-react/src/engine/lite.rs`, ARCHITECTURE section 6). The
/// solver covers `display` flex or none, both directions and their reverses,
/// `justify` and `align` other than `baseline`, `gap`, the `w` / `h` / `min` /
/// `max` sizes, `grow` / `shrink` / `basis`, and margins and padding, all in
/// points or percent, at any depth of `<View>`. A row that uses anything else
/// — `display="grid"` or `"block"`, `wrap`, `align_content`, a `baseline`
/// align, `col_span` / `row_span`, an `auto` margin — falls back to taffy,
/// that slot alone and for good. It still lays out the same; it is only
/// slower. To tell, read the log at `debug` level: the fallback prints once
/// per slot and names the attribute that caused it.
///
/// **Every row must be `row_h` tall.** That is what lets `show_rows` work out
/// the range without measuring anything, and it is the one thing this element
/// cannot check for you: a row that draws taller will overlap the next.
///
/// The row's rect is the list's, not the row's: each row is laid out into a
/// rect exactly `row_h` tall, and the list moves on by exactly `row_h`
/// whatever the row drew. So the rows always sit where `show_rows` put them —
/// a row that draws shorter leaves a gap under itself instead of pulling the
/// whole list up, and one that draws taller reaches into the next row.
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
    let layout = cx.layout_id();
    let mut render = render;
    cx.leaf_fill(&style, move |ui| {
        egui::ScrollArea::vertical().show_rows(ui, row_h, rows, move |ui, range| {
            // The rect every row's tree is laid out into, and the room it
            // takes. Fixed, so a row moves the cursor on by exactly the height
            // `show_rows` worked the visible range out from, and so a row
            // tree's root size does not move with the scroll offset. See
            // `Cx::with_root_size`.
            let row_size = egui::vec2(ui.available_width(), row_h);
            // The same shape as any container element: rebuild a `Cx` around
            // the `Ui` egui handed back, then enter a scope per row.
            let mut cx = Cx::new(store, ui, scope);
            for (slot, i) in range.enumerate() {
                // Hooks are keyed by the row index, so row 7 keeps its state
                // wherever it sits. The layout is keyed by the slot the row
                // occupies, because a slot is drawn on every frame: scrolling
                // reuses its nodes instead of building a tree for each row that
                // comes into view, measuring it in an invisible pass, and
                // throwing it away when the row goes out again.
                //
                // `scope_sharing_ui`, not `scope`: a row needs a hook scope,
                // not an egui `Ui` of its own. The row index reaches the
                // widgets through the hook scope, which is what salts each
                // leaf's `Ui` inside the row's `<View>`, so a `Ui` per row
                // would only cost a frame's worth of `Ui::new_child` calls.
                cx.scope_sharing_ui(i, |cx| {
                    cx.with_layout_id(layout.with(("vl-slot", slot)), |cx| {
                        cx.with_root_size(row_size, |cx| render(cx, i))
                    })
                });
            }
        });
    });
}
