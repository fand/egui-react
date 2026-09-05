//! Procedural macros for react-egui: `rsx!`, `#[component]` and `#[hook]`.
//!
//! These are re-exported from `react_egui`, which is also the crate the
//! expansions refer to: everything they emit is fully qualified as
//! `::react_egui::..`, so user code only needs `use react_egui::prelude::*`.

mod component;
mod hook;
mod rsx;
mod util;

use proc_macro::TokenStream;

/// Turn a function into a react-egui component.
///
/// ```ignore
/// #[component]
/// fn Counter(cx: &mut Cx, initial: i32, label: Option<&str>, #[event] on_change: i32) {
///     ..
/// }
/// ```
///
/// Generates a `CounterProps` struct with a typed builder, an optional
/// `CounterEvent` enum (one variant per `#[event]` argument) and a
/// `Counter(cx, props)` function. The body's tail expression is shown as a
/// `View`.
///
/// Argument attributes:
///
/// - `#[prop(default)]` / `#[prop(default = expr)]`: optional prop.
/// - `#[prop(into)]`: the setter takes `impl Into<T>`.
/// - `#[event]`: becomes an `Emitter` in the body and a variant in the event enum.
///
/// An argument of type `Option<T>` is optional without any attribute. An
/// argument named `children` receives the child nodes; if it is not declared,
/// one of type `()` is generated, because `rsx!` always passes children.
#[proc_macro_attribute]
pub fn component(attr: TokenStream, item: TokenStream) -> TokenStream {
    component::expand(attr.into(), item.into()).into()
}

/// Turn a function into a custom hook.
///
/// Adds `#[track_caller]` and wraps the body in
/// `cx.hook_scope(Location::caller(), |cx| ..)`, so that the hooks inside are
/// keyed by *where the custom hook was called*. The first `&mut Cx` argument is
/// the one that gets scoped.
#[proc_macro_attribute]
pub fn hook(attr: TokenStream, item: TokenStream) -> TokenStream {
    hook::expand(attr.into(), item.into()).into()
}

/// Build a `View` from JSX-like syntax.
///
/// ```ignore
/// rsx! {
///     <View direction="row" gap={8}>
///         <Text>"count: "</Text>
///         if *count > 0 {
///             <Button on_click={|| *count -= 1}>"-"</Button>
///         }
///         for (i, todo) in todos.iter().enumerate() {
///             <Row key={i} label={todo} />
///         }
///     </View>
/// }
/// ```
///
/// Expands to `::react_egui::view(|cx| { .. })`. Elements are component
/// functions, `{expr}` embeds any `View`, `"text"` is a string literal (bare
/// text is an error), and `if` / `for` / `match` are the real Rust control-flow
/// statements.
#[proc_macro]
pub fn rsx(input: TokenStream) -> TokenStream {
    rsx::expand(input.into()).into()
}
