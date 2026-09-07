//! Rust syntax highlighting for the code column, in Catppuccin colours.
//!
//! `egui_extras`' built-in highlighter is for "C, C++, Rust and Python,
//! okish": it does not know `_` belongs to an identifier, so `use_state` came
//! out as three tokens in three colours. This one is Rust only, one pass over
//! the bytes, and it colours by the Catppuccin style guide: Mocha on a dark
//! theme, Latte on a light one.

use egui::text::{ByteIndex, LayoutJob, LayoutSection, TextFormat};
use egui::{Color32, FontId};

/// What a token is, and so what colour it gets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Text,
    Comment,
    Keyword,
    Str,
    Number,
    Function,
    Type,
    Constant,
    Macro,
    Attribute,
    Lifetime,
    Operator,
    Delimiter,
}

/// The Catppuccin colours a token kind maps to, Mocha and Latte.
///
/// From the style guide's language defaults: keywords Mauve, strings Green,
/// numbers and constants Peach, functions Blue, types and attributes Yellow,
/// macros Rosewater, comments and delimiters Overlay 2, operators Sky,
/// symbols (here: lifetimes) Red.
fn color(kind: Kind, dark: bool) -> Color32 {
    let (mocha, latte) = match kind {
        Kind::Text => (0xcdd6f4, 0x4c4f69),
        Kind::Comment | Kind::Delimiter => (0x9399b2, 0x7c7f93),
        Kind::Keyword => (0xcba6f7, 0x8839ef),
        Kind::Str => (0xa6e3a1, 0x40a02b),
        Kind::Number | Kind::Constant => (0xfab387, 0xfe640b),
        Kind::Function => (0x89b4fa, 0x1e66f5),
        Kind::Type | Kind::Attribute => (0xf9e2af, 0xdf8e1d),
        Kind::Macro => (0xf5e0dc, 0xdc8a78),
        Kind::Lifetime => (0xf38ba8, 0xd20f39),
        Kind::Operator => (0x89dceb, 0x04a5e5),
    };
    let rgb = if dark { mocha } else { latte };
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

const KEYWORDS: &[&str] = &[
    "as",
    "async",
    "await",
    "break",
    "const",
    "continue",
    "crate",
    "dyn",
    "else",
    "enum",
    "extern",
    "fn",
    "for",
    "if",
    "impl",
    "in",
    "let",
    "loop",
    "macro_rules",
    "match",
    "mod",
    "move",
    "mut",
    "pub",
    "ref",
    "return",
    "self",
    "Self",
    "static",
    "struct",
    "super",
    "trait",
    "type",
    "unsafe",
    "use",
    "where",
    "while",
];

/// The whole of `source` as one `LayoutJob`, every byte in a section.
///
/// Every byte, because epaint lays out only what a section covers: a gap
/// would be a missing character, not a plain one.
pub fn highlight(source: &str, dark: bool, font_id: FontId) -> LayoutJob {
    let mut job = LayoutJob {
        text: source.to_owned(),
        ..Default::default()
    };
    let mut last: Option<(Kind, usize)> = None;
    for (start, end, kind) in tokens(source) {
        // Adjacent tokens of one kind share a section: fewer sections, same
        // picture.
        if let Some((k, at)) = last
            && k == kind
        {
            let index = job.sections.len() - 1;
            job.sections[index].byte_range.end = ByteIndex(end);
            let _ = at;
            continue;
        }
        job.sections.push(LayoutSection {
            leading_space: 0.0,
            byte_range: ByteIndex(start)..ByteIndex(end),
            format: TextFormat {
                font_id: font_id.clone(),
                color: color(kind, dark),
                ..Default::default()
            },
        });
        last = Some((kind, start));
    }
    job
}

/// `(start, end, kind)` for each token of `source`, in order, covering it.
fn tokens(source: &str) -> Vec<(usize, usize, Kind)> {
    let bytes = source.as_bytes();
    let n = bytes.len();
    let mut out = Vec::new();
    let mut i = 0;
    let at = |i: usize| bytes.get(i).copied().unwrap_or(0);
    let is_ident_start = |c: u8| c.is_ascii_alphabetic() || c == b'_';
    let is_ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';

    while i < n {
        let start = i;
        let c = at(i);
        let kind = if c == b'/' && at(i + 1) == b'/' {
            while i < n && at(i) != b'\n' {
                i += 1;
            }
            Kind::Comment
        } else if c == b'/' && at(i + 1) == b'*' {
            // Block comments nest in Rust.
            let mut depth = 0;
            loop {
                if i >= n {
                    break;
                }
                if at(i) == b'/' && at(i + 1) == b'*' {
                    depth += 1;
                    i += 2;
                } else if at(i) == b'*' && at(i + 1) == b'/' {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            Kind::Comment
        } else if c == b'"' || (c == b'b' && at(i + 1) == b'"') {
            i += if c == b'b' { 2 } else { 1 };
            while i < n && at(i) != b'"' {
                i += if at(i) == b'\\' { 2 } else { 1 };
            }
            i = (i + 1).min(n);
            Kind::Str
        } else if (c == b'r' && (at(i + 1) == b'"' || at(i + 1) == b'#'))
            || (c == b'b' && at(i + 1) == b'r' && (at(i + 2) == b'"' || at(i + 2) == b'#'))
        {
            // r"…", r#"…"#, br"…": the closing quote carries the same
            // number of hashes as the opening one.
            i += if c == b'b' { 2 } else { 1 };
            let mut hashes = 0;
            while at(i) == b'#' {
                hashes += 1;
                i += 1;
            }
            if at(i) == b'"' {
                i += 1;
                loop {
                    if i >= n {
                        break;
                    }
                    if at(i) == b'"' && (1..=hashes).all(|k| at(i + k) == b'#') {
                        i += 1 + hashes;
                        break;
                    }
                    i += 1;
                }
                Kind::Str
            } else {
                // `r#ident`: a raw identifier, not a string.
                while i < n && is_ident(at(i)) {
                    i += 1;
                }
                Kind::Text
            }
        } else if c == b'\'' {
            // A char literal ('x', '\n', '\u{1F600}') or a lifetime ('a).
            let escaped = at(i + 1) == b'\\';
            let close = if escaped {
                source[i + 2..].find('\'').map(|k| i + 2 + k)
            } else if at(i + 2) == b'\''
                || (at(i + 1) >= 0x80 && source[i + 1..].chars().nth(1) == Some('\''))
            {
                let ch = source[i + 1..].chars().next().map_or(1, char::len_utf8);
                Some(i + 1 + ch)
            } else {
                None
            };
            match close {
                Some(close) if close - i <= 12 => {
                    i = close + 1;
                    Kind::Str
                }
                _ => {
                    i += 1;
                    while i < n && is_ident(at(i)) {
                        i += 1;
                    }
                    Kind::Lifetime
                }
            }
        } else if c == b'#' && (at(i + 1) == b'[' || (at(i + 1) == b'!' && at(i + 2) == b'[')) {
            // `#[…]` and `#![…]`, to the matching bracket.
            let mut depth = 0;
            while i < n {
                match at(i) {
                    b'[' => depth += 1,
                    b']' => {
                        depth -= 1;
                        if depth == 0 {
                            i += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            Kind::Attribute
        } else if c.is_ascii_digit() {
            while i < n && (at(i).is_ascii_digit() || at(i) == b'_') {
                i += 1;
            }
            if at(i) == b'.' && at(i + 1).is_ascii_digit() {
                i += 1;
                while i < n && (at(i).is_ascii_digit() || at(i) == b'_') {
                    i += 1;
                }
            }
            // The suffix or the rest of a hex literal: `1.0f32`, `0xff`, `1e9`.
            while i < n && is_ident(at(i)) {
                i += 1;
            }
            Kind::Number
        } else if is_ident_start(c) {
            while i < n && is_ident(at(i)) {
                i += 1;
            }
            let word = &source[start..i];
            if KEYWORDS.contains(&word) {
                Kind::Keyword
            } else if word == "true" || word == "false" {
                Kind::Constant
            } else if at(i) == b'!' && at(i + 1) != b'=' {
                Kind::Macro
            } else if word.len() > 1
                && word
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
            {
                Kind::Constant
            } else if word.as_bytes()[0].is_ascii_uppercase() {
                Kind::Type
            } else if at(i) == b'(' {
                Kind::Function
            } else {
                Kind::Text
            }
        } else if c.is_ascii_whitespace() {
            while i < n && at(i).is_ascii_whitespace() {
                i += 1;
            }
            Kind::Text
        } else if b"()[]{},;.:".contains(&c) {
            i += 1;
            Kind::Delimiter
        } else if c.is_ascii_punctuation() {
            i += 1;
            Kind::Operator
        } else {
            // A non-ASCII character outside a string or comment: one char,
            // plain.
            i += source[i..].chars().next().map_or(1, char::len_utf8);
            Kind::Text
        };
        out.push((start, i.max(start + 1).min(n), kind));
        i = i.max(start + 1);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(source: &str) -> Vec<(&str, Kind)> {
        tokens(source)
            .into_iter()
            .map(|(s, e, k)| (&source[s..e], k))
            .filter(|(t, _)| !t.trim().is_empty())
            .collect()
    }

    #[test]
    fn an_underscore_is_part_of_the_identifier() {
        let t = kinds("let mut plain = use_state(cx, || false);");
        assert!(t.contains(&("use_state", Kind::Function)), "{t:?}");
        assert!(t.contains(&("false", Kind::Constant)), "{t:?}");
        assert!(t.contains(&("let", Kind::Keyword)), "{t:?}");
    }

    #[test]
    fn strings_chars_lifetimes_and_comments() {
        let t = kinds(
            r##"fn f<'a>(s: &'a str) -> char { let c = '\n'; let r = r#"x"#; /* a /* b */ c */ 'x' } // end"##,
        );
        assert!(t.contains(&("'a", Kind::Lifetime)), "{t:?}");
        assert!(t.contains(&("'\\n'", Kind::Str)), "{t:?}");
        assert!(t.contains(&("r#\"x\"#", Kind::Str)), "{t:?}");
        assert!(t.contains(&("/* a /* b */ c */", Kind::Comment)), "{t:?}");
        assert!(t.contains(&("'x'", Kind::Str)), "{t:?}");
        assert!(t.contains(&("// end", Kind::Comment)), "{t:?}");
    }

    #[test]
    fn macros_types_constants_attributes() {
        let t = kinds("#[component]\npub fn App(cx: &mut Cx) { rsx! { <View w={ROW_H}/> } }");
        assert!(t.contains(&("#[component]", Kind::Attribute)), "{t:?}");
        assert!(t.contains(&("App", Kind::Type)), "{t:?}");
        assert!(t.contains(&("Cx", Kind::Type)), "{t:?}");
        assert!(t.contains(&("rsx", Kind::Macro)), "{t:?}");
        assert!(t.contains(&("ROW_H", Kind::Constant)), "{t:?}");
    }

    #[test]
    fn every_byte_is_covered_once() {
        let source = "let x = \"a\\\"b\"; // c\n'a' 'b' 1.5e3 0xff r\"s\" é";
        let mut at = 0;
        for (s, e, _) in tokens(source) {
            assert_eq!(s, at, "gap or overlap at {s} in {source:?}");
            assert!(e > s);
            at = e;
        }
        assert_eq!(at, source.len());
    }
}
