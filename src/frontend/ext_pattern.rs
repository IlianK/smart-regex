//! regex-engine/src/frontend/ext_pattern.rs
//!
//! `ExtPat`: external surface-syntax pattern AST
//! modeled on `Text.Regex.PDeriv.ExtPattern`'s `EPat` 

#[derive(Debug, Clone, PartialEq)]
pub enum ExtPat {
    /// the empty string, `translate`s to `Regex::Eps`.
    Empty,
    Eps,
    /// Literal `∅` token `translate`s to `Regex::Phi`.
    Never,
    /// A marking group `( ... )`.
    Group(Box<ExtPat>),
    /// A non-marking group `(?: ... )`.
    GroupNonMarking(Box<ExtPat>),
    /// Alternation `a|b|c`.
    Or(Vec<ExtPat>),
    /// Concatenation `ab c`.
    Concat(Vec<ExtPat>),
    /// `r?` / `r??` (bool: greedy).
    Opt(Box<ExtPat>, bool),
    /// `r+` / `r+?` (bool: greedy).
    Plus(Box<ExtPat>, bool),
    /// `r*` / `r*?` (bool: greedy).
    Star(Box<ExtPat>, bool),
    /// `r{lo,hi}` / `r{lo,}` / `r{lo}` (bool: greedy).
    Bound(Box<ExtPat>, u32, Option<u32>, bool),
    /// `^`.
    Carat,
    /// `$`.
    Dollar,
    /// `.`.
    Dot,
    /// `[ ... ]` listed characters (ranges and `\d`/`\w`/`\s`
    /// shorthands already expanded by the parser).
    Any(Vec<char>),
    /// `[^ ... ]`.
    NoneOf(Vec<char>),
    /// `\c` for a `c` that carries regex meaning to `translate`
    /// (`d`/`D`/`w`/`W`/`s`/`S`) or, for any other `c`, an escaped literal
    /// (`\.`, `\(`, `\\`, ...).
    Escape(char),
    /// An ordinary, unescaped literal character.
    Char(char),
}

/// Case-fold an `ExtPat`: every literal letter becomes "either case"
/// Class shorthands (`\d`/`\w`/`\s`/...) are already case-agnostic 
pub fn case_fold(ep: &ExtPat) -> ExtPat {
    match ep {
        ExtPat::Empty => ExtPat::Empty,
        ExtPat::Eps => ExtPat::Eps,
        ExtPat::Never => ExtPat::Never,
        ExtPat::Group(inner) => ExtPat::Group(Box::new(case_fold(inner))),
        ExtPat::GroupNonMarking(inner) => ExtPat::GroupNonMarking(Box::new(case_fold(inner))),
        ExtPat::Or(alts) => ExtPat::Or(alts.iter().map(case_fold).collect()),
        ExtPat::Concat(parts) => ExtPat::Concat(parts.iter().map(case_fold).collect()),
        ExtPat::Opt(inner, g) => ExtPat::Opt(Box::new(case_fold(inner)), *g),
        ExtPat::Plus(inner, g) => ExtPat::Plus(Box::new(case_fold(inner)), *g),
        ExtPat::Star(inner, g) => ExtPat::Star(Box::new(case_fold(inner)), *g),
        ExtPat::Bound(inner, lo, hi, g) => ExtPat::Bound(Box::new(case_fold(inner)), *lo, *hi, *g),
        ExtPat::Carat => ExtPat::Carat,
        ExtPat::Dollar => ExtPat::Dollar,
        ExtPat::Dot => ExtPat::Dot,
        ExtPat::Any(cs) => ExtPat::Any(both_cases(cs)),
        ExtPat::NoneOf(cs) => ExtPat::NoneOf(both_cases(cs)),
        ExtPat::Escape(c) => ExtPat::Escape(*c),
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

/// Does `ep` open with a top-level `^`?
pub(crate) fn starts_with_carat(ep: &ExtPat) -> bool {
    match ep {
        ExtPat::Carat => true,
        ExtPat::Concat(parts) => parts.first().is_some_and(starts_with_carat),
        ExtPat::Group(inner) | ExtPat::GroupNonMarking(inner) => starts_with_carat(inner),
        _ => false,
    }
}

/// Symmetric counterpart of `starts_with_carat` for a trailing `$`.
pub(crate) fn ends_with_dollar(ep: &ExtPat) -> bool {
    match ep {
        ExtPat::Dollar => true,
        ExtPat::Concat(parts) => parts.last().is_some_and(ends_with_dollar),
        ExtPat::Group(inner) | ExtPat::GroupNonMarking(inner) => ends_with_dollar(inner),
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

// -------------------------------
// Tests
// -------------------------------

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
