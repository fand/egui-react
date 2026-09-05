//! Core of react-egui: `View`, `Cx`, `Store`, `State`, `Handle`, `Dispatch`,
//! the hooks and the layout attribute types.
//!
//! `rsx!`, `#[component]` and `#[hook]` come later; for now components are
//! written by hand in the shape those macros will generate.

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

/// The taffy types the layout attributes convert into.
pub use egui_taffy::taffy;

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
}
