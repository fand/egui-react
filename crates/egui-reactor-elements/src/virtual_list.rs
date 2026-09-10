//! [`VirtualList`]: a long list that only draws the rows you can see.

use egui_reactor::layout::ItemStyle;
use egui_reactor::prelude::*;

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
/// (`crates/egui-reactor/src/engine/lite.rs`, ARCHITECTURE section 6). The
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
/// The pitch is exactly `row_h`: `show_rows` itself would add the `Ui`'s
/// `item_spacing.y` between rows, and this element zeroes it, so the gap
/// between rows is the row's own business (an `h` shorter than `row_h`, or
/// padding). A plain `show_rows` list with `item_spacing.y = gap` and
/// `row_height = h` matches a `<VirtualList row_h={h + gap}>`.
///
/// The row's rect is the list's, not the row's: each row is laid out into a
/// rect exactly `row_h` tall, and the list moves on by exactly `row_h`
/// whatever the row drew. So the rows always sit where `show_rows` put them —
/// a row that draws shorter leaves a gap under itself instead of pulling the
/// whole list up, and one that draws taller reaches into the next row. That
/// rect is the root of the row's `<View>`; a row that is a bare leaf, with no
/// `<View>` around it, draws straight into the list and takes whatever height
/// it drew, so give such a row a `<View h={row_h}>` if its height can vary.
///
/// The size comes from the style, not the content (`leaf_fill`), so give it
/// `grow` or an `h`; with neither it fills the window on that axis.
///
/// `horizontal` lets the list scroll sideways as well, but only together with
/// `row_w`: it is the width every row is laid out into, and with it the width
/// of the content the list scrolls over. The element has to be told, because
/// it cannot measure it — a row's leaves are drawn into rects the layout
/// engine reserved, and the list reserves the row's rect and nothing more, so
/// whatever a row draws past that rect never reaches the scroll area's `Ui`
/// and never counts towards its scroll range. Given `row_w`, a row's
/// `<View w="100%">` is already the width of the sheet and reaches past the
/// edge on its own; it needs no `w` of its own and no `shrink={0}`. `row_w` is
/// never taken as narrower than the viewport, so a list given less than its
/// window still fills it.
///
/// `on_scroll` reports where the list is: the offset in points from the top
/// left of the content to the top left of the viewport, taken after the rows
/// are drawn, so it is the offset egui actually used this frame and not the
/// one it will use next. It fires every frame, scrolled or not, because that
/// is when the number is known. Anything drawn *beside* the list from that
/// offset — a frozen column header, a row header, a ruler — is what it is
/// for. A caller that keeps it in state must write only when it differs from
/// what is already there: a write per frame marks the state dirty and asks for
/// a repaint, and a list that repaints for ever is a list that never idles
/// (ARCHITECTURE 5.6). The headers then run one frame behind the body, which
/// is the same one-frame delay every handler write has (5.7).
#[component]
pub fn VirtualList(
    cx: &mut Cx,
    #[prop(default)] style: ItemStyle,
    rows: usize,
    row_h: f32,
    // The width every row is laid out into, and the width of the content a
    // `horizontal` list scrolls over. `None` is the viewport's width, which is
    // what a list that does not scroll sideways wants.
    row_w: Option<f32>,
    #[prop(default)] horizontal: bool,
    // The bound is spelled out rather than elided. `#[component]` rewrites an
    // elided lifetime in a prop to the props struct's own, and a closure taking
    // a `Cx` has to be callable with whatever lifetimes the row's `Cx` has.
    render: impl for<'a, 's, 'u> FnMut(&'a mut Cx<'s, 'u>, usize),
    #[event] on_scroll: egui::Vec2,
) {
    let (store, scope) = (cx.store, cx.scope_id());
    let layout = cx.layout_id();
    let mut render = render;
    let offset = cx.leaf_fill(&style, move |ui| {
        // `show_rows` places the rows `row_h + item_spacing.y` apart. The
        // element promises `row_h`, so the spacing goes.
        ui.spacing_mut().item_spacing.y = 0.0;
        // The row loop is bound to a name so that the `show_rows` call stays on
        // one line: the closure is the same one it always was.
        let draw_rows = move |ui: &mut egui::Ui, range: std::ops::Range<usize>| {
            // The rect every row's tree is laid out into, and the room it
            // takes. Fixed, so a row moves the cursor on by exactly the height
            // `show_rows` worked the visible range out from, and so a row
            // tree's root size does not move with the scroll offset. See
            // `Cx::with_root_size`.
            //
            // On a sideways-scrolling list the width egui offers is infinite;
            // the viewport's is what a row of `w="100%"` means without `row_w`.
            let viewport_w = ui.available_width();
            let viewport_w = if viewport_w.is_finite() {
                viewport_w
            } else {
                ui.clip_rect().width()
            };
            let width = row_w.map_or(viewport_w, |w| w.max(viewport_w));
            // What tells the scroll area how far it may scroll sideways. The
            // rows cannot: each one takes exactly the rect reserved for it, so
            // `min_rect` would never grow past the viewport however wide the
            // rows were laid out.
            ui.set_min_width(width);
            let row_size = egui::vec2(width, row_h);
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
        };
        let out = egui::ScrollArea::new([horizontal, true]).show_rows(ui, row_h, rows, draw_rows);
        // The offset egui settled on for the frame the rows were just drawn
        // into, handed back out of the leaf the way `Canvas` hands back its
        // `Response`: an event is emitted from the component body, not from
        // inside the closure the leaf runs.
        out.state.offset
    });

    on_scroll.emit(offset);
}
