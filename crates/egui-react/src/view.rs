//! [`View`]: what a component body, an `rsx!` block and a child slot all are.

use crate::cx::Cx;
use crate::layout::ItemStyle;

/// Something that can draw itself into a [`Cx`].
///
/// The canonical implementation is a closure, which is what `rsx!` expands to.
/// The other impls exist so that plain values (`"text"`, `Option`, `Vec`) can
/// be dropped into a child slot without ceremony.
pub trait View {
    /// Draw this view.
    fn show(self, cx: &mut Cx<'_, '_>);
}

/// Draws nothing. This is what an empty child slot is.
impl View for () {
    fn show(self, _cx: &mut Cx<'_, '_>) {}
}

impl View for &str {
    fn show(self, cx: &mut Cx<'_, '_>) {
        cx.leaf(&ItemStyle::default(), |ui| ui.label(self));
    }
}

impl View for String {
    fn show(self, cx: &mut Cx<'_, '_>) {
        cx.leaf(&ItemStyle::default(), |ui| ui.label(self));
    }
}

impl<V: View> View for Option<V> {
    fn show(self, cx: &mut Cx<'_, '_>) {
        if let Some(view) = self {
            view.show(cx);
        }
    }
}

impl<V: View> View for Vec<V> {
    fn show(self, cx: &mut Cx<'_, '_>) {
        for view in self {
            view.show(cx);
        }
    }
}

impl<V: View, const N: usize> View for [V; N] {
    fn show(self, cx: &mut Cx<'_, '_>) {
        for view in self {
            view.show(cx);
        }
    }
}

/// The escape hatch: any closure taking a `Cx` is a view.
///
/// A blanket impl over `IntoIterator` would be nicer than the `Vec` and array
/// impls above, but it overlaps both this one and `Option`, so the concrete
/// impls stay.
impl<F: FnOnce(&mut Cx<'_, '_>)> View for F {
    fn show(self, cx: &mut Cx<'_, '_>) {
        self(cx);
    }
}

/// Wrap a closure as a [`View`], fixing the closure's argument type.
///
/// `rsx!` always emits `::egui_react::view(|cx| { .. })`: passing a bare
/// closure where `impl View` is expected leaves `cx` un-inferred, because the
/// `FnOnce` bound is higher-ranked over the `Cx` lifetimes. Users writing the
/// escape hatch by hand want this function for the same reason.
pub fn view<F: FnOnce(&mut Cx<'_, '_>)>(f: F) -> impl View {
    f
}
