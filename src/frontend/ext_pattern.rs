//! regex-engine/src/frontend/ext_pattern.rs
//!
//! Surface-syntax pattern AST. Parsed from a pattern string, then lowered
//! to `Regex` by `translate`.

#[derive(Debug, Clone, PartialEq)]
pub enum ExtPat {
    /// Missing branch, e.g. the empty side of `a|` or `(|b)`.
    Empty,
    /// `ε`.
    Eps,
    /// `∅`.
    Never,
    /// `( ... )`.
    Group(Box<ExtPat>),
    /// `(?: ... )`.
    GroupNonMarking(Box<ExtPat>),
    /// `a|b|c`.
    Or(Vec<ExtPat>),
    /// `abc`.
    Concat(Vec<ExtPat>),
    /// `r?` (including `r??`; laziness is discarded).
    Opt(Box<ExtPat>),
    /// `r+` (including `r+?`).
    Plus(Box<ExtPat>),
    /// `r*` (including `r*?`).
    Star(Box<ExtPat>),
    /// `r{lo,hi}`, `r{lo,}`, `r{lo}` (including a trailing `?`).
    Bound(Box<ExtPat>, u32, Option<u32>),
    /// `^`.
    Carat,
    /// `$`.
    Dollar,
    /// `.`.
    Dot,
    /// `[ ... ]`, with shorthands and ranges already expanded.
    Any(Vec<char>),
    /// `[^ ... ]`.
    NoneOf(Vec<char>),
    /// `\c` for a shorthand (`d`/`D`/`w`/`W`/`s`/`S`) or an escaped literal.
    Escape(char),
    /// An ordinary literal character.
    Char(char),
    /// `\b` (`true`) / `\B` (`false`). `translate` rejects these.
    WordBoundary(bool),
}

/// Case-fold an `ExtPat`: every literal letter becomes an `Any` of both cases.
pub fn case_fold(ep: &ExtPat) -> ExtPat {
    match ep {
        ExtPat::Empty => ExtPat::Empty,
        ExtPat::Eps => ExtPat::Eps,
        ExtPat::Never => ExtPat::Never,
        ExtPat::Group(inner) => ExtPat::Group(Box::new(case_fold(inner))),
        ExtPat::GroupNonMarking(inner) => ExtPat::GroupNonMarking(Box::new(case_fold(inner))),
        ExtPat::Or(alts) => ExtPat::Or(alts.iter().map(case_fold).collect()),
        ExtPat::Concat(parts) => ExtPat::Concat(parts.iter().map(case_fold).collect()),
        ExtPat::Opt(inner) => ExtPat::Opt(Box::new(case_fold(inner))),
        ExtPat::Plus(inner) => ExtPat::Plus(Box::new(case_fold(inner))),
        ExtPat::Star(inner) => ExtPat::Star(Box::new(case_fold(inner))),
        ExtPat::Bound(inner, lo, hi) => ExtPat::Bound(Box::new(case_fold(inner)), *lo, *hi),
        ExtPat::Carat => ExtPat::Carat,
        ExtPat::Dollar => ExtPat::Dollar,
        ExtPat::Dot => ExtPat::Dot,
        ExtPat::Any(cs) => ExtPat::Any(both_cases(cs)),
        ExtPat::NoneOf(cs) => ExtPat::NoneOf(both_cases(cs)),
        ExtPat::Escape(c) => ExtPat::Escape(*c),
        ExtPat::WordBoundary(b) => ExtPat::WordBoundary(*b),
        ExtPat::Char(c) => {
            let (lo, up) = (c.to_ascii_lowercase(), c.to_ascii_uppercase());
            if lo != up {
                ExtPat::Any(vec![lo, up])
            } else {
                ExtPat::Char(*c)
            }
        }
    }
}

/// Whether `ep` opens with a top-level `^`, or every alternative of a
/// top-level `Or` does.
pub(crate) fn starts_with_carat(ep: &ExtPat) -> bool {
    match ep {
        ExtPat::Carat => true,
        ExtPat::Concat(parts) => parts.first().is_some_and(starts_with_carat),
        ExtPat::Group(inner) | ExtPat::GroupNonMarking(inner) => starts_with_carat(inner),
        ExtPat::Or(branches) => branches.iter().all(starts_with_carat),
        _ => false,
    }
}

/// Symmetric counterpart of `starts_with_carat` for a trailing `$`.
pub(crate) fn ends_with_dollar(ep: &ExtPat) -> bool {
    match ep {
        ExtPat::Dollar => true,
        ExtPat::Concat(parts) => parts.last().is_some_and(ends_with_dollar),
        ExtPat::Group(inner) | ExtPat::GroupNonMarking(inner) => ends_with_dollar(inner),
        ExtPat::Or(branches) => branches.iter().all(ends_with_dollar),
        _ => false,
    }
}

fn both_cases(cs: &[char]) -> Vec<char> {
    let mut v = Vec::with_capacity(cs.len() * 2);
    for &c in cs {
        v.push(c.to_ascii_lowercase());
        v.push(c.to_ascii_uppercase());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_fold_letter_becomes_any_both_cases() {
        assert_eq!(case_fold(&ExtPat::Char('a')), ExtPat::Any(vec!['a', 'A']));
        assert_eq!(case_fold(&ExtPat::Char('Z')), ExtPat::Any(vec!['z', 'Z']));
    }

    #[test]
    fn case_fold_digit_is_unchanged() {
        assert_eq!(case_fold(&ExtPat::Char('5')), ExtPat::Char('5'));
    }

    #[test]
    fn case_fold_escape_shorthand_passes_through() {
        assert_eq!(case_fold(&ExtPat::Escape('d')), ExtPat::Escape('d'));
    }

    #[test]
    fn case_fold_recurses_into_concat() {
        let ep = ExtPat::Concat(vec![ExtPat::Char('a'), ExtPat::Char('b')]);
        assert_eq!(
            case_fold(&ep),
            ExtPat::Concat(vec![ExtPat::Any(vec!['a', 'A']), ExtPat::Any(vec!['b', 'B'])])
        );
    }

    #[test]
    fn case_fold_any_class_gets_both_cases() {
        assert_eq!(case_fold(&ExtPat::Any(vec!['a', 'b'])), ExtPat::Any(vec!['a', 'A', 'b', 'B']));
    }
}