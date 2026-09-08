//! The formula language: tokens, syntax, values, and the reference arithmetic
//! a paste needs.
//!
//! Syntax only. Nothing here reads a sheet, because a reference cannot be
//! resolved without knowing what every other cell evaluates to, and that
//! question — and the ordering it needs — is `eval.rs`'s. Keeping the two
//! apart is what lets the parse be memoised on `structure_rev` alone while the
//! evaluation reruns on `value_rev`: they are two functions, so they can be
//! two memos.
//!
//! The language is deliberately small (`docs/tasks/spreadsheet/task.md`): the
//! example is about the UI over it, not about being a spreadsheet. It has
//! numbers, quoted strings, references, ranges, the five arithmetic operators,
//! six comparisons, and six functions. What it does *not* have — absolute
//! references, whole columns, named ranges, string functions — is out of
//! scope, not unfinished.

use crate::sheet::{CellRef, is_formula, shifted};

/// What a formula evaluates to, and what a literal cell holds.
///
/// `Empty` is a value rather than an `Option` because "empty" behaves
/// differently in different places — zero in arithmetic, skipped by `SUM`,
/// `""` next to a string — and every one of those rules is a `match` arm in
/// `eval.rs` rather than a `None` check at every call site.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Empty,
    Num(f64),
    Text(String),
    Err(Error),
}

/// The six things that can go wrong, spelled the way a spreadsheet spells
/// them.
///
/// An error is a *value*: it flows through arithmetic and lands in the cell
/// that asked, which is why a broken cell does not break the sheet.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Error {
    /// The text is not a formula.
    Parse,
    /// A reference outside `A1:Z10000`, or one a paste pushed off the edge.
    Ref,
    /// The cell is on, or downstream of, a cycle.
    Cycle,
    Div0,
    /// An unknown function name.
    Name,
    /// The right syntax with the wrong kind of value: text in arithmetic, a
    /// bare range, `IF` with the wrong number of arguments.
    Value,
}

impl Error {
    /// Every error, which is also the list the tokenizer reads back: an error
    /// label written into cell text (a paste puts `#REF!` there) parses as
    /// that error rather than as a syntax mistake.
    pub const ALL: [Self; 6] = [
        Self::Parse,
        Self::Ref,
        Self::Cycle,
        Self::Div0,
        Self::Name,
        Self::Value,
    ];

    /// What the cell shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Parse => "#PARSE!",
            Self::Ref => "#REF!",
            Self::Cycle => "#CYCLE!",
            Self::Div0 => "#DIV/0!",
            Self::Name => "#NAME?",
            Self::Value => "#VALUE!",
        }
    }

    /// The inverse of [`Error::label`].
    pub fn from_label(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|e| e.label() == s)
    }
}

/// A binary operator. The comparisons give `1` or `0`, so that `=IF(A1>3,..)`
/// and `=(A1>3)*2` both work without a boolean type.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// The six functions. A closed set, so an unknown name is a `#NAME?` the
/// moment the formula is parsed rather than a surprise at evaluation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Func {
    Sum,
    Avg,
    Min,
    Max,
    Count,
    If,
}

impl Func {
    /// Read a function name, case-insensitively. `None` is `#NAME?`.
    pub fn parse(name: &str) -> Option<Self> {
        match name.to_ascii_uppercase().as_str() {
            "SUM" => Some(Self::Sum),
            "AVG" => Some(Self::Avg),
            "MIN" => Some(Self::Min),
            "MAX" => Some(Self::Max),
            "COUNT" => Some(Self::Count),
            "IF" => Some(Self::If),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Sum => "SUM",
            Self::Avg => "AVG",
            Self::Min => "MIN",
            Self::Max => "MAX",
            Self::Count => "COUNT",
            Self::If => "IF",
        }
    }
}

/// A parsed formula.
///
/// [`Expr::Range`] is an expression rather than a special case of an argument
/// list, so `=A1:A3` parses and then fails at evaluation with `#VALUE!`. A
/// parse error would be the wrong answer: the text is well-formed, it is the
/// *value* of a rectangle outside a function that does not exist.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Num(f64),
    Str(String),
    Ref(CellRef),
    Range(CellRef, CellRef),
    Neg(Box<Expr>),
    Bin(Op, Box<Expr>, Box<Expr>),
    Call(Func, Vec<Expr>),
}

/// How deep a formula may nest before the parser gives up.
///
/// Cell text arrives from a human, from a paste and from a saved file, and a
/// recursive-descent parser meets `((((((..))))))` with a blown stack. 64 is
/// far past anything a person writes and far short of the stack.
const MAX_DEPTH: u32 = 64;

/// Parse a formula. `text` is the cell's text **without** the leading `=`.
///
/// Never panics and never allocates unboundedly: every path out of the
/// tokenizer and the parser is an [`Error`], because every caller is holding
/// something a user typed.
pub fn parse(text: &str) -> Result<Expr, Error> {
    let toks = tokenize(text)?;
    let mut p = Parser {
        toks: &toks,
        pos: 0,
        depth: 0,
    };
    let expr = p.expr()?;
    if p.pos != p.toks.len() {
        return Err(Error::Parse);
    }
    Ok(expr)
}

/// Every cell the expression reads, with ranges expanded, sorted and
/// deduplicated.
///
/// This is the edge list of the dependency graph, and it is also what the
/// status bar shows under the cursor. Sorted because a stable order makes the
/// graph — and therefore the evaluation order and the tests — deterministic.
pub fn refs(expr: &Expr) -> Vec<CellRef> {
    let mut out = Vec::new();
    collect_refs(expr, &mut out);
    out.sort_unstable();
    out.dedup();
    out
}

fn collect_refs(expr: &Expr, out: &mut Vec<CellRef>) {
    match expr {
        Expr::Num(_) | Expr::Str(_) => {}
        Expr::Ref(at) => out.push(*at),
        Expr::Range(from, to) => {
            let range = crate::sheet::Range::new(*from, *to);
            out.extend(range.cells());
        }
        Expr::Neg(inner) => collect_refs(inner, out),
        Expr::Bin(_, a, b) => {
            collect_refs(a, out);
            collect_refs(b, out);
        }
        Expr::Call(_, args) => {
            for arg in args {
                collect_refs(arg, out);
            }
        }
    }
}

/// The same cell text with every reference moved by `(dcol, drow)`.
///
/// This is what copy and paste do, and it works on the *text*, not on an
/// [`Expr`]: only the reference tokens are rewritten and everything between
/// them is copied across, so `"= A1 + 1"` pastes as `"= B1 + 1"` with its
/// spacing intact. Text that is not a formula is returned unchanged, and text
/// the tokenizer cannot read is returned unchanged too — a paste is not the
/// moment to lose what the user wrote.
///
/// A reference that would leave the sheet becomes `#REF!` in the text, the
/// same way a real spreadsheet writes the error into the formula rather than
/// refusing the paste.
pub fn shift(text: &str, dcol: i32, drow: i32) -> String {
    if !is_formula(text) {
        return text.to_owned();
    }
    let body = &text[1..];
    let Ok(toks) = tokenize(body) else {
        return text.to_owned();
    };

    let mut out = String::with_capacity(text.len() + 8);
    out.push('=');
    let mut last = 0;
    for t in &toks {
        let Tok::Cell(at) = t.tok else { continue };
        out.push_str(&body[last..t.start]);
        match shifted(at, dcol, drow) {
            Some(moved) => out.push_str(&moved.name()),
            None => out.push_str(Error::Ref.label()),
        }
        last = t.end;
    }
    out.push_str(&body[last..]);
    out
}

/// What a cell that is *not* a formula holds.
///
/// The rule is the one every spreadsheet uses and the one the UI's
/// right-alignment depends on: text that reads as a finite number is a number,
/// blank is empty, everything else is text. Non-finite is deliberately text —
/// a cell saying `inf` is a cell saying a word.
pub fn literal(text: &str) -> Value {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Value::Empty;
    }
    match trimmed.parse::<f64>() {
        Ok(n) if n.is_finite() => Value::Num(n),
        _ => Value::Text(text.to_owned()),
    }
}

// ---------------------------------------------------------------------------
// Tokenizer
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    /// A bare word of letters: a function name, or nothing valid.
    Word(String),
    Cell(CellRef),
    /// An error label written into the text, such as the `#REF!` a paste left
    /// behind.
    Err(Error),
    Op(Op),
    LParen,
    RParen,
    Comma,
    Colon,
}

/// A token and where it was, so that [`shift`] can splice the original text
/// back together around the references it rewrites.
#[derive(Clone, Debug)]
struct Spanned {
    tok: Tok,
    start: usize,
    end: usize,
}

fn tokenize(text: &str) -> Result<Vec<Spanned>, Error> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;

    while i < bytes.len() {
        let start = i;
        let b = bytes[i];
        let tok = match b {
            b' ' | b'\t' | b'\n' | b'\r' => {
                i += 1;
                continue;
            }
            b'(' => {
                i += 1;
                Tok::LParen
            }
            b')' => {
                i += 1;
                Tok::RParen
            }
            b',' => {
                i += 1;
                Tok::Comma
            }
            b':' => {
                i += 1;
                Tok::Colon
            }
            b'+' => {
                i += 1;
                Tok::Op(Op::Add)
            }
            b'-' => {
                i += 1;
                Tok::Op(Op::Sub)
            }
            b'*' => {
                i += 1;
                Tok::Op(Op::Mul)
            }
            b'/' => {
                i += 1;
                Tok::Op(Op::Div)
            }
            b'^' => {
                i += 1;
                Tok::Op(Op::Pow)
            }
            b'=' => {
                i += 1;
                Tok::Op(Op::Eq)
            }
            b'<' => {
                i += 1;
                match bytes.get(i) {
                    Some(b'>') => {
                        i += 1;
                        Tok::Op(Op::Ne)
                    }
                    Some(b'=') => {
                        i += 1;
                        Tok::Op(Op::Le)
                    }
                    _ => Tok::Op(Op::Lt),
                }
            }
            b'>' => {
                i += 1;
                match bytes.get(i) {
                    Some(b'=') => {
                        i += 1;
                        Tok::Op(Op::Ge)
                    }
                    _ => Tok::Op(Op::Gt),
                }
            }
            b'"' => {
                // No escapes: a quote ends the string. `""` inside one is out
                // of scope with the rest of the string handling.
                i += 1;
                let from = i;
                while i < bytes.len() && bytes[i] != b'"' {
                    i += 1;
                }
                if i >= bytes.len() {
                    return Err(Error::Parse);
                }
                let s = text[from..i].to_owned();
                i += 1;
                Tok::Str(s)
            }
            b'#' => {
                // An error label the text already carries. `#DIV/0!` has a
                // slash in it and `#NAME?` ends in a question mark, so the
                // scan takes both and stops at either terminator.
                i += 1;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'/') {
                    i += 1;
                }
                if i >= bytes.len() || !(bytes[i] == b'!' || bytes[i] == b'?') {
                    return Err(Error::Parse);
                }
                i += 1;
                Tok::Err(Error::from_label(&text[start..i]).ok_or(Error::Parse)?)
            }
            b'0'..=b'9' | b'.' => {
                while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                    i += 1;
                }
                // An exponent, but only when it really is one: `1e5` is a
                // number and `1` next to `E5` is a number next to a reference,
                // which is a syntax error either way, so the shape decides.
                if let Some(b'e' | b'E') = bytes.get(i) {
                    let mut j = i + 1;
                    if let Some(b'+' | b'-') = bytes.get(j) {
                        j += 1;
                    }
                    if bytes.get(j).is_some_and(u8::is_ascii_digit) {
                        i = j;
                        while i < bytes.len() && bytes[i].is_ascii_digit() {
                            i += 1;
                        }
                    }
                }
                Tok::Num(text[start..i].parse().map_err(|_| Error::Parse)?)
            }
            b if b.is_ascii_alphabetic() => {
                while i < bytes.len() && bytes[i].is_ascii_alphanumeric() {
                    i += 1;
                }
                word(&text[start..i])?
            }
            _ => return Err(Error::Parse),
        };
        out.push(Spanned { tok, start, end: i });
    }

    Ok(out)
}

/// Classify a run of letters and digits.
///
/// Letters then digits is meant as a reference, so `AA1` and `A0` are `#REF!`
/// and not `#NAME?`: the user wrote a cell, it is just not one of ours.
fn word(w: &str) -> Result<Tok, Error> {
    let letters = w.bytes().take_while(u8::is_ascii_alphabetic).count();
    let digits = w.len() - letters;
    if digits == 0 {
        return Ok(Tok::Word(w.to_owned()));
    }
    if w[letters..].bytes().all(|b| b.is_ascii_digit()) {
        return CellRef::parse(w).map(Tok::Cell).ok_or(Error::Ref);
    }
    Err(Error::Parse)
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

/// Recursive descent, one level per precedence, loosest first:
/// comparison < `+ -` < `* /` < unary `-` < `^` < atom.
///
/// Two consequences worth naming, because both surprise people and both are
/// pinned by tests: `^` binds tighter than unary minus, so `-2^2` is `-4`, and
/// `^` is right-associative, so `2^3^2` is `512`.
struct Parser<'a> {
    toks: &'a [Spanned],
    pos: usize,
    depth: u32,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|t| &t.tok)
    }

    fn eat(&mut self, tok: &Tok) -> bool {
        if self.peek() == Some(tok) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Run `f` one level deeper, refusing text that nests past [`MAX_DEPTH`].
    fn nested<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T, Error>) -> Result<T, Error> {
        if self.depth >= MAX_DEPTH {
            return Err(Error::Parse);
        }
        self.depth += 1;
        let out = f(self);
        self.depth -= 1;
        out
    }

    fn expr(&mut self) -> Result<Expr, Error> {
        self.nested(Self::comparison)
    }

    fn comparison(&mut self) -> Result<Expr, Error> {
        let mut lhs = self.sum()?;
        while let Some(&Tok::Op(op)) = self.peek() {
            if !matches!(op, Op::Eq | Op::Ne | Op::Lt | Op::Le | Op::Gt | Op::Ge) {
                break;
            }
            self.pos += 1;
            let rhs = self.sum()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn sum(&mut self) -> Result<Expr, Error> {
        let mut lhs = self.product()?;
        while let Some(&Tok::Op(op @ (Op::Add | Op::Sub))) = self.peek() {
            self.pos += 1;
            let rhs = self.product()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn product(&mut self) -> Result<Expr, Error> {
        let mut lhs = self.unary()?;
        while let Some(&Tok::Op(op @ (Op::Mul | Op::Div))) = self.peek() {
            self.pos += 1;
            let rhs = self.unary()?;
            lhs = Expr::Bin(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn unary(&mut self) -> Result<Expr, Error> {
        if self.eat(&Tok::Op(Op::Sub)) {
            // Looser than `^` on purpose, so `-2^2` is `-(2^2)`.
            return self.nested(|p| Ok(Expr::Neg(Box::new(p.unary()?))));
        }
        // A leading `+` is allowed and means nothing, as everywhere else.
        self.eat(&Tok::Op(Op::Add));
        self.power()
    }

    fn power(&mut self) -> Result<Expr, Error> {
        let base = self.atom()?;
        if self.eat(&Tok::Op(Op::Pow)) {
            // The right-hand side goes back through `unary`, which makes `^`
            // right-associative and lets `2^-1` parse.
            let exp = self.nested(Self::unary)?;
            return Ok(Expr::Bin(Op::Pow, Box::new(base), Box::new(exp)));
        }
        Ok(base)
    }

    fn atom(&mut self) -> Result<Expr, Error> {
        let Some(tok) = self.peek().cloned() else {
            return Err(Error::Parse);
        };
        self.pos += 1;
        match tok {
            Tok::Num(n) => Ok(Expr::Num(n)),
            Tok::Str(s) => Ok(Expr::Str(s)),
            Tok::Err(e) => Err(e),
            Tok::Cell(at) => {
                if self.eat(&Tok::Colon) {
                    let Some(Tok::Cell(to)) = self.peek().cloned() else {
                        return Err(Error::Parse);
                    };
                    self.pos += 1;
                    let range = crate::sheet::Range::new(at, to);
                    return Ok(Expr::Range(range.from, range.to));
                }
                Ok(Expr::Ref(at))
            }
            Tok::LParen => {
                let inner = self.expr()?;
                if !self.eat(&Tok::RParen) {
                    return Err(Error::Parse);
                }
                Ok(inner)
            }
            Tok::Word(name) => {
                // A word is only ever a function: there are no constants and
                // no named ranges, so `=FOO` without a call is `#NAME?` too.
                let func = Func::parse(&name).ok_or(Error::Name)?;
                if !self.eat(&Tok::LParen) {
                    return Err(Error::Name);
                }
                let mut args = Vec::new();
                if !self.eat(&Tok::RParen) {
                    loop {
                        args.push(self.expr()?);
                        if self.eat(&Tok::Comma) {
                            continue;
                        }
                        if self.eat(&Tok::RParen) {
                            break;
                        }
                        return Err(Error::Parse);
                    }
                }
                Ok(Expr::Call(func, args))
            }
            Tok::Op(_) | Tok::RParen | Tok::Comma | Tok::Colon => Err(Error::Parse),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheet::CellRef;

    fn cell(name: &str) -> CellRef {
        CellRef::parse(name).expect("a reference the test wrote")
    }

    fn num(n: f64) -> Box<Expr> {
        Box::new(Expr::Num(n))
    }

    fn bin(op: Op, a: Box<Expr>, b: Box<Expr>) -> Box<Expr> {
        Box::new(Expr::Bin(op, a, b))
    }

    #[test]
    fn multiplication_binds_tighter_than_addition() {
        assert_eq!(
            parse("1+2*3"),
            Ok(*bin(Op::Add, num(1.0), bin(Op::Mul, num(2.0), num(3.0))))
        );
        assert_eq!(
            parse("(1+2)*3"),
            Ok(*bin(Op::Mul, bin(Op::Add, num(1.0), num(2.0)), num(3.0)))
        );
        // Same precedence goes left to right.
        assert_eq!(
            parse("1-2-3"),
            Ok(*bin(Op::Sub, bin(Op::Sub, num(1.0), num(2.0)), num(3.0)))
        );
    }

    #[test]
    fn comparison_is_the_loosest_operator() {
        assert_eq!(
            parse("1+1>A1*2"),
            Ok(*bin(
                Op::Gt,
                bin(Op::Add, num(1.0), num(1.0)),
                bin(Op::Mul, Box::new(Expr::Ref(cell("A1"))), num(2.0)),
            ))
        );
        for (text, op) in [
            ("1=2", Op::Eq),
            ("1<>2", Op::Ne),
            ("1<2", Op::Lt),
            ("1<=2", Op::Le),
            ("1>2", Op::Gt),
            ("1>=2", Op::Ge),
        ] {
            assert_eq!(parse(text), Ok(*bin(op, num(1.0), num(2.0))), "{text}");
        }
    }

    #[test]
    fn the_power_operator_is_right_associative() {
        // 2^(3^2) = 512, not (2^3)^2 = 64.
        assert_eq!(
            parse("2^3^2"),
            Ok(*bin(Op::Pow, num(2.0), bin(Op::Pow, num(3.0), num(2.0))))
        );
    }

    #[test]
    fn unary_minus_is_looser_than_the_power_operator() {
        // -(2^2), so -4 and not 4.
        assert_eq!(
            parse("-2^2"),
            Ok(Expr::Neg(bin(Op::Pow, num(2.0), num(2.0))))
        );
        assert_eq!(
            parse("-2*3"),
            Ok(*bin(Op::Mul, Box::new(Expr::Neg(num(2.0))), num(3.0)))
        );
        assert_eq!(
            parse("2^-1"),
            Ok(*bin(Op::Pow, num(2.0), Box::new(Expr::Neg(num(1.0)))))
        );
        assert_eq!(parse("--1"), Ok(Expr::Neg(Box::new(Expr::Neg(num(1.0))))));
        assert_eq!(parse("+1"), Ok(Expr::Num(1.0)));
    }

    #[test]
    fn numbers_strings_references_and_ranges_are_atoms() {
        assert_eq!(parse("3.5"), Ok(Expr::Num(3.5)));
        assert_eq!(parse(".5"), Ok(Expr::Num(0.5)));
        assert_eq!(parse("1e3"), Ok(Expr::Num(1000.0)));
        assert_eq!(parse("2E-2"), Ok(Expr::Num(0.02)));
        assert_eq!(
            parse("\"hi there\""),
            Ok(Expr::Str(String::from("hi there")))
        );
        assert_eq!(parse("\"\""), Ok(Expr::Str(String::new())));
        assert_eq!(parse("b2"), Ok(Expr::Ref(cell("B2"))), "case-insensitive");
        // A range is normalised the moment it is parsed.
        assert_eq!(parse("C3:A1"), Ok(Expr::Range(cell("A1"), cell("C3"))));
    }

    #[test]
    fn a_call_takes_any_number_of_arguments() {
        assert_eq!(
            parse("SUM(A1:A3, 4, B1*2)"),
            Ok(Expr::Call(
                Func::Sum,
                vec![
                    Expr::Range(cell("A1"), cell("A3")),
                    Expr::Num(4.0),
                    *bin(Op::Mul, Box::new(Expr::Ref(cell("B1"))), num(2.0)),
                ]
            ))
        );
        assert_eq!(parse("count()"), Ok(Expr::Call(Func::Count, vec![])));
        for func in [
            Func::Sum,
            Func::Avg,
            Func::Min,
            Func::Max,
            Func::Count,
            Func::If,
        ] {
            let text = format!("{}(1)", func.name().to_ascii_lowercase());
            assert_eq!(
                parse(&text),
                Ok(Expr::Call(func, vec![Expr::Num(1.0)])),
                "{text}"
            );
        }
    }

    #[test]
    fn syntax_that_does_not_read_is_a_parse_error() {
        for text in [
            "",
            "1+",
            "*2",
            "(1",
            "1)",
            "1 2",
            "SUM(1,)",
            "\"unterminated",
            "1 & 2",
            "A1:",
            "SUM(1;2)",
            "#WHAT!",
            "1A2",
        ] {
            assert_eq!(parse(text), Err(Error::Parse), "{text:?}");
        }
    }

    #[test]
    fn an_unknown_function_is_a_name_error_at_parse_time() {
        assert_eq!(parse("VLOOKUP(1)"), Err(Error::Name));
        // A bare word is a call without its parentheses, not a constant.
        assert_eq!(parse("TRUE"), Err(Error::Name));
        assert_eq!(parse("SUM"), Err(Error::Name));
    }

    #[test]
    fn a_reference_outside_the_sheet_is_a_ref_error_at_parse_time() {
        assert_eq!(parse("AA1"), Err(Error::Ref));
        assert_eq!(parse("A0"), Err(Error::Ref));
        assert_eq!(parse("A10001+1"), Err(Error::Ref));
    }

    #[test]
    fn an_error_label_in_the_text_parses_as_that_error() {
        // This is what a paste leaves behind, and reading it back is what
        // makes the cell keep saying `#REF!` instead of `#PARSE!`.
        for error in Error::ALL {
            assert_eq!(parse(error.label()), Err(error), "{}", error.label());
            assert_eq!(parse(&format!("1+{}", error.label())), Err(error));
        }
    }

    #[test]
    fn a_formula_that_nests_too_deep_is_refused_rather_than_crashing() {
        let deep = format!("{}1{}", "(".repeat(200), ")".repeat(200));
        assert_eq!(parse(&deep), Err(Error::Parse));
        // And a shallow one still reads.
        let ok = format!("{}1{}", "(".repeat(8), ")".repeat(8));
        assert_eq!(parse(&ok), Ok(Expr::Num(1.0)));
    }

    #[test]
    fn refs_expands_ranges_and_sorts() {
        let expr = parse("SUM(A1:B2)+B2+A1").expect("parses");
        assert_eq!(
            refs(&expr),
            vec![cell("A1"), cell("B1"), cell("A2"), cell("B2")],
            "reading order, deduplicated"
        );
        assert_eq!(refs(&parse("1+2").expect("parses")), vec![]);
    }

    #[test]
    fn shift_moves_every_reference_and_keeps_the_spacing() {
        assert_eq!(shift("= A1 + 1", 1, 0), "= B1 + 1");
        assert_eq!(shift("=A1+B2", 0, 1), "=A2+B3");
        assert_eq!(shift("=B2", -1, -1), "=A1");
        assert_eq!(shift("=SUM(A1:A3)", 2, 5), "=SUM(C6:C8)");
        assert_eq!(shift("=A1", 0, 0), "=A1");
        // Strings are not references, however much they look like one.
        assert_eq!(shift("=\"A1\"+A1", 1, 0), "=\"A1\"+B1");
    }

    #[test]
    fn shift_off_the_edge_writes_the_error_into_the_text() {
        assert_eq!(shift("=A1+B1", -1, 0), "=#REF!+A1");
        assert_eq!(shift("=A1", 0, -1), "=#REF!");
        assert_eq!(shift("=Z1", 1, 0), "=#REF!");
        assert_eq!(shift("=A10000", 0, 1), "=#REF!");
        // And the result is still readable text, which is what keeps the cell
        // saying `#REF!` rather than `#PARSE!`.
        assert_eq!(parse(&shift("=A1", 0, -1)[1..]), Err(Error::Ref));
    }

    #[test]
    fn shift_leaves_alone_what_it_cannot_read() {
        assert_eq!(shift("12", 1, 1), "12");
        assert_eq!(shift("hello A1", 1, 1), "hello A1");
        assert_eq!(shift("", 1, 1), "");
        // Text that is a formula but not a readable one comes back whole.
        assert_eq!(shift("=1 & 2", 1, 0), "=1 & 2");
    }

    #[test]
    fn a_literal_is_a_number_when_it_reads_as_one() {
        assert_eq!(literal("12"), Value::Num(12.0));
        assert_eq!(literal(" -3.5 "), Value::Num(-3.5));
        assert_eq!(literal("1e3"), Value::Num(1000.0));
        assert_eq!(literal(""), Value::Empty);
        assert_eq!(literal("   "), Value::Empty);
        assert_eq!(literal("hello"), Value::Text(String::from("hello")));
        assert_eq!(literal("12 apples"), Value::Text(String::from("12 apples")));
        // Not a number the grid can add up, so it is a word.
        assert_eq!(literal("inf"), Value::Text(String::from("inf")));
        assert_eq!(literal("NaN"), Value::Text(String::from("NaN")));
    }

    #[test]
    fn every_error_has_a_label_that_reads_back() {
        for error in Error::ALL {
            assert_eq!(Error::from_label(error.label()), Some(error));
        }
        assert_eq!(Error::from_label("#REF"), None);
        assert_eq!(Error::from_label(""), None);
    }
}
