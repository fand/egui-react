//! Two functions over a sheet: parse it, and work out what every cell says.
//!
//! They are two functions rather than one because they change for different
//! reasons, and that is the whole example. [`compile`] answers "which cells
//! are formulas, what does each read, and in what order can they be
//! evaluated" — a question only a *formula* can change. [`evaluate`] answers
//! "what does each cell say" — a question any keystroke can change. The screen
//! hangs one `use_memo` off each, keyed on `structure_rev` and `value_rev`, so
//! typing a number into a literal cell re-runs the second and not the first.
//!
//! Neither is incremental. The whole sheet is evaluated whenever a revision
//! changes, which for the sizes here (a few hundred formulas) is microseconds
//! and far under a frame. Recalculating only the dirty cells is explicitly out
//! of scope: it would make the code about the algorithm rather than about the
//! two memo stages.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::formula::{self, Error, Expr, Func, Op, Value};
use crate::sheet::{CellRef, Range, Sheet, is_formula};

/// A sheet, parsed and ordered. The output of the first memo stage.
#[derive(Clone, Debug, Default)]
pub struct Compiled {
    /// Every formula cell, parsed. Literal cells are not here, because a
    /// literal has no syntax to get wrong and no dependencies to order.
    pub formulas: HashMap<CellRef, Result<Expr, Error>>,
    /// The formula cells in an order where everything a cell reads comes
    /// first. Cells on, or downstream of, a cycle are absent.
    pub order: Vec<CellRef>,
    /// Formula cells that are on, or downstream of, a cycle.
    ///
    /// One set for both, because the answer is the same: a cell that cannot be
    /// evaluated because one of its inputs cannot be evaluated shows
    /// `#CYCLE!`. A real spreadsheet propagates the error cell by cell and
    /// arrives at the same place by a longer route.
    pub cyclic: HashSet<CellRef>,
    /// Per formula cell, the cells it reads, in reading order.
    ///
    /// Kept after the ordering is done because the status bar shows it under
    /// the cursor — "what does this cell depend on" is the question a
    /// spreadsheet user asks most and the one no spreadsheet answers well.
    pub deps: HashMap<CellRef, Vec<CellRef>>,
}

/// What every cell says. The output of the second memo stage.
#[derive(Clone, Debug, Default)]
pub struct Values {
    /// Formula cells and literal cells. An absent key is [`Value::Empty`].
    pub cells: HashMap<CellRef, Value>,
}

impl Values {
    /// What a cell says, `Empty` if it says nothing.
    pub fn get(&self, at: CellRef) -> &Value {
        // A `static` rather than a `const`: `Value` owns a `String` in one of
        // its variants, so a temporary cannot be promoted to `'static`.
        static EMPTY: Value = Value::Empty;
        self.cells.get(&at).unwrap_or(&EMPTY)
    }
}

/// Parse every formula in the sheet and put the formulas in an order that can
/// be evaluated.
///
/// The ordering is Kahn's algorithm over the formula cells, and it is chosen
/// over a depth-first search for two reasons that both matter here: it is
/// iterative, so a chain ten thousand cells long cannot overflow the stack,
/// and what it leaves behind when it runs out of work is exactly "everything
/// on or after a cycle", which is the set [`Values`] has to fill with
/// `#CYCLE!`. A reference to a literal or an empty cell is a leaf: it needs no
/// ordering, only a value, and it already has one.
pub fn compile(sheet: &Sheet) -> Compiled {
    let mut formulas = HashMap::new();
    let mut deps = HashMap::new();

    for (at, text) in &sheet.cells {
        if !is_formula(text) {
            continue;
        }
        let parsed = formula::parse(&text[1..]);
        deps.insert(*at, parsed.as_ref().map(formula::refs).unwrap_or_default());
        formulas.insert(*at, parsed);
    }

    // Edges point from a cell to the formula cells that read it. Only formula
    // cells are nodes; a reference to a literal is already satisfied.
    let mut waiting_on: HashMap<CellRef, usize> = HashMap::new();
    let mut readers: HashMap<CellRef, Vec<CellRef>> = HashMap::new();
    for (at, refs) in &deps {
        let mut count = 0;
        for dep in refs {
            if dep != at && formulas.contains_key(dep) {
                readers.entry(*dep).or_default().push(*at);
                count += 1;
            } else if dep == at {
                // A cell that reads itself waits on itself forever, which is
                // what makes a self-reference fall out as a cycle below.
                count += 1;
            }
        }
        waiting_on.insert(*at, count);
    }

    // A `BTreeSet` and not a `Vec`: the ready set is drained in reading order,
    // so the order — and every test that reads it — is the same on every run,
    // whatever order the `HashMap` handed the cells over in.
    let mut ready: BTreeSet<CellRef> = waiting_on
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(at, _)| *at)
        .collect();

    let mut order = Vec::with_capacity(formulas.len());
    while let Some(at) = ready.pop_first() {
        order.push(at);
        for reader in readers.get(&at).into_iter().flatten() {
            if let Some(count) = waiting_on.get_mut(reader) {
                *count -= 1;
                if *count == 0 {
                    ready.insert(*reader);
                }
            }
        }
    }

    // Whatever never became ready is on a cycle or reads something that is.
    let cyclic: HashSet<CellRef> = formulas
        .keys()
        .copied()
        .filter(|at| waiting_on.get(at).is_some_and(|count| *count > 0))
        .collect();

    Compiled {
        formulas,
        order,
        cyclic,
        deps,
    }
}

/// Work out what every cell says.
///
/// Literals first — they are the leaves, and they are the same whatever the
/// formulas do — then the formulas in [`Compiled::order`], each against the
/// values already written. Because the order is topological, every reference a
/// formula makes has already been filled in, so this is one pass with no
/// recursion between cells.
pub fn evaluate(sheet: &Sheet, compiled: &Compiled) -> Values {
    let mut cells: HashMap<CellRef, Value> = HashMap::with_capacity(sheet.len());

    for (at, text) in &sheet.cells {
        if !is_formula(text) {
            cells.insert(*at, formula::literal(text));
        }
    }

    for at in &compiled.order {
        let value = match compiled.formulas.get(at) {
            Some(Ok(expr)) => eval(expr, &cells),
            // A formula that would not parse says so where it stands; the
            // cells that read it get the error passed on like any other value.
            Some(Err(error)) => Value::Err(*error),
            None => Value::Empty,
        };
        cells.insert(*at, value);
    }

    for at in &compiled.cyclic {
        cells.insert(*at, Value::Err(Error::Cycle));
    }

    Values { cells }
}

/// Evaluate one expression against the values written so far.
fn eval(expr: &Expr, cells: &HashMap<CellRef, Value>) -> Value {
    match expr {
        Expr::Num(n) => Value::Num(*n),
        Expr::Str(s) => Value::Text(s.clone()),
        Expr::Ref(at) => cells.get(at).cloned().unwrap_or(Value::Empty),
        // A rectangle is not a value. It is only ever an argument, and the
        // functions that take one reach past this arm.
        Expr::Range(..) => Value::Err(Error::Value),
        Expr::Neg(inner) => match number(&eval(inner, cells)) {
            Ok(n) => finite(-n),
            Err(error) => Value::Err(error),
        },
        Expr::Bin(op, a, b) => binary(*op, &eval(a, cells), &eval(b, cells)),
        Expr::Call(func, args) => call(*func, args, cells),
    }
}

/// A value as a number, for arithmetic.
///
/// `Empty` is zero: a `SUM` over a column with gaps in it should not be an
/// error, and neither should `=A1+1` on a cell the user has not filled in yet.
/// Text is `#VALUE!` — there is no coercion from `"12"`, because a cell whose
/// text reads as a number is already a number ([`formula::literal`]).
fn number(value: &Value) -> Result<f64, Error> {
    match value {
        Value::Empty => Ok(0.0),
        Value::Num(n) => Ok(*n),
        Value::Text(_) => Err(Error::Value),
        Value::Err(error) => Err(*error),
    }
}

/// Wrap an arithmetic result, turning an overflow or a `NaN` into `#VALUE!`
/// rather than letting `inf` spread through the sheet.
fn finite(n: f64) -> Value {
    if n.is_finite() {
        Value::Num(n)
    } else {
        Value::Err(Error::Value)
    }
}

fn binary(op: Op, a: &Value, b: &Value) -> Value {
    // An error on either side is the answer, whatever the operator: the first
    // thing that went wrong is the thing worth reporting.
    if let Value::Err(error) = a {
        return Value::Err(*error);
    }
    if let Value::Err(error) = b {
        return Value::Err(*error);
    }

    if matches!(op, Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge) {
        return compare(op, a, b);
    }

    let (a, b) = match (number(a), number(b)) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(error), _) | (_, Err(error)) => return Value::Err(error),
    };
    match op {
        Op::Add => finite(a + b),
        Op::Sub => finite(a - b),
        Op::Mul => finite(a * b),
        Op::Div if b == 0.0 => Value::Err(Error::Div0),
        Op::Div => finite(a / b),
        Op::Pow => finite(a.powf(b)),
        Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge => unreachable!("handled above"),
    }
}

/// Compare two values, giving `1` or `0`.
///
/// Two texts compare as strings and two numbers as numbers. `Empty` takes the
/// shape of whatever it is next to — zero against a number, `""` against a
/// text — so that `=A1=""` and `=A1=0` both answer a question about an empty
/// cell instead of returning `#VALUE!`. A text against a number is the one
/// pair with no sensible answer.
fn compare(op: Op, a: &Value, b: &Value) -> Value {
    let ordering = match (a, b) {
        (Value::Text(a), Value::Text(b)) => a.cmp(b),
        (Value::Text(a), Value::Empty) => a.as_str().cmp(""),
        (Value::Empty, Value::Text(b)) => "".cmp(b.as_str()),
        (Value::Text(_), _) | (_, Value::Text(_)) => return Value::Err(Error::Value),
        _ => match (number(a), number(b)) {
            (Ok(a), Ok(b)) => a.total_cmp(&b),
            (Err(error), _) | (_, Err(error)) => return Value::Err(error),
        },
    };
    let yes = match op {
        Op::Eq => ordering.is_eq(),
        Op::Ne => ordering.is_ne(),
        Op::Lt => ordering.is_lt(),
        Op::Le => ordering.is_le(),
        Op::Gt => ordering.is_gt(),
        Op::Ge => ordering.is_ge(),
        _ => return Value::Err(Error::Value),
    };
    Value::Num(if yes { 1.0 } else { 0.0 })
}

fn call(func: Func, args: &[Expr], cells: &HashMap<CellRef, Value>) -> Value {
    if func == Func::If {
        // The one function whose arity is fixed, and the one whose arguments
        // are not a flat list of numbers.
        let [cond, then, otherwise] = args else {
            return Value::Err(Error::Value);
        };
        let cond = eval(cond, cells);
        return match number(&cond) {
            // Both branches are evaluated. There is nothing to save by not
            // doing it — the dependency graph already contains both, so both
            // were computed before this cell was reached.
            Ok(n) => eval(if n != 0.0 { then } else { otherwise }, cells),
            Err(error) => Value::Err(error),
        };
    }

    if args.is_empty() && func != Func::Count {
        return Value::Err(Error::Value);
    }

    // Flatten the arguments into the numbers they contribute. Empty cells and
    // text are skipped rather than refused, because the whole point of `SUM`
    // over a rectangle is that the rectangle may have headings and gaps in it.
    // An error is not skipped: it is the answer.
    let mut numbers: Vec<f64> = Vec::new();
    for arg in args {
        if let Expr::Range(from, to) = arg {
            for at in Range::new(*from, *to).cells() {
                match cells.get(&at) {
                    Some(Value::Num(n)) => numbers.push(*n),
                    Some(Value::Err(error)) => return Value::Err(*error),
                    _ => {}
                }
            }
            continue;
        }
        match eval(arg, cells) {
            Value::Num(n) => numbers.push(n),
            Value::Err(error) => return Value::Err(error),
            Value::Empty | Value::Text(_) => {}
        }
    }

    match func {
        Func::Sum => finite(numbers.iter().sum()),
        Func::Count => Value::Num(numbers.len() as f64),
        // An average of nothing is a division by zero, and says so.
        Func::Avg if numbers.is_empty() => Value::Err(Error::Div0),
        Func::Avg => finite(numbers.iter().sum::<f64>() / numbers.len() as f64),
        // `MIN` and `MAX` of nothing are zero, as in every spreadsheet: an
        // empty column is not a mistake, it is an empty column.
        Func::Min => Value::Num(numbers.iter().copied().fold(f64::INFINITY, f64::min))
            .zero_if(numbers.is_empty()),
        Func::Max => Value::Num(numbers.iter().copied().fold(f64::NEG_INFINITY, f64::max))
            .zero_if(numbers.is_empty()),
        Func::If => unreachable!("handled above"),
    }
}

impl Value {
    /// `Num(0)` when `yes`, otherwise self. Only `MIN` and `MAX` need it.
    fn zero_if(self, yes: bool) -> Self {
        if yes { Self::Num(0.0) } else { self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(name: &str) -> CellRef {
        CellRef::parse(name).expect("a reference the test wrote")
    }

    /// Build a sheet from `("A1", "12")` pairs, compile it and evaluate it.
    fn run(cells: &[(&str, &str)]) -> (Compiled, Values) {
        let sheet = Sheet::from_cells(cells.iter().map(|&(name, text)| (cell(name), text)));
        let compiled = compile(&sheet);
        let values = evaluate(&sheet, &compiled);
        (compiled, values)
    }

    /// What one cell says, in a sheet described by `cells`.
    fn value_of(cells: &[(&str, &str)], at: &str) -> Value {
        run(cells).1.get(cell(at)).clone()
    }

    /// What a formula with no references evaluates to.
    fn calc(text: &str) -> Value {
        value_of(&[("A1", &format!("={text}"))], "A1")
    }

    fn num(text: &str) -> f64 {
        match calc(text) {
            Value::Num(n) => n,
            other => panic!("{text} is {other:?}, not a number"),
        }
    }

    #[test]
    fn arithmetic_follows_the_precedence_the_parser_gives_it() {
        assert_eq!(num("1+2*3"), 7.0);
        assert_eq!(num("(1+2)*3"), 9.0);
        assert_eq!(num("2^3^2"), 512.0, "right-associative");
        assert_eq!(num("-2^2"), -4.0, "unary minus is looser than ^");
        assert_eq!(num("10-2-3"), 5.0);
        assert_eq!(num("10/2/5"), 1.0);
        assert_eq!(num("2^-2"), 0.25);
    }

    #[test]
    fn comparisons_give_one_or_zero() {
        assert_eq!(num("1<2"), 1.0);
        assert_eq!(num("1>2"), 0.0);
        assert_eq!(num("2>=2"), 1.0);
        assert_eq!(num("2<=1"), 0.0);
        assert_eq!(num("1=1"), 1.0);
        assert_eq!(num("1<>1"), 0.0);
        assert_eq!(num("\"a\"<\"b\""), 1.0, "two texts compare as strings");
        assert_eq!(num("1+1>1"), 1.0, "and bind looser than arithmetic");
        // An empty cell takes the shape of what it is compared with.
        assert_eq!(value_of(&[("B1", "=A1=0")], "B1"), Value::Num(1.0));
        assert_eq!(value_of(&[("B1", "=A1=\"\"")], "B1"), Value::Num(1.0));
        // Text against a number has no answer.
        assert_eq!(calc("\"a\"<1"), Value::Err(Error::Value));
    }

    #[test]
    fn a_literal_cell_is_a_number_or_a_word() {
        let (_, values) = run(&[("A1", "12"), ("A2", "hello"), ("A3", " 3.5 ")]);
        assert_eq!(*values.get(cell("A1")), Value::Num(12.0));
        assert_eq!(*values.get(cell("A2")), Value::Text(String::from("hello")));
        assert_eq!(*values.get(cell("A3")), Value::Num(3.5));
        assert_eq!(*values.get(cell("Z9")), Value::Empty, "nothing was typed");
    }

    /// The rectangle every aggregate test below runs over: two numbers, one
    /// word, one gap, one more number.
    const MIXED: [(&str, &str); 5] = [
        ("A1", "10"),
        ("A2", "hello"),
        // A3 is left out entirely: an empty cell inside a range.
        ("A4", "2"),
        ("A5", "6"),
        ("B1", "9"),
    ];

    fn over_mixed(text: &str) -> Value {
        let mut cells = MIXED.to_vec();
        cells.push(("C1", text));
        value_of(&cells, "C1")
    }

    #[test]
    fn every_function_skips_the_empties_and_the_text_in_a_range() {
        assert_eq!(over_mixed("=SUM(A1:A5)"), Value::Num(18.0));
        assert_eq!(over_mixed("=COUNT(A1:A5)"), Value::Num(3.0));
        assert_eq!(over_mixed("=AVG(A1:A5)"), Value::Num(6.0));
        assert_eq!(over_mixed("=MIN(A1:A5)"), Value::Num(2.0));
        assert_eq!(over_mixed("=MAX(A1:A5)"), Value::Num(10.0));
        // Several arguments, ranges and scalars mixed.
        assert_eq!(over_mixed("=SUM(A1:A5, B1, 1)"), Value::Num(28.0));
        assert_eq!(over_mixed("=COUNT(A1:A5, B1)"), Value::Num(4.0));
        assert_eq!(over_mixed("=MAX(A1:A5, B1)"), Value::Num(10.0));
        // A rectangle, not just a column.
        assert_eq!(over_mixed("=SUM(A1:B5)"), Value::Num(27.0));
    }

    #[test]
    fn a_function_over_nothing_answers_rather_than_failing() {
        assert_eq!(calc("SUM(Z1:Z9)"), Value::Num(0.0));
        assert_eq!(calc("COUNT(Z1:Z9)"), Value::Num(0.0));
        assert_eq!(calc("MIN(Z1:Z9)"), Value::Num(0.0));
        assert_eq!(calc("MAX(Z1:Z9)"), Value::Num(0.0));
        assert_eq!(calc("COUNT()"), Value::Num(0.0));
        // Except an average, which is a division by a count of zero.
        assert_eq!(calc("AVG(Z1:Z9)"), Value::Err(Error::Div0));
        assert_eq!(calc("SUM()"), Value::Err(Error::Value));
    }

    #[test]
    fn if_picks_a_branch_and_wants_exactly_three_arguments() {
        assert_eq!(
            calc("IF(1>0,\"yes\",\"no\")"),
            Value::Text(String::from("yes"))
        );
        assert_eq!(
            calc("IF(1<0,\"yes\",\"no\")"),
            Value::Text(String::from("no"))
        );
        assert_eq!(calc("IF(0,1,2)"), Value::Num(2.0), "zero is false");
        assert_eq!(calc("IF(-1,1,2)"), Value::Num(1.0), "anything else is true");
        assert_eq!(calc("IF(1,2)"), Value::Err(Error::Value));
        assert_eq!(calc("IF(1,2,3,4)"), Value::Err(Error::Value));
        assert_eq!(calc("IF(\"x\",1,2)"), Value::Err(Error::Value));
        // The condition may read the sheet, which is what the preset does.
        assert_eq!(
            value_of(
                &[
                    ("A1", "5"),
                    ("A2", "3"),
                    ("B1", "=IF(A1>A2,\"over\",\"ok\")")
                ],
                "B1"
            ),
            Value::Text(String::from("over"))
        );
    }

    #[test]
    fn every_error_reaches_the_cell_that_asked_for_it() {
        assert_eq!(calc("1+"), Value::Err(Error::Parse));
        assert_eq!(calc("AA1"), Value::Err(Error::Ref));
        assert_eq!(calc("1/0"), Value::Err(Error::Div0));
        assert_eq!(calc("NOPE(1)"), Value::Err(Error::Name));
        assert_eq!(calc("A1:A3"), Value::Err(Error::Cycle), "reads itself");
        assert_eq!(
            value_of(&[("A1", "hi"), ("B1", "=A1+1")], "B1"),
            Value::Err(Error::Value),
            "text in arithmetic"
        );
        assert_eq!(
            value_of(&[("B1", "=A1:A3")], "B1"),
            Value::Err(Error::Value),
            "a bare rectangle is not a value"
        );
        // Overflow is an error rather than an infinity spreading through the
        // sheet.
        assert_eq!(calc("1e300*1e300"), Value::Err(Error::Value));
    }

    #[test]
    fn an_error_travels_to_everything_downstream_of_it() {
        let cells = [
            ("A1", "hi"),
            ("B1", "=A1+1"),
            ("C1", "=B1*2"),
            ("D1", "=SUM(B1:C1)"),
        ];
        assert_eq!(value_of(&cells, "B1"), Value::Err(Error::Value));
        assert_eq!(value_of(&cells, "C1"), Value::Err(Error::Value));
        assert_eq!(value_of(&cells, "D1"), Value::Err(Error::Value));
        assert_eq!(
            value_of(&[("A1", "=1/0"), ("B1", "=A1+1")], "B1"),
            Value::Err(Error::Div0),
            "and it keeps its own name"
        );
    }

    #[test]
    fn a_diamond_evaluates_its_shared_input_once_and_first() {
        let cells = [
            ("A1", "3"),
            ("B1", "=A1*2"),
            ("C1", "=A1+10"),
            ("D1", "=B1+C1"),
        ];
        let (compiled, values) = run(&cells);
        assert_eq!(*values.get(cell("D1")), Value::Num(19.0));

        let place = |name: &str| {
            compiled
                .order
                .iter()
                .position(|at| *at == cell(name))
                .unwrap_or_else(|| panic!("{name} is in the order"))
        };
        assert!(place("B1") < place("D1"));
        assert!(place("C1") < place("D1"));
        assert_eq!(compiled.order.len(), 3, "A1 is a literal, not a formula");
        assert_eq!(compiled.deps[&cell("D1")], vec![cell("B1"), cell("C1")]);
        assert!(compiled.cyclic.is_empty());
    }

    #[test]
    fn a_two_cell_cycle_is_reported_in_both_cells() {
        let cells = [("A1", "=B1"), ("B1", "=A1"), ("C1", "=1+1"), ("D1", "7")];
        let (compiled, values) = run(&cells);
        assert_eq!(*values.get(cell("A1")), Value::Err(Error::Cycle));
        assert_eq!(*values.get(cell("B1")), Value::Err(Error::Cycle));
        assert_eq!(
            compiled.cyclic,
            HashSet::from([cell("A1"), cell("B1")]),
            "and only those two"
        );
        // Unrelated cells are untouched: a cycle is not a broken sheet.
        assert_eq!(*values.get(cell("C1")), Value::Num(2.0));
        assert_eq!(*values.get(cell("D1")), Value::Num(7.0));
        assert_eq!(compiled.order, vec![cell("C1")]);
    }

    #[test]
    fn a_cell_that_reads_itself_is_a_cycle() {
        let (compiled, values) = run(&[("A1", "=A1+1"), ("B1", "=2")]);
        assert_eq!(*values.get(cell("A1")), Value::Err(Error::Cycle));
        assert_eq!(*values.get(cell("B1")), Value::Num(2.0));
        assert_eq!(compiled.cyclic, HashSet::from([cell("A1")]));
        // And so is a cell that reads a range it is inside.
        let (_, values) = run(&[("A2", "=SUM(A1:A3)")]);
        assert_eq!(*values.get(cell("A2")), Value::Err(Error::Cycle));
    }

    #[test]
    fn a_cell_downstream_of_a_cycle_cannot_be_evaluated_either() {
        let cells = [
            ("A1", "=B1"),
            ("B1", "=A1"),
            ("C1", "=A1+1"),
            ("D1", "=C1*2"),
            ("E1", "=1"),
        ];
        let (compiled, values) = run(&cells);
        for name in ["A1", "B1", "C1", "D1"] {
            assert_eq!(*values.get(cell(name)), Value::Err(Error::Cycle), "{name}");
        }
        assert_eq!(*values.get(cell("E1")), Value::Num(1.0));
        assert_eq!(compiled.order, vec![cell("E1")]);
        assert_eq!(compiled.cyclic.len(), 4);
    }

    #[test]
    fn fixing_a_cycle_clears_every_cell_it_touched() {
        let (_, values) = run(&[("A1", "=B1"), ("B1", "=A1"), ("C1", "=A1+1")]);
        assert_eq!(*values.get(cell("C1")), Value::Err(Error::Cycle));
        // The same sheet with `B1` made a literal: nothing is cyclic any more.
        let (compiled, values) = run(&[("A1", "=B1"), ("B1", "4"), ("C1", "=A1+1")]);
        assert!(compiled.cyclic.is_empty());
        assert_eq!(*values.get(cell("A1")), Value::Num(4.0));
        assert_eq!(*values.get(cell("C1")), Value::Num(5.0));
    }

    #[test]
    fn a_chain_is_ordered_however_the_cells_were_written() {
        // Written back to front, so nothing but the ordering can make it work.
        let cells = [
            ("A5", "=A4+1"),
            ("A4", "=A3+1"),
            ("A3", "=A2+1"),
            ("A2", "=A1+1"),
            ("A1", "1"),
        ];
        let (compiled, values) = run(&cells);
        assert_eq!(*values.get(cell("A5")), Value::Num(5.0));
        assert_eq!(
            compiled.order,
            vec![cell("A2"), cell("A3"), cell("A4"), cell("A5")]
        );
    }

    #[test]
    fn compile_keeps_the_parse_and_the_dependencies_of_every_formula() {
        let (compiled, _) = run(&[("A1", "1"), ("B1", "=SUM(A1:A2)+A1"), ("C1", "=nope()")]);
        assert_eq!(compiled.formulas.len(), 2, "literals are not compiled");
        assert_eq!(compiled.deps[&cell("B1")], vec![cell("A1"), cell("A2")]);
        assert_eq!(compiled.formulas[&cell("C1")], Err(Error::Name));
        assert_eq!(compiled.deps[&cell("C1")], Vec::<CellRef>::new());
    }

    #[test]
    fn an_empty_sheet_compiles_and_evaluates_to_nothing() {
        let (compiled, values) = run(&[]);
        assert!(compiled.order.is_empty());
        assert!(compiled.cyclic.is_empty());
        assert!(values.cells.is_empty());
        assert_eq!(*values.get(cell("A1")), Value::Empty);
    }
}
