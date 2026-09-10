//! [`Suspense`], the boundary that waits for the `use_future`s inside it.

use egui_reactor::prelude::*;

/// Where the children are drawn while the boundary is suspended.
///
/// Far off screen and large enough that nothing wraps differently there than it
/// will once the children become visible. Fixed, so that the layout engine
/// measures the same box on every pass and does not ask for a pass of its own.
const OFFSCREEN_RECT: egui::Rect = egui::Rect {
    min: egui::pos2(-1.0e5, -1.0e5),
    max: egui::pos2(-1.0e5 + 4096.0, -1.0e5 + 4096.0),
};

/// Draw `fallback` until every `use_future` below has a result.
///
/// This is what React's `<Suspense>` does, without the throw: a child waits by
/// returning early (`let Poll::Ready(x) = use_future(..) else { return };`) and
/// the pending hook counts itself against the nearest boundary.
///
/// While suspended the children are still drawn, into an invisible `Ui` off
/// screen, so their hooks run and their futures start and finish. They keep the
/// same scope id in both paths, so state and futures survive the switch. Two
/// things follow from drawing them for real: their `use_effect`s run while
/// suspended (React would not run them, because it never commits), and their
/// handlers do not fire, because an invisible `Ui` is also disabled.
///
/// The switch happens within one frame, through `request_discard`. The boundary
/// starts suspended so that, if the pass budget runs out and the discard is
/// refused, what stays on screen is the fallback rather than half-drawn
/// children.
///
/// `shares_ui`, so children and fallback go straight into the parent's surface:
/// inside a `<View>` the children become nodes of the parent's taffy tree.
#[component(shares_ui)]
pub fn Suspense(cx: &mut Cx, fallback: impl View, children: impl View) {
    let (store, scope) = (cx.store, cx.scope_id());
    // The offscreen path draws the same children, so it keeps the layout id as
    // well as the scope id: the taffy trees below are then the same trees, and
    // resolving does not have to build them again.
    let layout = cx.layout_id();
    let suspended = use_handle(cx, || true);

    if suspended.get() {
        fallback.show(cx);

        // A root `Ui` of its own, so the children take no space from the
        // parent's layout. `invisible` turns off both painting and input.
        let mut ui = egui::Ui::new(
            store.ctx().clone(),
            scope.with("__egui_reactor_suspense_offscreen"),
            egui::UiBuilder::new()
                .max_rect(OFFSCREEN_RECT)
                .invisible()
                .sizing_pass(),
        );
        store.begin_suspense();
        {
            let mut cx = Cx::new(store, &mut ui, scope);
            cx.with_layout_id(layout, |cx| children.show(cx));
        }
        if store.end_suspense() == 0 {
            suspended.set(false);
            store
                .ctx()
                .request_discard("egui-reactor: suspense resolved");
        }
    } else {
        store.begin_suspense();
        children.show(cx);
        if store.end_suspense() > 0 {
            suspended.set(true);
            store
                .ctx()
                .request_discard("egui-reactor: suspense pending");
        }
    }
}
