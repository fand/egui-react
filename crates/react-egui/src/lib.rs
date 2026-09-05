//! Core of react-egui: `View`, `Cx`, `Store`, `State`, `Handle`, `Dispatch`,
//! the hooks and the layout attribute types.
//!
//! `rsx!`, `#[component]` and `#[hook]` come from `react-egui-macros` and are
//! re-exported here, so `use react_egui::prelude::*` is the only import a
//! component file needs.

mod context;
mod cx;
mod dispatch;
mod events;
mod hooks;
pub mod layout;
mod state;
mod store;
mod view;

pub use context::{provide_context, use_context};
pub use cx::Cx;
pub use dispatch::{Dispatch, use_reducer};
pub use events::{Arity0, Arity1, Emitter, EventSink, Handler};
pub use hooks::{FnCleanup, IntoCleanup, NoCleanup, use_effect, use_handle, use_memo, use_state};
pub use layout::{
    Align, AlignSelf, ContainerStyle, Direction, Display, ItemStyle, Justify, Length,
};
pub use state::{Handle, State};
pub use store::{Collision, Store};
pub use view::{View, view};

pub use react_egui_macros::{component, hook, rsx};

/// The taffy types the layout attributes convert into.
pub use egui_taffy::taffy;

#[doc(hidden)]
pub mod __private {
    pub use typed_builder;
    pub use typed_builder::TypedBuilder;

    /// Implemented by every `#[component]` props struct.
    pub trait Props {
        /// The typed-builder builder type for this props struct.
        type Builder;
        /// Start building.
        fn builder() -> Self::Builder;
    }
}

pub use __private::Props;

/// Get the props builder for a component function.
///
/// A function item's type cannot be named, so `rsx!` passes the function by
/// reference and lets the `Fn` bound infer the props type: `<Counter/>` becomes
/// `Counter(cx, props_builder(&Counter)..build())` and the user only has to
/// `use` the component itself.
pub fn props_builder<P, F>(_component: &F) -> P::Builder
where
    P: Props,
    F: for<'a, 's, 'u> Fn(&'a mut Cx<'s, 'u>, P),
{
    P::builder()
}

/// Everything a component needs, in one `use`.
pub mod prelude {
    pub use crate::context::{provide_context, use_context};
    pub use crate::cx::Cx;
    pub use crate::dispatch::{Dispatch, use_reducer};
    pub use crate::events::{Emitter, EventSink, Handler};
    pub use crate::hooks::{use_effect, use_handle, use_memo, use_state};
    pub use crate::layout::{
        Align, AlignSelf, ContainerStyle, Direction, Display, ItemStyle, Justify, Length,
    };
    pub use crate::state::{Handle, State};
    pub use crate::store::{Collision, Store};
    pub use crate::view::{View, view};
    pub use react_egui_macros::{component, hook, rsx};
}
