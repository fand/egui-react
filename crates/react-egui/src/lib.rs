//! Core of react-egui: `Cx`, `Store`, `State`, `Handle` and hooks.
//!
//! This is the spike implementation: enough of the runtime to check that the
//! design in `docs/ARCHITECTURE.md` holds up against Rust's borrow rules and
//! egui's execution model. `rsx!`, `#[component]` and `#[hook]` come later; for
//! now components are written by hand in the shape those macros will generate.

mod cx;
mod hooks;
mod state;
mod store;

pub use cx::Cx;
pub use hooks::{FnCleanup, IntoCleanup, NoCleanup, use_effect, use_handle, use_state};
pub use state::{Handle, State};
pub use store::{Collision, Store};

/// Everything a component needs, in one `use`.
pub mod prelude {
    pub use crate::cx::Cx;
    pub use crate::hooks::{use_effect, use_handle, use_state};
    pub use crate::state::{Handle, State};
    pub use crate::store::{Collision, Store};
}
