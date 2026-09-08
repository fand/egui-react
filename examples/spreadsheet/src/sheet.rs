//! The sheet: cell references, rectangles of them, the text in each cell, the
//! messages that change it, and the reducer.
//!
//! Nothing here knows about egui — a column width is an `f32`, not a
//! `Rangef` — so the whole document can be built and reduced in a unit test,
//! and `formula.rs` and `eval.rs` next door can turn it into values without
//! either side of the screen being involved.
//!
//! The two revision counters are the point of the file, in the same shape as
//! `patch`'s `topology_rev` / `param_rev`. Every message says, by which
//! counter it bumps, whether it can change the *program* (which cells hold
//! formulas, what they read, in what order they must be evaluated) or only the
//! *numbers the program reads*. That single distinction is what lets the UI
//! re-parse the sheet when a formula is typed and not when a number is.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// How many columns the sheet has: `A`..`Z`.
///
/// A `pub const` rather than a field, because every reference in the file is
/// bounded by it — [`CellRef::parse`] refuses `AA1` — and a bound that can
/// change per document would have to be threaded through the parser.
pub const COLS: u16 = 26;

/// How many rows the sheet has: `1`..`10000`.
///
/// Large enough that drawing them all is out of the question, which is what
/// makes the grid a `<VirtualList>` and not a `<View>` per row.
pub const ROWS: u32 = 10_000;

/// The width a column starts at, in points.
///
/// It lives here rather than with the other screen constants because
/// [`Sheet::empty`] builds the width vector and a sheet has to be buildable
/// without a screen; `lib.rs` re-exports it next to `ROW_H` and friends.
pub const DEFAULT_COL_W: f32 = 80.0;

/// One cell, 0-based on both axes: `A1` is `(0, 0)`.
///
/// 0-based because every use of it is arithmetic — an arrow key adds one, a
/// paste adds a delta, a range iterates — and the only place the 1-based
/// spelling matters is [`CellRef::name`], which is where it is put back.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CellRef {
    /// Ordered before `col` deliberately: sorting a `Vec<CellRef>` then gives
    /// reading order, which is the order the dependency inspector and the
    /// evaluation queue want.
    pub row: u32,
    pub col: u16,
}

impl CellRef {
    /// A reference, unchecked. Out-of-sheet values are refused where they can
    /// arrive from outside ([`CellRef::parse`], [`shifted`]); this is the
    /// constructor for coordinates the caller already knows are in range.
    pub const fn new(col: u16, row: u32) -> Self {
        Self { row, col }
    }

    /// The spelling a user sees: `"B2"`.
    pub fn name(self) -> String {
        let col = char::from(b'A' + (self.col % COLS) as u8);
        format!("{}{}", col, self.row + 1)
    }

    /// Read a reference a user typed. Case-insensitive, surrounding space
    /// allowed, `None` for anything outside `COLS` x `ROWS`.
    ///
    /// `None` rather than a clamp: `AA1` and `A0` are mistakes, and a formula
    /// that silently meant `Z1` instead would be worse than `#REF!`. This is
    /// also the only place cell text becomes a reference, so it is the only
    /// place that has to be careful, and it never panics.
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let mut chars = s.chars();
        let letter = chars.next()?;
        if !letter.is_ascii_alphabetic() {
            return None;
        }
        let digits = chars.as_str();
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        // A row number longer than a `u32` fails here rather than wrapping.
        let row: u32 = digits.parse().ok()?;
        if row == 0 || row > ROWS {
            return None;
        }
        let col = u16::from(letter.to_ascii_uppercase() as u8 - b'A');
        if col >= COLS {
            return None;
        }
        Some(Self::new(col, row - 1))
    }
}

/// The same cell moved by `(dcol, drow)`, or `None` if that leaves the sheet.
///
/// This is what a paste does to every reference in the text it carries, so the
/// answer to "off the edge" has to be a value and not a panic; the caller
/// writes `#REF!` into the text instead (`formula::shift`).
pub fn shifted(at: CellRef, dcol: i32, drow: i32) -> Option<CellRef> {
    let col = i64::from(at.col) + i64::from(dcol);
    let row = i64::from(at.row) + i64::from(drow);
    if col < 0 || col >= i64::from(COLS) || row < 0 || row >= i64::from(ROWS) {
        return None;
    }
    Some(CellRef::new(col as u16, row as u32))
}

/// A rectangle of cells, normalised so that `from <= to` on both axes.
///
/// The UI holds a selection as an anchor and a cursor, either of which may be
/// the top left; normalising once here means nothing downstream — the tint,
/// the delete, the copy — has to ask which way round it is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Range {
    pub from: CellRef,
    pub to: CellRef,
}

impl Range {
    /// The rectangle spanned by two corners, in either order.
    pub fn new(a: CellRef, b: CellRef) -> Self {
        Self {
            from: CellRef::new(a.col.min(b.col), a.row.min(b.row)),
            to: CellRef::new(a.col.max(b.col), a.row.max(b.row)),
        }
    }

    /// One cell.
    pub fn one(at: CellRef) -> Self {
        Self { from: at, to: at }
    }

    pub fn contains(self, at: CellRef) -> bool {
        (self.from.col..=self.to.col).contains(&at.col)
            && (self.from.row..=self.to.row).contains(&at.row)
    }

    pub fn cols(self) -> u16 {
        self.to.col - self.from.col + 1
    }

    pub fn rows(self) -> u32 {
        self.to.row - self.from.row + 1
    }

    /// Every cell in the rectangle, in reading order.
    ///
    /// A rectangle can be the whole sheet, so callers that only care about
    /// cells that hold something walk [`Sheet::cells`] and ask
    /// [`Range::contains`] instead — [`reduce`] does exactly that for
    /// [`Msg::Clear`].
    pub fn cells(self) -> impl Iterator<Item = CellRef> {
        (self.from.row..=self.to.row).flat_map(move |row| {
            (self.from.col..=self.to.col).map(move |col| CellRef::new(col, row))
        })
    }
}

/// The whole document.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sheet {
    /// The raw text as typed, per cell. An absent key is an empty cell; a cell
    /// is never stored as `""`, so "is this sheet empty" is one `len` and a
    /// deleted cell costs nothing.
    #[serde(with = "cells_by_name")]
    pub cells: HashMap<CellRef, String>,
    /// Per column, in points. Always `COLS` long.
    ///
    /// In the document rather than in the UI state so that a resize is undone
    /// and saved with everything else — a width the user dragged is a change
    /// to the sheet, not a preference.
    pub col_w: Vec<f32>,
    /// Bumped by every change that could alter *which* cells are formulas,
    /// what they read, or in what order they must be evaluated.
    ///
    /// This is the deps of the memo that parses the sheet and orders it.
    pub structure_rev: u64,
    /// Bumped by every change to any cell's text.
    ///
    /// This is the deps of the memo that evaluates. A number typed into a
    /// literal cell bumps this one only, which is the whole example in a line.
    pub value_rev: u64,
}

impl Default for Sheet {
    fn default() -> Self {
        Self::empty()
    }
}

impl Sheet {
    /// A sheet with nothing in it and every column at its default width.
    pub fn empty() -> Self {
        Self {
            cells: HashMap::new(),
            col_w: vec![DEFAULT_COL_W; COLS as usize],
            structure_rev: 0,
            value_rev: 0,
        }
    }

    /// A sheet built from cells, for the preset and for tests.
    ///
    /// Both counters start at zero; [`Msg::Load`] steps them past whatever the
    /// running sheet was at when a sheet like this is dropped in.
    pub fn from_cells<I, S>(cells: I) -> Self
    where
        I: IntoIterator<Item = (CellRef, S)>,
        S: Into<String>,
    {
        let mut sheet = Self::empty();
        for (at, text) in cells {
            let text = text.into();
            if !text.is_empty() {
                sheet.cells.insert(at, text);
            }
        }
        sheet
    }

    /// What was typed into a cell, or `""`.
    pub fn text(&self, at: CellRef) -> &str {
        self.cells.get(&at).map_or("", String::as_str)
    }

    /// How many cells hold something. The status bar shows it.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// The width of one column, falling back to the default for a column index
    /// the saved width vector does not cover (an older save, a shorter sheet).
    pub fn width(&self, col: u16) -> f32 {
        self.col_w
            .get(col as usize)
            .copied()
            .unwrap_or(DEFAULT_COL_W)
    }
}

/// Everything that can change a sheet. One message per user action.
#[derive(Clone, Debug, PartialEq)]
pub enum Msg {
    /// One cell. Empty text removes it.
    Set { at: CellRef, text: String },
    /// Several cells in one step, so that a paste is one undo and one
    /// re-parse rather than one per cell.
    SetMany(Vec<(CellRef, String)>),
    /// Every cell in the rectangle (the Delete key).
    Clear(Range),
    /// Sent once, on release: a width per pixel of drag would fill the undo
    /// history with a hundred steps that all say the same thing (the same
    /// reasoning as `patch`'s `MoveNode`).
    SetColWidth { col: u16, w: f32 },
    /// Replace the sheet with another one, stepping both counters past the
    /// current values so the memos below cannot mistake it for the old sheet.
    Load(Box<Sheet>),
}

/// Whether a cell's text is a formula rather than a value.
///
/// One character, one place: the tokenizer, the reducer and the UI all ask
/// this question and all have to agree on the answer.
pub fn is_formula(text: &str) -> bool {
    text.starts_with('=')
}

/// Apply one message. The only function that changes a sheet.
///
/// Which counter a message bumps *is* the specification of this example:
///
/// | message | `structure_rev` | `value_rev` |
/// |---|---|---|
/// | `Set`, both texts literals | – | +1 |
/// | `Set`, either text a formula | +1 | +1 |
/// | `SetMany` / `Clear` | +1 if a formula was involved | +1 if anything changed |
/// | `SetColWidth` | – | – |
/// | `Load` | +1 | +1 |
///
/// A message that changes nothing bumps nothing and leaves the sheet equal to
/// what it was, so `use_undoable` records no step for it and neither memo
/// reruns.
pub fn reduce(sheet: &mut Sheet, msg: Msg) {
    match msg {
        Msg::Set { at, text } => {
            let (changed, structural) = set_one(sheet, at, text);
            bump(sheet, changed, structural);
        }

        Msg::SetMany(cells) => {
            let mut changed = false;
            let mut structural = false;
            for (at, text) in cells {
                let (c, s) = set_one(sheet, at, text);
                changed |= c;
                structural |= s;
            }
            bump(sheet, changed, structural);
        }

        Msg::Clear(range) => {
            // Walk what the sheet holds rather than what the rectangle covers:
            // a selection can be the whole 26 x 10,000 grid, and clearing it
            // is a question about a few hundred keys.
            let doomed: Vec<CellRef> = sheet
                .cells
                .keys()
                .copied()
                .filter(|at| range.contains(*at))
                .collect();
            let mut changed = false;
            let mut structural = false;
            for at in doomed {
                let (c, s) = set_one(sheet, at, String::new());
                changed |= c;
                structural |= s;
            }
            bump(sheet, changed, structural);
        }

        Msg::SetColWidth { col, w } => {
            // Neither counter: a width changes where the cells are drawn, not
            // what they say. The sheet still differs from the one before it,
            // so the undo history keeps the step.
            if let Some(slot) = sheet.col_w.get_mut(col as usize)
                && *slot != w
            {
                *slot = w;
            }
        }

        Msg::Load(next) => {
            let structure_rev = sheet.structure_rev.max(next.structure_rev) + 1;
            let value_rev = sheet.value_rev.max(next.value_rev) + 1;
            *sheet = *next;
            sheet.structure_rev = structure_rev;
            sheet.value_rev = value_rev;
        }
    }
}

/// Write one cell, reporting `(anything changed, a formula was involved)`.
///
/// "A formula was involved" is asked of the text on both sides: replacing a
/// formula with a number changes the parse just as much as typing one does.
fn set_one(sheet: &mut Sheet, at: CellRef, text: String) -> (bool, bool) {
    if at.col >= COLS || at.row >= ROWS {
        return (false, false);
    }
    let old = sheet.text(at);
    if old == text {
        return (false, false);
    }
    let structural = is_formula(old) || is_formula(&text);
    if text.is_empty() {
        sheet.cells.remove(&at);
    } else {
        sheet.cells.insert(at, text);
    }
    (true, structural)
}

fn bump(sheet: &mut Sheet, changed: bool, structural: bool) {
    if changed {
        sheet.value_rev += 1;
    }
    if structural {
        sheet.structure_rev += 1;
    }
}

/// `cells` as a `"B2" -> text` map.
///
/// A `HashMap` with a struct key has no JSON spelling, and `use_persisted`
/// saves JSON. Writing the key as the name the user sees also means a saved
/// sheet can be read by a human. A key that cannot be read back is dropped
/// with a warning rather than failing the whole load: one broken entry should
/// not cost the user the rest of the sheet.
mod cells_by_name {
    use super::{CellRef, HashMap};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        cells: &HashMap<CellRef, String>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        s.collect_map(cells.iter().map(|(at, text)| (at.name(), text)))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> Result<HashMap<CellRef, String>, D::Error> {
        let raw = HashMap::<String, String>::deserialize(d)?;
        Ok(raw
            .into_iter()
            .filter_map(|(name, text)| match CellRef::parse(&name) {
                Some(at) => Some((at, text)),
                None => {
                    log::warn!(
                        "spreadsheet: dropping the saved cell {name:?}, which is not a reference"
                    );
                    None
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(sheet: &mut Sheet, name: &str, text: &str) {
        reduce(
            sheet,
            Msg::Set {
                at: cell(name),
                text: String::from(text),
            },
        );
    }

    fn cell(name: &str) -> CellRef {
        CellRef::parse(name).expect("a reference the test wrote")
    }

    /// `(structure_rev, value_rev)`, the pair every test below compares.
    fn revs(sheet: &Sheet) -> (u64, u64) {
        (sheet.structure_rev, sheet.value_rev)
    }

    #[test]
    fn a_reference_reads_and_writes_its_name() {
        assert_eq!(cell("A1"), CellRef::new(0, 0));
        assert_eq!(cell("B2"), CellRef::new(1, 1));
        assert_eq!(cell("z9999"), CellRef::new(25, 9998));
        assert_eq!(cell(" b2 "), CellRef::new(1, 1));
        assert_eq!(cell("A1").name(), "A1");
        assert_eq!(cell("z9999").name(), "Z9999");
        assert_eq!(CellRef::new(7, 41).name(), "H42");
    }

    #[test]
    fn a_reference_outside_the_sheet_is_not_a_reference() {
        // Only 26 columns, so a two-letter column is a mistake, not column 27.
        assert_eq!(CellRef::parse("AA1"), None);
        assert_eq!(CellRef::parse("A0"), None, "rows are 1-based on screen");
        assert_eq!(CellRef::parse("A10001"), None);
        assert_eq!(CellRef::parse("A"), None);
        assert_eq!(CellRef::parse("1"), None);
        assert_eq!(CellRef::parse(""), None);
        assert_eq!(CellRef::parse("A1B"), None);
        assert_eq!(CellRef::parse("A99999999999999999999"), None);
    }

    #[test]
    fn a_shifted_reference_stops_at_the_edge() {
        assert_eq!(shifted(cell("B2"), 1, 1), Some(cell("C3")));
        assert_eq!(shifted(cell("B2"), -1, -1), Some(cell("A1")));
        assert_eq!(shifted(cell("A1"), -1, 0), None);
        assert_eq!(shifted(cell("A1"), 0, -1), None);
        assert_eq!(shifted(cell("Z1"), 1, 0), None);
        assert_eq!(shifted(cell("A10000"), 0, 1), None);
    }

    #[test]
    fn a_range_is_normalised_and_iterates_in_reading_order() {
        let range = Range::new(cell("C3"), cell("B2"));
        assert_eq!(range.from, cell("B2"));
        assert_eq!(range.to, cell("C3"));
        assert_eq!(range.cols(), 2);
        assert_eq!(range.rows(), 2);
        assert_eq!(
            range.cells().collect::<Vec<_>>(),
            vec![cell("B2"), cell("C2"), cell("B3"), cell("C3")]
        );
        assert!(range.contains(cell("C2")));
        assert!(!range.contains(cell("D2")));
    }

    #[test]
    fn a_literal_bumps_only_the_value_revision() {
        let mut sheet = Sheet::empty();
        set(&mut sheet, "A1", "12");
        assert_eq!(revs(&sheet), (0, 1), "nothing to re-parse");
        set(&mut sheet, "A1", "13");
        assert_eq!(revs(&sheet), (0, 2));
        // Text is a literal too: still nothing to parse.
        set(&mut sheet, "A2", "hello");
        assert_eq!(revs(&sheet), (0, 3));
    }

    #[test]
    fn a_formula_bumps_both_revisions() {
        let mut sheet = Sheet::empty();
        set(&mut sheet, "A1", "=1+1");
        assert_eq!(revs(&sheet), (1, 1));
        // Changing the formula's text is another parse.
        set(&mut sheet, "A1", "=1+2");
        assert_eq!(revs(&sheet), (2, 2));
    }

    #[test]
    fn switching_between_a_formula_and_a_literal_bumps_both() {
        let mut sheet = Sheet::empty();
        set(&mut sheet, "A1", "=1+1");
        assert_eq!(revs(&sheet), (1, 1));
        set(&mut sheet, "A1", "7");
        assert_eq!(revs(&sheet), (2, 2), "the old text was a formula");
        set(&mut sheet, "A1", "");
        assert_eq!(revs(&sheet), (2, 3), "and now it is only a number leaving");
    }

    #[test]
    fn writing_the_same_text_again_bumps_nothing() {
        let mut sheet = Sheet::empty();
        set(&mut sheet, "A1", "12");
        set(&mut sheet, "B1", "=A1");
        let before = revs(&sheet);
        set(&mut sheet, "A1", "12");
        set(&mut sheet, "B1", "=A1");
        set(&mut sheet, "C1", "");
        assert_eq!(revs(&sheet), before, "no undo step, and no memo rerun");
    }

    #[test]
    fn empty_text_removes_the_cell() {
        let mut sheet = Sheet::empty();
        set(&mut sheet, "A1", "12");
        assert_eq!(sheet.len(), 1);
        set(&mut sheet, "A1", "");
        assert!(
            sheet.is_empty(),
            "a cell is never stored as an empty string"
        );
        assert_eq!(sheet.text(cell("A1")), "");
    }

    #[test]
    fn set_many_bumps_the_structure_only_when_a_formula_is_involved() {
        let mut sheet = Sheet::empty();
        reduce(
            &mut sheet,
            Msg::SetMany(vec![
                (cell("A1"), String::from("1")),
                (cell("A2"), String::from("2")),
            ]),
        );
        assert_eq!(revs(&sheet), (0, 1), "one step for the whole paste");

        reduce(
            &mut sheet,
            Msg::SetMany(vec![
                (cell("B1"), String::from("3")),
                (cell("B2"), String::from("=A1")),
            ]),
        );
        assert_eq!(revs(&sheet), (1, 2));

        // Nothing in this one changes, so nothing is bumped.
        reduce(
            &mut sheet,
            Msg::SetMany(vec![
                (cell("A1"), String::from("1")),
                (cell("B2"), String::from("=A1")),
            ]),
        );
        assert_eq!(revs(&sheet), (1, 2));
    }

    #[test]
    fn clear_bumps_the_structure_only_when_it_removes_a_formula() {
        let mut sheet = Sheet::empty();
        set(&mut sheet, "A1", "1");
        set(&mut sheet, "A2", "2");
        set(&mut sheet, "C1", "=A1");
        let before = revs(&sheet);

        reduce(&mut sheet, Msg::Clear(Range::new(cell("A1"), cell("A2"))));
        assert_eq!(revs(&sheet), (before.0, before.1 + 1), "two literals went");
        assert_eq!(sheet.len(), 1);

        reduce(&mut sheet, Msg::Clear(Range::new(cell("C1"), cell("C1"))));
        assert_eq!(revs(&sheet), (before.0 + 1, before.1 + 2));
        assert!(sheet.is_empty());

        // An empty rectangle changes nothing.
        let before = revs(&sheet);
        reduce(&mut sheet, Msg::Clear(Range::new(cell("A1"), cell("Z100"))));
        assert_eq!(revs(&sheet), before);
    }

    #[test]
    fn a_column_width_bumps_neither_revision() {
        let mut sheet = Sheet::empty();
        set(&mut sheet, "A1", "1");
        let before = revs(&sheet);
        reduce(&mut sheet, Msg::SetColWidth { col: 0, w: 140.0 });
        assert_eq!(sheet.width(0), 140.0);
        assert_eq!(
            revs(&sheet),
            before,
            "the values did not move, only the cells"
        );
        // A column that does not exist is ignored rather than panicking.
        reduce(&mut sheet, Msg::SetColWidth { col: 999, w: 10.0 });
        assert_eq!(sheet.col_w.len(), COLS as usize);
    }

    #[test]
    fn loading_steps_past_the_revisions_the_running_sheet_reached() {
        let mut sheet = Sheet::empty();
        for i in 0..5 {
            set(&mut sheet, "A1", &format!("={i}"));
        }
        let before = revs(&sheet);
        assert_eq!(before, (5, 5));

        // The preset's own counters are zero, so a memo keyed on them would
        // see the deps go *backwards* without this.
        let preset = Sheet::from_cells([(cell("A1"), "=1+1")]);
        assert_eq!(revs(&preset), (0, 0));
        reduce(&mut sheet, Msg::Load(Box::new(preset)));
        assert_eq!(revs(&sheet), (before.0 + 1, before.1 + 1));
        assert_eq!(sheet.text(cell("A1")), "=1+1");
    }

    #[test]
    fn a_sheet_survives_a_round_trip_through_json() {
        let mut sheet = Sheet::from_cells([(cell("A1"), "12"), (cell("H42"), "=SUM(A1:A9)")]);
        reduce(&mut sheet, Msg::SetColWidth { col: 3, w: 120.0 });
        let json = serde_json::to_string(&sheet).expect("a sheet serialises");
        assert!(json.contains("\"H42\""), "keys are the names a user reads");
        let back: Sheet = serde_json::from_str(&json).expect("and reads back");
        assert_eq!(back, sheet);
    }

    #[test]
    fn a_saved_cell_that_is_not_a_reference_is_dropped_not_fatal() {
        let json = r#"{"cells":{"A1":"12","nonsense":"9"},"col_w":[80.0],"structure_rev":3,"value_rev":4}"#;
        let sheet: Sheet = serde_json::from_str(json).expect("the rest still loads");
        assert_eq!(sheet.len(), 1);
        assert_eq!(sheet.text(cell("A1")), "12");
        // A width vector shorter than the sheet falls back per column.
        assert_eq!(sheet.width(0), 80.0);
        assert_eq!(sheet.width(9), DEFAULT_COL_W);
    }

    #[test]
    fn is_formula_looks_at_one_character() {
        assert!(is_formula("=1+1"));
        assert!(is_formula("="));
        assert!(!is_formula(" =1+1"));
        assert!(!is_formula("1+1"));
        assert!(!is_formula(""));
    }
}
