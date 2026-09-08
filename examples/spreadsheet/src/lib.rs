//! A spreadsheet: formulas over 26 x 10,000 cells.
//!
//! *This is step 1 of the example: the model, the language and the evaluator,
//! with their unit tests. The screen — the grid, the headers, the editor and
//! the keyboard — is the next step, and this file is a placeholder until then.
//! The plan is `docs/tasks/spreadsheet/plan.md`.*
//!
//! What the finished example is about, and what the three modules below are
//! already shaped for:
//!
//! **A derivation chain over a 2D grid.** Cell text becomes parsed formulas,
//! a dependency graph and an evaluation order ([`eval::compile`]), and that
//! becomes a value per cell ([`eval::evaluate`]). Two stages, two `use_memo`s,
//! two revision counters on the document ([`sheet::Sheet::structure_rev`] and
//! [`sheet::Sheet::value_rev`]) — the same shape as `patch`'s `topology_rev` /
//! `param_rev`. Typing a number into a literal cell re-runs the evaluation
//! only; typing a formula re-runs the parse and the ordering as well, and
//! [`sheet::reduce`] is the one place that decides which.
//!
//! **Which state belongs to the item and which to the whole.** A
//! `<VirtualList>` row that scrolls out of view is unmounted and its hooks are
//! swept, so the draft being typed into a cell cannot live in the cell: a
//! draft that vanished because the user scrolled would be a bug. What stays in
//! the cell is the state that is *supposed* to reset — "focus me on my first
//! frame", "flash, my value just changed".
//!
//! `sheet.rs`, `formula.rs` and `eval.rs` never mention egui, the same way
//! `board.rs` and `graph.rs` do not, so every rule in them is a unit test
//! rather than a screenshot.

use egui_react::prelude::*;
use egui_react_elements::prelude::*;
use example_meta::Meta;

pub mod eval;
pub mod formula;
pub mod preset;
pub mod sheet;

pub use sheet::{COLS, DEFAULT_COL_W, ROWS};

pub const META: Meta = Meta {
    name: "spreadsheet",
    summary: "Formulas over 26 x 10,000 cells: two memo stages, and a draft that survives scrolling out of view.",
    hooks: &["use_state"],
    elements: &["View", "Text"],
    source: include_str!("lib.rs"),
    plain: None,
};

#[component]
pub fn App(cx: &mut Cx) {
    rsx! {
        <View direction="column" align="center" justify="center" gap={8} grow={1.0}>
            <Text strong>"spreadsheet"</Text>
        </View>
    }
}
