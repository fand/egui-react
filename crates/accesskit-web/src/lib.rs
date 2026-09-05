// Copyright 2023 The AccessKit Authors. All rights reserved.
// Licensed under the Apache License, Version 2.0 (found in
// the LICENSE-APACHE file) or the MIT license (found in
// the LICENSE-MIT file), at your option.

//! An AccessKit adapter for the web: a `TreeUpdate` mirrored into the DOM.
//!
//! The application draws into a `<canvas>`, so nothing it draws reaches
//! assistive technology. This adapter keeps a hidden `<div>` per accessibility
//! node next to the canvas, with the ARIA role, name and value the node
//! carries. Flutter web and Google Docs solve the same problem the same way.
//!
//! Nothing here knows about egui, eframe or any toolkit: the only input is an
//! [`accesskit::TreeUpdate`], the only output is an
//! [`accesskit::ActionRequest`] handed back through the
//! [`accesskit::ActionHandler`] the caller passes in.
//!
//! # Attribution
//!
//! This is a port of the unreleased `accesskit_web` prototype on AccessKit's
//! [`web-basics`](https://github.com/AccessKit/accesskit/tree/web-basics)
//! branch (last touched 2024-07), brought up to `accesskit` 0.24 /
//! `accesskit_consumer` 0.38. The role table, the node wrapper and the shape of
//! the adapter are theirs; see `docs/tasks/a11y/plan.md` for what changed and
//! why. Copyright of the original code stays with the AccessKit authors and it
//! keeps their Apache-2.0 OR MIT terms, which are also this repository's.

mod adapter;
pub use adapter::Adapter;

mod filters;
mod node;
