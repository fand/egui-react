//! What every example tells the gallery about itself.
//!
//! Each example crate is a library with a `pub fn App` and a `pub const META`.
//! The gallery depends on those libraries, builds its list from the `META`
//! constants and shows `source` next to the running `App`.

/// One example, as the gallery sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Meta {
    /// Crate name, also the `location.hash` deep link: `"counter"`.
    pub name: &'static str,
    /// One line saying what the example shows.
    pub summary: &'static str,
    /// The hooks it uses, for the tag filter: `["use_state"]`.
    pub hooks: &'static [&'static str],
    /// The elements it uses, for the tag filter: `["View", "Button"]`.
    pub elements: &'static [&'static str],
    /// The example's own source, from `include_str!("lib.rs")`.
    pub source: &'static str,
    /// The same UI written in plain egui, from `include_str!("plain.rs")`.
    /// `None` while the example has no plain version.
    pub plain: Option<&'static str>,
}
