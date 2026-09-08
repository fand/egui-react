//! The sheet the example opens with, built in code.
//!
//! A gallery shows an example the moment it is picked, and an empty grid says
//! nothing: it does not show that a `SUM` follows its column, that a total row
//! recalculates when one number changes, or that an `IF` reads two other
//! formulas. So the first thing on screen is a small monthly budget with
//! something in every corner of the language.
//!
//! It is built here rather than read from a file because there is no `<Suspense>`
//! in this example to justify a load — `patch` covers that — and because a
//! sheet written as Rust can be read next to the numbers it produces.

use crate::sheet::{CellRef, Sheet};

/// The columns the six months live in: `B` to `G`.
const MONTHS: [&str; 6] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun"];

/// The rows the categories live in, and what they spend each month.
///
/// Deliberately small: about ninety cells, which is enough to show every
/// function and few enough that the whole sheet is on screen at once.
const CATEGORIES: [(&str, [f64; 6]); 6] = [
    ("Rent", [1200.0, 1200.0, 1200.0, 1250.0, 1250.0, 1250.0]),
    ("Groceries", [420.0, 388.5, 455.0, 402.0, 517.25, 468.0]),
    ("Transport", [95.0, 110.0, 88.5, 132.0, 76.0, 104.5]),
    ("Utilities", [180.0, 172.0, 145.0, 121.0, 98.0, 92.5]),
    ("Fun", [210.0, 340.0, 155.0, 280.0, 395.0, 612.0]),
    ("Savings", [500.0, 500.0, 250.0, 500.0, 500.0, 0.0]),
];

/// What each month is allowed to cost, which is what the `IF` row compares
/// against.
const BUDGET: [f64; 6] = [2600.0, 2700.0, 2300.0, 2700.0, 2800.0, 2600.0];

/// The row a category's numbers start on, 0-based (row 2 on screen).
const FIRST: u32 = 1;
/// The row after the last category, 0-based (the blank row 8 on screen).
const LAST: u32 = FIRST + CATEGORIES.len() as u32;

/// The starter sheet: six months of six categories, a `SUM` down every column
/// and across every row, an `AVG` row, a budget, and an `IF` that says whether
/// the month went over it.
///
/// Row 8 is left blank on purpose. It sits inside the `SUM` ranges, so the
/// sheet also demonstrates on its first frame that an empty cell inside a
/// range is skipped rather than counted as a zero.
pub fn starter() -> Sheet {
    let mut cells: Vec<(CellRef, String)> = Vec::new();
    let mut put = |col: u16, row: u32, text: String| cells.push((CellRef::new(col, row), text));

    // Row 1: the header.
    put(0, 0, String::from("Category"));
    for (i, month) in MONTHS.iter().enumerate() {
        put(1 + i as u16, 0, String::from(*month));
    }
    put(7, 0, String::from("Total"));

    // Rows 2..7: one category each, with its own total across.
    for (r, (name, amounts)) in CATEGORIES.iter().enumerate() {
        let row = FIRST + r as u32;
        let n = row + 1;
        put(0, row, String::from(*name));
        for (i, amount) in amounts.iter().enumerate() {
            put(1 + i as u16, row, number(*amount));
        }
        put(7, row, format!("=SUM(B{n}:G{n})"));
    }

    // Row 9: the total per month, and the grand total in the corner.
    let total_row = LAST + 1;
    let n = total_row + 1;
    let (first, last) = (FIRST + 1, LAST);
    put(0, total_row, String::from("Total"));
    for col in 1..=7u16 {
        let c = char::from(b'A' + col as u8);
        put(col, total_row, format!("=SUM({c}{first}:{c}{last})"));
    }

    // Row 10: the average per month, over the same rectangle.
    let avg_row = total_row + 1;
    put(0, avg_row, String::from("Average"));
    for col in 1..=7u16 {
        let c = char::from(b'A' + col as u8);
        put(col, avg_row, format!("=AVG({c}{first}:{c}{last})"));
    }

    // Row 11: what each month was allowed to cost.
    let budget_row = avg_row + 1;
    put(0, budget_row, String::from("Budget"));
    for (i, amount) in BUDGET.iter().enumerate() {
        put(1 + i as u16, budget_row, number(*amount));
    }
    let b = budget_row + 1;
    put(7, budget_row, format!("=SUM(B{b}:G{b})"));

    // Row 12: the comparison, one formula reading two others.
    let status_row = budget_row + 1;
    put(0, status_row, String::from("Status"));
    for col in 1..=7u16 {
        let c = char::from(b'A' + col as u8);
        put(
            col,
            status_row,
            format!("=IF({c}{n}>{c}{b},\"over\",\"ok\")"),
        );
    }

    Sheet::from_cells(cells)
}

/// A number as a user would have typed it: no trailing `.0` on a whole one.
///
/// The preset goes in as *text*, the same text a keystroke would produce, so
/// that nothing in the sheet is reachable only by loading a preset.
fn number(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{n:.0}")
    } else {
        format!("{n}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eval::{compile, evaluate};
    use crate::formula::{Error, Value};

    fn cell(name: &str) -> CellRef {
        CellRef::parse(name).expect("a reference the test wrote")
    }

    #[test]
    fn the_starter_sheet_is_small_and_adds_up() {
        let sheet = starter();
        assert!(sheet.len() < 100, "{} cells", sheet.len());

        let compiled = compile(&sheet);
        let values = evaluate(&sheet, &compiled);

        // Nothing in it is broken: no cycle, no parse error, no `#VALUE!`.
        assert!(compiled.cyclic.is_empty());
        for (at, value) in &values.cells {
            assert!(
                !matches!(value, Value::Err(_)),
                "{} says {value:?}",
                at.name()
            );
        }

        // Rent across six months, and the column below it.
        assert_eq!(*values.get(cell("H2")), Value::Num(7350.0));
        assert_eq!(*values.get(cell("B9")), Value::Num(2605.0));
        // The grand total is reachable both ways round, which is what makes
        // the sheet a graph rather than a list.
        assert_eq!(*values.get(cell("H9")), Value::Num(15_657.25));
        assert_eq!(*values.get(cell("H10")), Value::Num(15_657.25 / 6.0));

        // And the `IF` row reads the two rows above it.
        assert_eq!(*values.get(cell("B12")), Value::Text(String::from("over")));
        assert_eq!(*values.get(cell("D12")), Value::Text(String::from("ok")));
    }

    #[test]
    fn a_number_in_the_preset_is_the_text_a_user_would_have_typed() {
        assert_eq!(number(1200.0), "1200");
        assert_eq!(number(388.5), "388.5");
        let sheet = starter();
        assert_eq!(sheet.text(cell("B2")), "1200");
        assert_eq!(sheet.text(cell("C3")), "388.5");
        assert_eq!(sheet.text(cell("H2")), "=SUM(B2:G2)");
        assert_eq!(sheet.text(cell("B9")), "=SUM(B2:B7)");
        assert_eq!(sheet.text(cell("B10")), "=AVG(B2:B7)");
        assert_eq!(sheet.text(cell("B12")), "=IF(B9>B11,\"over\",\"ok\")");
        assert_eq!(sheet.text(cell("A8")), "", "the blank row inside the sums");
    }

    #[test]
    fn breaking_one_cell_of_the_preset_shows_where_it_reaches() {
        // Not a test of the preset so much as of what the preset is for: one
        // number, and everything downstream of it moves.
        let mut sheet = starter();
        crate::sheet::reduce(
            &mut sheet,
            crate::sheet::Msg::Set {
                at: cell("B2"),
                text: String::from("oops"),
            },
        );
        let compiled = compile(&sheet);
        let values = evaluate(&sheet, &compiled);
        // The row total skips text, so it only loses the rent.
        assert_eq!(*values.get(cell("H2")), Value::Num(6150.0));
        assert_eq!(*values.get(cell("B9")), Value::Num(1405.0));
        assert_eq!(*values.get(cell("B12")), Value::Text(String::from("ok")));
        assert_ne!(*values.get(cell("C9")), Value::Err(Error::Value));
    }
}
