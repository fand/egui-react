//! Procedural macros for react-egui: `rsx!`, `#[component]` and `#[hook]`.
//!
//! Not implemented yet (phase 3). `rstml` is already declared as a dependency
//! so that the macro implementation can start without touching the manifest.

// `rstml` is intentionally unused for now; this keeps the dependency alive for
// `-D warnings` builds with `unused_crate_dependencies` enabled downstream.
use rstml as _;
