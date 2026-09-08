//! Core of egui-react: `View`, `Cx`, `Store`, `State`, `Handle`, `Dispatch`,
//! the hooks and the layout and paint attribute types.
//!
//! `rsx!`, `#[component]` and `#[hook]` come from `egui-react-macros` and are
//! re-exported here, so `use egui_react::prelude::*` is the only import a
//! component file needs.

mod context;
mod cx;
mod dispatch;
mod engine;
mod events;
mod future;
mod hooks;
pub mod layout;
pub mod paint;
mod state;
mod store;
mod view;

pub use context::{provide_context, use_context};
pub use cx::Cx;
pub use dispatch::{Dispatch, use_reducer};
pub use events::{Arity0, Arity1, Emitter, EventSink, Handler};
pub use future::{SpawnFuture, spawn, use_future};
pub use hooks::{
    FnCleanup, IntoCleanup, NoCleanup, use_animate, use_animate_with, use_effect, use_handle,
    use_memo, use_persisted, use_state,
};
pub use layout::{
    Align, AlignSelf, ContainerStyle, Direction, Display, Gap, ItemStyle, Justify, Length,
    PaintStyle,
};
pub use state::{Handle, State};
pub use store::{Collision, Store};
pub use view::{View, view};

pub use egui_react_macros::{component, hook, rsx};

/// The taffy types the layout attributes convert into.
pub use taffy;

#[doc(hidden)]
pub mod __private {
    pub use typed_builder;
    pub use typed_builder::TypedBuilder;

    /// Implemented by every `#[component]` props struct.
    pub trait Props {
        /// The typed-builder builder type for this props struct.
        type Builder;
        /// Whether the component draws into its parent's `Ui`.
        ///
        /// Set by `#[component(shares_ui)]`. Such a component still gets a hook
        /// scope of its own, but no `Ui::push_id`, so it can reach the parent's
        /// layout: that is what docked panels and grid rows need.
        const SHARES_UI: bool = false;
        /// Start building.
        fn builder() -> Self::Builder;
    }

    /// Enter one element's scope and call it. This is what `rsx!` emits.
    ///
    /// `P` is fixed by the `props` value, so the component's generic arguments
    /// are already settled by the time this is called.
    pub fn enter_scope<P, F, S>(cx: &mut super::Cx<'_, '_>, source: S, props: P, component: F)
    where
        P: Props,
        S: ::core::hash::Hash + ::core::fmt::Debug,
        F: for<'a, 'b, 'c> Fn(&'a mut super::Cx<'b, 'c>, P),
    {
        if P::SHARES_UI {
            cx.scope_sharing_ui(source, move |cx| component(cx, props));
        } else {
            cx.scope(source, move |cx| component(cx, props));
        }
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
    pub use crate::future::{spawn, use_future};
    pub use crate::hooks::{
        use_animate, use_animate_with, use_effect, use_handle, use_memo, use_persisted, use_state,
    };
    pub use crate::layout::{
        Align, AlignSelf, ContainerStyle, Direction, Display, Gap, ItemStyle, Justify, Length,
        PaintStyle,
    };
    pub use crate::state::{Handle, State};
    pub use crate::store::{Collision, Store};
    pub use crate::view::{View, view};
    pub use egui_react_macros::{component, hook, rsx};

    /// What `use_future` returns. Re-exported so a component file needs no
    /// `use std::task::Poll` of its own.
    pub use std::task::Poll;
}
